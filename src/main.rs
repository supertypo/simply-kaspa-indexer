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
use log::{info, warn};
use tokio::task;

use kaspa_db_filler_ng::blocks::fetch_blocks::fetch_blocks;
use kaspa_db_filler_ng::blocks::insert_blocks::insert_blocks;
use kaspa_db_filler_ng::blocks::process_blocks::process_blocks;
use kaspa_db_filler_ng::kaspad::client::connect_kaspad;
use kaspa_db_filler_ng::transactions::insert_transactions::insert_transactions;
use kaspa_db_filler_ng::transactions::insert_tx_inputs::insert_tx_inputs;
use kaspa_db_filler_ng::transactions::insert_tx_outputs::insert_tx_outputs;
use kaspa_db_filler_ng::transactions::process_transactions::process_transactions;
use kaspa_db_filler_ng::vars::vars::{load_block_checkpoint, load_virtual_checkpoint};
use kaspa_db_filler_ng::virtual_chain::process_virtual_chain::process_virtual_chain;

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
    
    let block_checkpoint_hash = load_block_checkpoint(db_pool.clone()).unwrap_or_else(|| {
        warn!("block_checkpoint_hash not found, using pruning_point_hash");
        block_dag_info.pruning_point_hash.to_string()
    });
    let virtual_checkpoint_hash = load_virtual_checkpoint(db_pool.clone()).unwrap_or_else(|| {
        warn!("virtual_checkpoint_hash not found, using pruning_point_hash");
        block_dag_info.pruning_point_hash.to_string()
    });

    let rpc_blocks_queue = Arc::new(ArrayQueue::new(3_000));
    let rpc_transactions_queue = Arc::new(ArrayQueue::new(3_000));
    let db_blocks_queue = Arc::new(ArrayQueue::new(3_000));
    let db_transactions_queue = Arc::new(ArrayQueue::new(30_000));
    let db_transactions_inputs_queue = Arc::new(ArrayQueue::new(60_000));
    let db_transactions_outputs_queue = Arc::new(ArrayQueue::new(60_000));
    let synced_queue = Arc::new(ArrayQueue::new(10));

    let mut tasks = vec![];
    tasks.push(task::spawn(fetch_blocks(block_checkpoint_hash, kaspad_client.clone(), synced_queue.clone(), rpc_blocks_queue.clone(), rpc_transactions_queue.clone())));
    tasks.push(task::spawn(process_blocks(rpc_blocks_queue.clone(), db_blocks_queue.clone())));
    tasks.push(task::spawn(process_transactions(rpc_transactions_queue.clone(), db_transactions_queue.clone(), db_transactions_inputs_queue.clone(), db_transactions_outputs_queue.clone(), db_pool.clone())));
    tasks.push(task::spawn(insert_blocks(db_blocks_queue.clone(), db_pool.clone())));
    tasks.push(task::spawn(insert_transactions(db_transactions_queue.clone(), db_pool.clone())));
    tasks.push(task::spawn(insert_tx_inputs(db_transactions_inputs_queue.clone(), db_pool.clone())));
    tasks.push(task::spawn(insert_tx_outputs(db_transactions_outputs_queue.clone(), db_pool.clone())));
    tasks.push(task::spawn(process_virtual_chain(virtual_checkpoint_hash, synced_queue.clone(), kaspad_client.clone(), db_pool.clone())));

    for task in tasks {
        let _ = task.await.expect("Should not happen");
    }
    Ok(())
}
