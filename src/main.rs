extern crate diesel;

use std::env;
use std::sync::Arc;
use std::time::Duration;

use clap::{Arg, Command, crate_description, crate_name};
use crossbeam_queue::ArrayQueue;
use diesel::pg::PgConnection;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use kaspa_rpc_core::api::rpc::RpcApi;
use kaspa_wrpc_client::KaspaRpcClient;
use log::{info, warn};
use tokio::task;

use kaspa_db_filler_ng::blocks::fetch_blocks::fetch_blocks;
use kaspa_db_filler_ng::blocks::insert_blocks::insert_blocks;
use kaspa_db_filler_ng::blocks::process_blocks::process_blocks;
use kaspa_db_filler_ng::kaspad::client::connect_kaspad;
use kaspa_db_filler_ng::transactions::insert_transactions::insert_txs_ins_outs;
use kaspa_db_filler_ng::transactions::process_transactions::process_transactions;
use kaspa_db_filler_ng::vars::vars::{load_block_checkpoint, load_virtual_checkpoint};
use kaspa_db_filler_ng::virtual_chain::process_virtual_chain::process_virtual_chain;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!();

#[tokio::main]
async fn main() {
    let matches = Command::new(crate_name!())
        .about(crate_description!())
        .arg(Arg::new("rpc-url")
            .short('s')
            .long("rpc-url")
            .help("The url to a kaspad instance")
            .default_value("ws://127.0.0.1:17110")
            .action(clap::ArgAction::Set))
        .arg(Arg::new("network")
            .short('n')
            .long("network")
            .help("The network type and suffix, e.g. 'testnet-11'")
            .default_value("mainnet")
            .action(clap::ArgAction::Set))
        .arg(Arg::new("database-url")
            .short('d')
            .long("database-url")
            .help("The url to a PostgreSQL instance")
            .default_value("postgres://postgres:postgres@localhost:5432/postgres")
            .action(clap::ArgAction::Set))
        .arg(Arg::new("log-level")
            .short('l')
            .long("log-level")
            .help("error, warn, info, debug, trace, off")
            .default_value("info")
            .action(clap::ArgAction::Set))
        .arg(Arg::new("from-pruning-point")
            .short('p')
            .long("from-pruning-point")
            .help("Start from the pruning point instead of a virtual parent")
            .action(clap::ArgAction::SetTrue))
        .arg(Arg::new("ignore-checkpoint")
            .short('i')
            .long("ignore-checkpoint")
            .help("Start from the pruning point or virtual parent instead of the saved checkpoint")
            .action(clap::ArgAction::SetTrue))
        .arg(Arg::new("initialize-db")
            .short('c')
            .long("initialize-db")
            .help("(Re-)initializes the database schema. Use with care.")
            .action(clap::ArgAction::SetTrue))
        .get_matches();

    let rpc_url = matches.get_one::<String>("rpc-url").unwrap().to_string();
    let network = matches.get_one::<String>("network").unwrap().to_lowercase();
    let database_url = matches.get_one::<String>("database-url").unwrap();
    let log_level = matches.get_one::<String>("log-level").unwrap();
    let from_pruning_point = matches.get_flag("from-pruning-point");
    let ignore_checkpoint = matches.get_flag("ignore-checkpoint");
    let initialize_db = matches.get_flag("initialize-db");

    env::set_var("RUST_LOG", log_level);
    env_logger::builder()
        .format_timestamp_millis()
        .init();

    let db_pool = Pool::builder()
        .connection_timeout(Duration::from_secs(10))
        .max_size(20)
        .build(ConnectionManager::<PgConnection>::new(database_url))
        .expect("Database pool FAILED");
    let db_con = &mut db_pool.get()
        .expect("Database connection FAILED");
    info!("Connection established");

    if initialize_db {
        info!("Initializing database");
        if let Err(e) = db_con.revert_all_migrations(MIGRATIONS) {
            info!("Unable to revert diesel migrations: {:?}", e);
        } else {
            info!("All migrations successfully reverted.");
        }
    }
    if initialize_db || network != "mainnet" {
        if let Err(e) = db_con.run_pending_migrations(MIGRATIONS) {
            info!("Unable to apply diesel migrations: {:?}", e);
        } else {
            info!("All migrations successfully applied.");
        }
    }

    let kaspad_client = connect_kaspad(rpc_url, network).await.expect("Kaspad connection FAILED");

    start_processing(ignore_checkpoint, from_pruning_point, db_pool, kaspad_client).await.expect("Unreachable");
}

async fn start_processing(ignore_checkpoint: bool,
                          from_pruning_point: bool,
                          db_pool: Pool<ConnectionManager<PgConnection>>,
                          kaspad_client: KaspaRpcClient) -> Result<(), ()> {
    let block_dag_info = kaspad_client.get_block_dag_info().await.expect("Error when invoking GetBlockDagInfo");
    let checkpoint_hash;
    if from_pruning_point {
        checkpoint_hash = block_dag_info.pruning_point_hash.to_string();
        info!("BlockDagInfo received: pruning_point={}", checkpoint_hash);
    } else {
        checkpoint_hash = block_dag_info.virtual_parent_hashes.get(0).unwrap().to_string();
        info!("BlockDagInfo received: virtual_parent_hash={}", checkpoint_hash);
    }
    let mut block_checkpoint_hash = checkpoint_hash.clone();
    let mut virtual_checkpoint_hash = checkpoint_hash.clone();
    if !ignore_checkpoint {
        let saved_block_checkpoint = load_block_checkpoint(db_pool.clone());
        if saved_block_checkpoint.is_some() {
            block_checkpoint_hash = saved_block_checkpoint.unwrap();
            info!("Loaded block_checkpoint={}", block_checkpoint_hash);
        } else {
            warn!("block_checkpoint_hash not found, using {}", if from_pruning_point { "pruning point" } else { "virtual_parent_hash" });
        }
        let saved_virtual_checkpoint = load_virtual_checkpoint(db_pool.clone());
        if saved_virtual_checkpoint.is_some() {
            virtual_checkpoint_hash = saved_virtual_checkpoint.unwrap();
            info!("Loaded virtual_checkpoint={}", virtual_checkpoint_hash);
        } else {
            warn!("virtual_checkpoint_hash not found, using {}", if from_pruning_point { "pruning point" } else { "virtual_parent_hash" });
        }
    }

    let rpc_blocks_queue = Arc::new(ArrayQueue::new(3_000));
    let rpc_transactions_queue = Arc::new(ArrayQueue::new(3_000));
    let db_blocks_queue = Arc::new(ArrayQueue::new(3_000));
    let db_transactions_queue = Arc::new(ArrayQueue::new(30_000));

    let mut tasks = vec![];
    tasks.push(task::spawn(fetch_blocks(block_checkpoint_hash, kaspad_client.clone(), rpc_blocks_queue.clone(), rpc_transactions_queue.clone())));
    tasks.push(task::spawn(process_blocks(rpc_blocks_queue.clone(), db_blocks_queue.clone())));
    tasks.push(task::spawn(process_transactions(rpc_transactions_queue.clone(), db_transactions_queue.clone(), db_pool.clone())));
    tasks.push(task::spawn(insert_blocks(db_blocks_queue.clone(), db_pool.clone())));
    tasks.push(task::spawn(insert_txs_ins_outs(db_transactions_queue.clone(), db_pool.clone())));
    tasks.push(task::spawn(process_virtual_chain(virtual_checkpoint_hash, kaspad_client.clone(), db_pool.clone())));

    for task in tasks {
        let _ = task.await.expect("Should not happen");
    }
    Ok(())
}
