extern crate diesel;

use std::env;
use std::ops::DerefMut;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use crossbeam_queue::ArrayQueue;
use diesel::{insert_into, Insertable, r2d2, RunQueryDsl};
use diesel::pg::PgConnection;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use dotenvy::dotenv;
use kaspa_rpc_core::api::rpc::RpcApi;
use kaspa_rpc_core::RpcBlock;
use kaspa_wrpc_client::KaspaRpcClient;
use log::{debug, error};
use log::info;
use tokio::task;
use tokio::time::sleep;

use kaspa_db_filler_ng::database::models::Block;
use kaspa_db_filler_ng::database::schema::blocks::dsl;
use kaspa_db_filler_ng::kaspad::client::connect;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!();

#[tokio::main]
async fn main() {
    dotenv().ok();
    env_logger::init();

    let db_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let db_manager = ConnectionManager::<PgConnection>::new(&db_url);
    let db_pool = r2d2::Pool::builder()
        .test_on_check_out(true)
        .connection_timeout(Duration::from_secs(10))
        .build(db_manager)
        .expect("Could not build connection pool");
    let db_con = &mut db_pool.get()
        .expect("Unable to get a new connection from the pool");
    info!("Connection to the database established!");

    if env::var("DEVELOPMENT").unwrap().eq_ignore_ascii_case("true") {
        info!("Applying pending migrations");
        db_con.run_pending_migrations(MIGRATIONS)
            .expect("Unable to apply pending migrations");
    }
    //
    // info!("Inserting test data");
    // for i in 0..9 {
    //     insert_into(dsl::blocks)
    //         .values(schema::blocks::hash.eq(hex::decode(format!("aba{}", i)).expect("")))
    //         .on_conflict_do_nothing()
    //         .execute(db_con)
    //         .expect("Failed to insert test data");
    // }
    //
    // info!("Fetching test data");
    // let results = dsl::blocks
    //     .select(schema::blocks::hash)
    //     .load::<Vec<u8>>(db_con)
    //     .expect("Failed to retrieve test data");
    //
    // info!("Displaying {} blocks:", results.len());
    // for r in results {
    //     info!("hash: {}", hex::encode(r))
    // }

    run_loop(&db_pool).await.expect("TODO");
}

async fn run_loop(db_pool: &Pool<ConnectionManager<PgConnection>>) -> Result<(), ()> {
    loop {
        while db_pool.get()
            .inspect_err(|e| error!("Postgres connection FAILED. Reason: '{e}'. Retrying in 10 seconds..."))
            .is_err() {
            sleep(Duration::from_secs(10)).await;
        }

        let url = env::var("KASPAD_RPC_URL").expect("KASPAD_RPC_URL must be set");
        let network = env::var("KASPAD_NETWORK").map(|n| Some(n)).unwrap_or_default();
        let mut client_option = None;
        while client_option.is_none() {
            match connect(&url, &network).await {
                Ok(r) => client_option = Some(r),
                Err(e) => {
                    info!("Kaspad connection FAILED. Reason: '{e}'. Retrying in 10 seconds...");
                    sleep(Duration::from_secs(10)).await;
                }
            };
        }
        let client = client_option.unwrap();

        let rpc_blocks_queue = Arc::new(ArrayQueue::new(1000));
        let db_blocks_queue = Arc::new(ArrayQueue::new(1000));
        let mut tasks = vec![];
        tasks.push(task::spawn(fetch_blocks(client, rpc_blocks_queue.clone())));
        tasks.push(task::spawn(process_blocks(rpc_blocks_queue.clone(), db_blocks_queue.clone())));
        tasks.push(task::spawn(insert_blocks(db_blocks_queue.clone(), db_pool.clone())));

        let mut abort = false;
        for task in tasks {
            if abort {
                task.abort();
            } else {
                match task.await {
                    Ok(_) => {
                        info!("Task unexpectedly completed. Shutting down remaining tasks");
                        abort = true;
                    }
                    Err(e) => {
                        info!("Task FAILURE: {e}. Shutting down remaining tasks");
                        abort = true;
                    }
                }
            }
        }
        info!("Sleeping 10 seconds before restarting...");
        sleep(Duration::from_secs(10)).await;
    }
}

async fn fetch_blocks(client: KaspaRpcClient, rpc_blocks_queue: Arc<ArrayQueue<RpcBlock>>) -> Result<(), ()> {
    let block_dag_info = client.get_block_dag_info().await
        .expect("Error when invoking GetBlockDagInfo");
    info!("BlockDagInfo received: pruning_point={}", block_dag_info.pruning_point_hash);
    let mut lowhash = block_dag_info.pruning_point_hash;

    loop {
        info!("Getting blocks with lowhash={}", lowhash);
        let res = client.get_blocks(Some(lowhash), true, true).await
            .expect("Error when invoking GetBlocks");
        info!("Received {} blocks", res.block_hashes.len());

        for b in res.blocks {
            while rpc_blocks_queue.is_full() {
                debug!("RPC blocks queue is full, sleeping 2 seconds...");
                sleep(Duration::from_secs(2)).await;
            }
            rpc_blocks_queue.push(b).unwrap();
        }
        lowhash = *res.block_hashes.last().unwrap();
    }
}

async fn process_blocks(rpc_blocks_queue: Arc<ArrayQueue<RpcBlock>>,
                        db_blocks_queue: Arc<ArrayQueue<Block>>) -> Result<(), ()> {
    loop {
        let block_option = rpc_blocks_queue.pop();
        if block_option.is_none() {
            debug!("RPC blocks queue is empty, sleeping 2 seconds...");
            sleep(Duration::from_secs(2)).await;
            continue;
        }
        let block = block_option.unwrap();
        let db_block = Block {
            hash: block.header.hash.as_bytes().to_vec(),
            accepted_id_merkle_root: None,
            difficulty: None,
            is_chain_block: None,
            merge_set_blues_hashes: None,
            merge_set_reds_hashes: None,
            selected_parent_hash: None,
            bits: None,
            blue_score: None,
            blue_work: None,
            daa_score: None,
            hash_merkle_root: None,
            nonce: None,
            parents: None,
            pruning_point: None,
            timestamp: None,
            utxo_commitment: None,
            version: None,
        };
        while db_blocks_queue.is_full() {
            debug!("DB blocks queue is full, sleeping 2 seconds...");
            sleep(Duration::from_secs(2)).await;
        }
        let _ = db_blocks_queue.push(db_block);
    }
}

async fn insert_blocks(db_blocks_queue: Arc<ArrayQueue<Block>>, db_pool: Pool<ConnectionManager<PgConnection>>) -> Result<(), ()> {
    // loop {
        info!("Process blocks started");
        // let con = match db_pool.get() {
        //     Ok(r) => { Some(r) }
        //     Err(e) => {
        //         info!("Database connection failed with {}. Sleeping for 10 seconds...", e);
        //         sleep(Duration::from_secs(10)).await;
        //         continue;
        //     }
        // };
        // let mut insert_queue = vec![];
        // let mut last_commit_time = SystemTime::now();
        loop {
            let con = match db_pool.get() {
                Ok(r) => { Some(r) }
                Err(e) => {
                    info!("Database connection failed with {}. Sleeping for 10 seconds...", e);
                    sleep(Duration::from_secs(10)).await;
                    continue;
                }
            };
            // if insert_queue.len() >= 100 || SystemTime::now().duration_since(last_commit_time).unwrap().as_secs() > 2 {
            //     let inserted_rows = insert_into(dsl::blocks)
            //         .values(&insert_queue)
            //         .on_conflict_do_nothing()
            //         .execute(con.unwrap().deref_mut());
            //     info!("Commited {} rows to database", inserted_rows.unwrap());
            //     insert_queue.clear();
            //     last_commit_time = SystemTime::now();
            // }
            let block_option = db_blocks_queue.pop();
            if block_option.is_some() {
                // insert_queue.push(block_option.unwrap());
                let a = insert_into(dsl::blocks)
                    // .values(&insert_queue)
                    .values(block_option.unwrap())
                    .on_conflict_do_nothing()
                    .execute(con.unwrap().deref_mut());
            }
        }
    // }
}
//
// async fn process_blocks(db_pool: Pool<ConnectionManager<PgConnection>>, blocks_queue: Arc<ArrayQueue<RpcBlock>>) -> Result<(), ()> {
//     loop {
//         info!("Process blocks started");
//         let con = match db_pool.get() {
//             Ok(r) => { Some(r) }
//             Err(e) => {
//                 info!("Database connetion failed with {}. Sleeping for 10 seconds...", e);
//                 sleep(Duration::from_secs(10)).await;
//             }
//         };
//         if con.is_none() {
//             continue;
//         }
//         loop {
//             let block_option = blocks_queue.pop();
//             if block_option.is_some() {
//                 let block = block_option.unwrap();
//                 let b = vec![
//                     hash.eq(block.header.hash.as_bytes().to_vec())
//                 ];
//                 insert_into(blocks)
//                     .values(b)
//                     .on_conflict_do_nothing()
//                     .execute(con.unwrap().deref_mut())
//                     .expect("Unable to persist block");
//                 info!("Inserted block {}", block.header.hash);
//             } else {
//                 info!("Queue is empty, sleeping 2 seconds...");
//                 sleep(Duration::from_secs(2)).await;
//             }
//         }
//     }
// }
