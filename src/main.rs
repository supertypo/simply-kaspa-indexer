extern crate diesel;

use std::env;
use std::sync::Arc;
use std::time::Duration;

use crossbeam_queue::ArrayQueue;
use diesel::pg::PgConnection;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use dotenvy::dotenv;
use kaspa_rpc_core::api::rpc::RpcApi;
use kaspa_wrpc_client::KaspaRpcClient;
use log::info;
use tokio::task;

use kaspa_db_filler_ng::blocks::fetch_blocks::fetch_blocks;
use kaspa_db_filler_ng::blocks::insert_blocks::insert_blocks;
use kaspa_db_filler_ng::blocks::process_blocks::process_blocks;
use kaspa_db_filler_ng::kaspad::client::connect_kaspad;
use kaspa_db_filler_ng::transactions::insert_transactions::insert_transactions;
use kaspa_db_filler_ng::transactions::process_transactions::process_transactions;
use kaspa_db_filler_ng::vars::vars::load_start_block_hash;
use kaspa_db_filler_ng::virtual_chain::fetch_virtual_chain::fetch_virtual_chains;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!();

#[tokio::main]
async fn main() {
    dotenv().ok();
    env_logger::builder()
        .format_timestamp_millis()
        .init();

    let db_url = env::var("DATABASE_URL").unwrap_or(String::from("postgres://postgres:postgres@localhost:5432/postgres"));
    let db_pool = Pool::builder()
        .test_on_check_out(true)
        .connection_timeout(Duration::from_secs(10))
        .max_size(20)
        .build(ConnectionManager::<PgConnection>::new(&db_url))
        .expect("Database pool FAILED");
    let db_con = &mut db_pool.get()
        .expect("Database connection FAILED");
    info!("Connection to database established");

    if env::var("DEVELOPMENT").unwrap().eq_ignore_ascii_case("true") {
        info!("Applying pending migrations");
        db_con.run_pending_migrations(MIGRATIONS)
            .expect("Unable to apply pending migrations");
    }

    let url = env::var("KASPAD_RPC_URL").unwrap_or(String::from("ws://localhost:17110"));
    let network = env::var("KASPAD_NETWORK").unwrap_or(String::from("mainnet"));
    let kaspad_client = connect_kaspad(url, network).await.expect("Kaspad connection FAILED");

    start_processing(db_pool, kaspad_client).await.expect("Unreachable");
}

async fn start_processing(db_pool: Pool<ConnectionManager<PgConnection>>, kaspad_client: KaspaRpcClient) -> Result<(), ()> {
    let block_dag_info = kaspad_client.get_block_dag_info().await.expect("Error when invoking GetBlockDagInfo");
    info!("BlockDagInfo received: pruning_point={}",block_dag_info.pruning_point_hash);
    let start_block_hash = load_start_block_hash(db_pool.clone()).unwrap_or(block_dag_info.pruning_point_hash.to_string());

    let rpc_blocks_queue = Arc::new(ArrayQueue::new(5_000));
    let rpc_transactions_queue = Arc::new(ArrayQueue::new(200_000));
    let db_blocks_queue = Arc::new(ArrayQueue::new(5_000));
    let db_transactions_queue = Arc::new(ArrayQueue::new(200_000));
    let synced_queue = Arc::new(ArrayQueue::new(10));

    let mut tasks = vec![];
    tasks.push(task::spawn(fetch_blocks(start_block_hash.clone(), kaspad_client.clone(), synced_queue.clone(), rpc_blocks_queue.clone(), rpc_transactions_queue.clone())));
    tasks.push(task::spawn(process_blocks(rpc_blocks_queue.clone(), db_blocks_queue.clone())));
    tasks.push(task::spawn(process_transactions(rpc_transactions_queue.clone(), db_transactions_queue.clone(), db_pool.clone())));
    tasks.push(task::spawn(insert_blocks(db_blocks_queue.clone(), db_pool.clone())));
    tasks.push(task::spawn(insert_transactions(db_transactions_queue.clone(), db_pool.clone())));
    tasks.push(task::spawn(fetch_virtual_chains(start_block_hash.clone(), synced_queue.clone(), kaspad_client.clone(), db_pool.clone())));

    for task in tasks {
        let _ = task.await.expect("Should not happen");
    }
    Ok(())
}
