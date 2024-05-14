extern crate diesel;

use std::str::FromStr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use std::{env, process};

use clap::{crate_description, crate_name, Arg, Command};
use crossbeam_queue::ArrayQueue;
use diesel::pg::PgConnection;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use futures_util::future::try_join_all;
use kaspa_hashes::Hash;
use kaspa_rpc_core::api::rpc::RpcApi;
use kaspa_wrpc_client::KaspaRpcClient;
use log::{debug, info, warn};
use regex::Regex;
use signal_hook::consts::{SIGINT, SIGQUIT, SIGTERM};
use signal_hook::iterator::Signals;
use signal_hook::low_level::signal_name;
use sqlx::postgres::PgPoolOptions;
use sqlx::Postgres;
use tokio::task;

use kaspa_db_filler_ng::blocks::fetch_blocks::fetch_blocks;
use kaspa_db_filler_ng::blocks::insert_blocks::insert_blocks;
use kaspa_db_filler_ng::blocks::process_blocks::process_blocks;
use kaspa_db_filler_ng::kaspad::client::connect_kaspad;
use kaspa_db_filler_ng::transactions::insert_transactions::insert_txs_ins_outs;
use kaspa_db_filler_ng::transactions::process_transactions::process_transactions;
use kaspa_db_filler_ng::vars::vars::load_block_checkpoint;
use kaspa_db_filler_ng::virtual_chain::process_virtual_chain::process_virtual_chain;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!();

#[tokio::main]
async fn main() {
    println!();
    println!("******************************************************");
    println!("***************** Kaspa DB Filler NG *****************");
    println!("******************************************************");
    println!("https://hub.docker.com/r/supertypo/kaspa-db-filler-ng");
    println!();
    let matches = Command::new(crate_name!())
        .about(crate_description!())
        .arg(
            Arg::new("rpc-url")
                .short('s')
                .long("rpc-url")
                .help("The url to a kaspad instance")
                .default_value("ws://127.0.0.1:17110")
                .action(clap::ArgAction::Set),
        )
        .arg(
            Arg::new("network")
                .short('n')
                .long("network")
                .help("The network type and suffix, e.g. 'testnet-11'")
                .default_value("mainnet")
                .action(clap::ArgAction::Set),
        )
        .arg(
            Arg::new("database-url")
                .short('d')
                .long("database-url")
                .help("The url to a PostgreSQL instance")
                .default_value("postgres://postgres:postgres@localhost:5432/postgres")
                .action(clap::ArgAction::Set),
        )
        .arg(
            Arg::new("log-level")
                .short('l')
                .long("log-level")
                .help("error, warn, info, debug, trace, off")
                .default_value("info")
                .action(clap::ArgAction::Set),
        )
        .arg(Arg::new("log-no-color").long("no-color").help("Disable colored output").action(clap::ArgAction::SetTrue))
        .arg(
            Arg::new("batch-scale")
                .short('b')
                .long("batch-scale")
                .help("Batch size factor [0.1-10]. Affects database batch sizes (as well as internal queue sizes)")
                .default_value("1.0")
                .action(clap::ArgAction::Set)
                .value_parser(clap::value_parser!(f64)),
        )
        .arg(
            Arg::new("ignore-checkpoint")
                .short('i')
                .long("ignore-checkpoint")
                .help("Ignore checkpoint and start from a specified block hash, 'p' for pruning point or 'v' for virtual")
                .action(clap::ArgAction::Set),
        )
        .arg(
            Arg::new("initialize-db")
                .short('c')
                .long("initialize-db")
                .help("(Re-)initializes the database schema. Use with care")
                .action(clap::ArgAction::SetTrue),
        )
        .get_matches();

    let rpc_url = matches.get_one::<String>("rpc-url").unwrap().to_string();
    let network = matches.get_one::<String>("network").unwrap().to_lowercase();
    let database_url = matches.get_one::<String>("database-url").unwrap();
    let log_level = matches.get_one::<String>("log-level").unwrap().to_lowercase();
    let log_no_color = matches.get_flag("log-no-color");
    let batch_scale = matches.get_one::<f64>("batch-scale").unwrap().to_owned();
    let ignore_checkpoint = matches.get_one::<String>("ignore-checkpoint").map(|i| i.to_lowercase());
    let initialize_db = matches.get_flag("initialize-db");

    env::set_var("RUST_LOG", log_level);
    env::set_var("RUST_LOG_STYLE", if log_no_color { "never" } else { "always" });
    env_logger::builder().target(env_logger::Target::Stdout).format_target(false).format_timestamp_millis().init();

    if batch_scale < 0.1 || batch_scale > 10.0 {
        panic!("Invalid batch-scale");
    }

    let db_url_cleaned = Regex::new(r"(postgres://postgres:)[^@]+(@)").unwrap().replace(database_url, "$1$2");
    debug!("Connecting to PostgreSQL {}", db_url_cleaned);
    let db_pool = Pool::builder()
        .connection_timeout(Duration::from_secs(10))
        .max_size(20)
        .build(ConnectionManager::<PgConnection>::new(database_url))
        .expect("Database pool FAILED");
    let db_con = &mut db_pool.get().expect("Database connection FAILED");
    info!("Connected to PostgreSQL {}", db_url_cleaned);

    let sqlx_pool = PgPoolOptions::new()
        .acquire_timeout(Duration::from_secs(10))
        .max_connections(10)
        .connect(database_url)
        .await
        .expect("SQLX database pool FAILED");

    if initialize_db {
        info!("Initializing database");
        if let Err(e) = db_con.revert_all_migrations(MIGRATIONS) {
            info!("Unable to revert diesel migrations: {:?}", e);
        } else {
            info!("All migrations successfully reverted.");
        }
    }
    if let Err(e) = db_con.run_pending_migrations(MIGRATIONS) {
        info!("Unable to apply diesel migrations: {:?}", e);
    } else {
        info!("All migrations successfully applied.");
    }

    let kaspad = connect_kaspad(rpc_url, network).await.expect("Kaspad connection FAILED");

    start_processing(batch_scale, ignore_checkpoint, db_pool, sqlx_pool, kaspad).await.expect("Unreachable");
}

async fn start_processing(
    batch_scale: f64,
    ignore_checkpoint: Option<String>,
    db_pool: Pool<ConnectionManager<PgConnection>>,
    sqlx_pool: sqlx::Pool<Postgres>,
    kaspad: KaspaRpcClient,
) -> Result<(), ()> {
    let block_dag_info = kaspad.get_block_dag_info().await.expect("Error when invoking GetBlockDagInfo");
    let checkpoint: Hash;

    if let Some(ignore_checkpoint) = ignore_checkpoint {
        warn!("Checkpoint ignored due to user request (-i). This might lead to inconsistencies.");
        if ignore_checkpoint == "p" {
            checkpoint = block_dag_info.pruning_point_hash;
            info!("Starting from pruning_point {}", checkpoint);
        } else if ignore_checkpoint == "v" {
            checkpoint = *block_dag_info.virtual_parent_hashes.get(0).unwrap();
            info!("Starting from virtual_parent {}", checkpoint);
        } else {
            checkpoint = Hash::from_str(ignore_checkpoint.as_str()).expect("Supplied block hash is invalid");
            info!("Starting from user supplied block {}", checkpoint);
        }
    } else {
        if let Some(saved_block_checkpoint) = load_block_checkpoint(db_pool.clone()) {
            checkpoint = Hash::from_str(saved_block_checkpoint.as_str()).expect("Saved checkpoint is invalid!");
            info!("Starting from checkpoint {}", checkpoint);
        } else {
            checkpoint = block_dag_info.pruning_point_hash;
            warn!("Checkpoint not found, starting from pruning point {}", checkpoint);
        }
    }

    let run = Arc::new(AtomicBool::new(true));
    let running_clone = run.clone();
    std::thread::spawn(move || {
        for signal in Signals::new(&[SIGINT, SIGQUIT, SIGTERM]).expect("Signal handler setup FAILED").forever() {
            let signal_name = signal_name(signal).unwrap_or("UNKNOWN");
            if !running_clone.load(Ordering::Relaxed) {
                warn!("{} received, terminating...", signal_name);
                process::exit(1);
            }
            running_clone.store(false, Ordering::Relaxed);
            warn!("{} received, stopping... (repeat for forced close)", signal_name);
        }
    });

    let base_buffer = 3000f64;
    let rpc_blocks_queue = Arc::new(ArrayQueue::new((base_buffer * batch_scale) as usize));
    let rpc_txs_queue = Arc::new(ArrayQueue::new((base_buffer * batch_scale) as usize));
    let db_blocks_queue = Arc::new(ArrayQueue::new((base_buffer * batch_scale) as usize));
    let db_txs_queue = Arc::new(ArrayQueue::new((base_buffer * batch_scale) as usize));

    let start_vcp = Arc::new(AtomicBool::new(false));
    let tasks = vec![
        task::spawn(fetch_blocks(run.clone(), checkpoint, kaspad.clone(), rpc_blocks_queue.clone(), rpc_txs_queue.clone())),
        task::spawn(process_blocks(run.clone(), rpc_blocks_queue.clone(), db_blocks_queue.clone())),
        task::spawn(process_transactions(run.clone(), rpc_txs_queue.clone(), db_txs_queue.clone(), db_pool.clone())),
        task::spawn(insert_blocks(run.clone(), batch_scale, start_vcp.clone(), db_blocks_queue.clone(), sqlx_pool.clone())),
        task::spawn(insert_txs_ins_outs(run.clone(), batch_scale, db_txs_queue.clone(), db_pool.clone())),
        task::spawn(process_virtual_chain(run.clone(), start_vcp.clone(), batch_scale, checkpoint, kaspad.clone(), db_pool.clone())),
    ];
    try_join_all(tasks).await.unwrap();
    Ok(())
}
