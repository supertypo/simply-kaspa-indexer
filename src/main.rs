extern crate diesel;

use std::env;
use std::sync::Arc;
use std::time::Duration;

use crossbeam_queue::ArrayQueue;
use diesel::{ExpressionMethods, insert_into, QueryDsl, r2d2, RunQueryDsl};
use diesel::pg::PgConnection;
use diesel::r2d2::{ConnectionManager, Pool, PooledConnection};
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use dotenvy::dotenv;
use kaspa_rpc_core::api::rpc::RpcApi;
use kaspa_rpc_core::RpcBlock;
use kaspa_wrpc_client::KaspaRpcClient;
use tokio::task;
use tokio::time::sleep;

use kaspa_db_filler_ng::database::schema::blocks::dsl::blocks;
use kaspa_db_filler_ng::database::schema::blocks::hash;
use kaspa_db_filler_ng::kaspad::client::connect;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!();

#[tokio::main]
async fn main() {
    dotenv().ok();

    let db_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let db_manager = ConnectionManager::<PgConnection>::new(&db_url);
    let db_pool = r2d2::Pool::builder()
        .test_on_check_out(true)
        .connection_timeout(Duration::from_secs(10))
        .build(db_manager)
        .expect("Could not build connection pool");
    let db_con = &mut db_pool.get()
        .expect("Unable to get a new connection from the pool");
    println!("Connection to the database established!");

    if env::var("DEVELOPMENT").unwrap().eq_ignore_ascii_case("true") {
        println!("Applying pending migrations");
        db_con.run_pending_migrations(MIGRATIONS)
            .expect("Unable to apply pending migrations");
    }

    println!("Inserting test data");
    for i in 0..9 {
        insert_into(blocks)
            .values(hash.eq(hex::decode(format!("aba{}", i)).expect("")))
            .on_conflict_do_nothing()
            .execute(db_con)
            .expect("Failed to insert test data");
    }

    println!("Fetching test data");
    let results = blocks
        .select(hash)
        .load::<Vec<u8>>(db_con)
        .expect("Failed to retrieve test data");

    println!("Displaying {} blocks:", results.len());
    for r in results {
        println!("hash: {}", hex::encode(r))
    }

    run_loop(db_pool).await.expect("TODO");
}

async fn run_loop(db_pool: Pool<ConnectionManager<PgConnection>>) -> Result<(), ()> {
    loop {
        let mut con_option = None;
        while con_option.is_none() {
            match db_pool.get() {
                Ok(r) => con_option = Some(r),
                Err(e) => {
                    println!("Postgres connection FAILED. Reason: '{e}'. Retrying in 10 seconds...");
                    sleep(Duration::from_secs(10)).await;
                }
            }
        }

        let url = env::var("KASPAD_RPC_URL").expect("KASPAD_RPC_URL must be set");
        let network = env::var("KASPAD_NETWORK").map(|n| Some(n)).unwrap_or_default();
        let mut client_option = None;
        while client_option.is_none() {
            match connect(&url, &network).await {
                Ok(r) => client_option = Some(r),
                Err(e) => {
                    println!("Kaspad connection FAILED. Reason: '{e}'. Retrying in 10 seconds...");
                    sleep(Duration::from_secs(10)).await;
                }
            };
        }
        let client = client_option.unwrap();

        let blocks_queue = Arc::new(ArrayQueue::new(1000));
        let mut tasks = vec![];
        tasks.push(task::spawn(fetch_blocks(client, blocks_queue.clone())));
        tasks.push(task::spawn(process_blocks(con_option, blocks_queue.clone())));

        let mut abort = false;
        for task in tasks {
            if abort {
                task.abort();
            } else {
                match task.await {
                    Ok(_) => {
                        println!("Task unexpectedly completed. Shutting down remaining tasks");
                        abort = true;
                    }
                    Err(e) => {
                        println!("Task FAILURE: {e}. Shutting down remaining tasks");
                        abort = true;
                    }
                }
            }
        }
        println!("Sleeping 10 seconds before restarting...");
        sleep(Duration::from_secs(10)).await;
    }
}

async fn fetch_blocks(client: KaspaRpcClient, blocks_queue: Arc<ArrayQueue<RpcBlock>>) -> Result<(), ()> {
    let block_dag_info = client.get_block_dag_info().await
        .expect("Error when invoking GetBlockDagInfo");
    println!("BlockDagInfo received: pruning_point={}, first_parent={}",
             block_dag_info.pruning_point_hash, block_dag_info.virtual_parent_hashes[0]);
    let mut lowhash = block_dag_info.pruning_point_hash;

    loop {
        println!("Getting blocks with lowhash={}", lowhash);
        let res = client.get_blocks(Some(lowhash), true, true).await
            .expect("Error when invoking GetBlocks");
        println!("Adding {} blocks to queue", res.block_hashes.len());

        for b in res.blocks {
            while blocks_queue.is_full() {
                println!("Queue is full, sleeping 2 seconds...");
                sleep(Duration::from_secs(2)).await;
            }
            blocks_queue.push(b).unwrap();
        }
        lowhash = *res.block_hashes.last().unwrap();
    }
}

async fn process_blocks(con_option: Option<PooledConnection<ConnectionManager<PgConnection>>>, blocks_queue: Arc<ArrayQueue<RpcBlock>>) -> Result<(), ()> {
    let con = &mut con_option.unwrap();
    loop {
        let block_option = blocks_queue.pop();
        if block_option.is_some() {
            let block = block_option.unwrap();
            let b = vec![
                hash.eq(block.header.hash.as_bytes().to_vec())
            ];
            insert_into(blocks)
                .values(b)
                .on_conflict_do_nothing()
                .execute(con)
                .expect("Unable to persist block");
            println!("Inserted block {}", block.header.hash);
        } else {
            println!("Queue is empty, sleeping 2 seconds...");
            sleep(Duration::from_secs(2)).await;
        }
    }
}
