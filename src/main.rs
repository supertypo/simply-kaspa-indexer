extern crate diesel;

use std::collections::HashSet;
use std::env;
use std::ops::{DerefMut, Div};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use bigdecimal::BigDecimal;
use crossbeam_queue::ArrayQueue;
use diesel::{Connection, ExpressionMethods, insert_into, Insertable, r2d2, RunQueryDsl};
use diesel::pg::PgConnection;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::result::Error;
use diesel::upsert::excluded;
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use dotenvy::dotenv;
use kaspa_rpc_core::api::rpc::RpcApi;
use kaspa_rpc_core::RpcBlock;
use kaspa_wrpc_client::KaspaRpcClient;
use log::{debug, error, trace, warn};
use log::info;
use tokio::task;
use tokio::time::sleep;

use kaspa_db_filler_ng::database::models::Block;
use kaspa_db_filler_ng::database::schema::blocks;
use kaspa_db_filler_ng::kaspad::client::connect;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!();

#[tokio::main]
async fn main() {
    dotenv().ok();
    env_logger::builder()
        .format_timestamp_millis()
        .init();

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
    info!("BlockDagInfo received: pruning_point={}, first_parent={}",
             block_dag_info.pruning_point_hash, block_dag_info.virtual_parent_hashes[0]);

    // let start_point = block_dag_info.pruning_point_hash.to_string(); // FIXME: Use start point
    // let start_point = block_dag_info.virtual_parent_hashes[0].to_string();
    let start_point = "6abe8c84bc86555a288c9506d7c4782ea26e8a2d2f103de14df161b633e0c2ac";

    info!("start_point={}", start_point);
    let start_hash = kaspa_hashes::Hash::from_slice(hex::decode(start_point.as_bytes()).unwrap().as_slice());
    let mut low_hash = start_hash;

    loop {
        let start_time = SystemTime::now();
        info!("Getting blocks with low_hash={}", low_hash);
        let res = client.get_blocks(Some(low_hash), true, true).await
            .expect("Error when invoking GetBlocks");
        info!("Received {} blocks", res.blocks.len());
        trace!("Block hashes: \n{:#?}", res.block_hashes);

        let blocks_len = res.blocks.len();
        if blocks_len > 1 {
            low_hash = res.blocks.last().unwrap().header.hash;
            for b in res.blocks {
                let block_hash = b.header.hash;
                if block_hash == low_hash && block_hash != start_hash {
                    trace!("Ignoring low_hash block {}", low_hash);
                    continue;
                }
                while rpc_blocks_queue.is_full() {
                    warn!("RPC blocks queue is full, sleeping 2 seconds...");
                    sleep(Duration::from_secs(2)).await;
                }
                rpc_blocks_queue.push(b).unwrap();
            }
        }
        debug!("Fetch blocks BPS: {}", 1000 * blocks_len as u128
            / SystemTime::now().duration_since(start_time).unwrap().as_millis());
        if blocks_len < 50 && SystemTime::now().duration_since(start_time).unwrap().as_secs() < 3 {
            sleep(Duration::from_secs(2)).await;
        }
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
            accepted_id_merkle_root: Some(block.header.accepted_id_merkle_root.as_bytes().to_vec()),
            difficulty: block.verbose_data.as_ref().map(|v| v.difficulty),
            is_chain_block: block.verbose_data.as_ref().map(|v| v.is_chain_block),
            merge_set_blues_hashes: block.verbose_data.as_ref().map(|v| v.merge_set_blues_hashes.iter()
                .map(|w| Some(w.as_bytes().to_vec())).collect()),
            merge_set_reds_hashes: block.verbose_data.as_ref().map(|v| v.merge_set_reds_hashes.iter()
                .map(|w| Some(w.as_bytes().to_vec())).collect()),
            selected_parent_hash: block.verbose_data.as_ref().map(|v| v.selected_parent_hash.as_bytes().to_vec()),
            bits: block.header.bits.into(),
            blue_score: block.header.blue_score as i64,
            blue_work: block.header.blue_work.to_be_bytes_var(),
            daa_score: block.header.daa_score as i64,
            hash_merkle_root: block.header.hash_merkle_root.as_bytes().to_vec(),
            nonce: BigDecimal::from(block.header.nonce),
            parents: block.header.parents_by_level[0].iter().map(|v| Some(v.as_bytes().to_vec())).collect(),
            pruning_point: block.header.pruning_point.as_bytes().to_vec(),
            timestamp: block.header.timestamp.div(1000) as i32,
            utxo_commitment: block.header.utxo_commitment.as_bytes().to_vec(),
            version: block.header.version as i16,
        };
        while db_blocks_queue.is_full() {
            warn!("DB blocks queue is full, sleeping 2 seconds...");
            sleep(Duration::from_secs(2)).await;
        }
        let _ = db_blocks_queue.push(db_block);
    }
}

async fn insert_blocks(db_blocks_queue: Arc<ArrayQueue<Block>>, db_pool: Pool<ConnectionManager<PgConnection>>) -> Result<(), ()> {
    const INSERT_QUEUE_SIZE: usize = 500;
    loop {
        info!("Process blocks started");
        let mut insert_queue: HashSet<Block> = HashSet::with_capacity(INSERT_QUEUE_SIZE);
        let mut last_commit_time = SystemTime::now();
        loop {
            if insert_queue.len() >= INSERT_QUEUE_SIZE || (insert_queue.len() >= 1 && SystemTime::now().duration_since(last_commit_time).unwrap().as_secs() > 2) {
                let con_option = match db_pool.get() {
                    Ok(r) => { Some(r) }
                    Err(e) => {
                        info!("Database connection failed with {}. Sleeping for 10 seconds...", e);
                        sleep(Duration::from_secs(10)).await;
                        None
                    }
                };
                if con_option.is_none() {
                    continue;
                }
                let _ = con_option.unwrap().deref_mut().transaction(|con| {
                    match insert_into(blocks::dsl::blocks)
                        .values(Vec::from_iter(&insert_queue))
                        .on_conflict(blocks::hash)
                        .do_update()
                        .set((
                            blocks::hash.eq(excluded(blocks::hash)),
                            blocks::accepted_id_merkle_root.eq(excluded(blocks::accepted_id_merkle_root)),
                            blocks::difficulty.eq(excluded(blocks::difficulty)),
                            blocks::is_chain_block.eq(excluded(blocks::is_chain_block)),
                            blocks::merge_set_blues_hashes.eq(excluded(blocks::merge_set_blues_hashes)),
                            blocks::merge_set_reds_hashes.eq(excluded(blocks::merge_set_reds_hashes)),
                            blocks::selected_parent_hash.eq(excluded(blocks::selected_parent_hash)),
                            blocks::bits.eq(excluded(blocks::bits)),
                            blocks::blue_score.eq(excluded(blocks::blue_score)),
                            blocks::blue_work.eq(excluded(blocks::blue_work)),
                            blocks::daa_score.eq(excluded(blocks::daa_score)),
                            blocks::hash_merkle_root.eq(excluded(blocks::hash_merkle_root)),
                            blocks::nonce.eq(excluded(blocks::nonce)),
                            blocks::parents.eq(excluded(blocks::parents)),
                            blocks::pruning_point.eq(excluded(blocks::pruning_point)),
                            blocks::timestamp.eq(excluded(blocks::timestamp)),
                            blocks::utxo_commitment.eq(excluded(blocks::utxo_commitment)),
                            blocks::version.eq(excluded(blocks::version))))
                        .execute(con) {
                        Ok(inserted_rows) => {
                            info!("Committed {} new rows to database", inserted_rows);
                            insert_queue = HashSet::with_capacity(INSERT_QUEUE_SIZE);
                            last_commit_time = SystemTime::now();
                        }
                        Err(e) => {
                            error!("Commit to database failed: {e}");
                        }
                    };
                    Ok::<_, Error>(())
                }).unwrap();
            }
            if insert_queue.len() >= INSERT_QUEUE_SIZE { // In case db insertion failed
                sleep(Duration::from_secs(1)).await;
            } else {
                let block_option = db_blocks_queue.pop();
                if block_option.is_some() {
                    insert_queue.insert(block_option.unwrap());
                } else {
                    sleep(Duration::from_secs(1)).await;
                }
            }
        }
    }
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
