use std::env;
use std::str::FromStr;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use crossbeam_queue::ArrayQueue;
use futures_util::future::try_join_all;
use kaspa_database::client::client::KaspaDbClient;
use kaspa_db_filler_ng::blocks::fetch_blocks::fetch_blocks;
use kaspa_db_filler_ng::blocks::insert_blocks::insert_blocks_parents;
use kaspa_db_filler_ng::blocks::process_blocks::process_blocks;
use kaspa_db_filler_ng::cli::cli_args::{get_cli_args, CliArgs};
use kaspa_db_filler_ng::kaspad::client::connect_kaspad;
use kaspa_db_filler_ng::settings::settings::Settings;
use kaspa_db_filler_ng::signal::signal_handler::notify_on_signals;
use kaspa_db_filler_ng::transactions::insert_transactions::insert_txs_ins_outs;
use kaspa_db_filler_ng::transactions::process_transactions::process_transactions;
use kaspa_db_filler_ng::vars::vars::load_block_checkpoint;
use kaspa_db_filler_ng::virtual_chain::process_virtual_chain::process_virtual_chain;
use kaspa_hashes::Hash as KaspaHash;
use kaspa_rpc_core::api::rpc::RpcApi;
use kaspa_wrpc_client::KaspaRpcClient;
use log::{info, warn};
use tokio::task;

#[tokio::main]
async fn main() {
    println!();
    println!("**************************************************************");
    println!("********************* Kaspa DB Filler NG *********************");
    println!("**************************************************************");
    println!("https://hub.docker.com/r/supertypo/kaspa-db-filler-ng");
    println!();
    let cli_args = get_cli_args();

    env::set_var("RUST_LOG", &cli_args.log_level);
    env::set_var("RUST_LOG_STYLE", if cli_args.log_no_color { "never" } else { "always" });
    env_logger::builder().target(env_logger::Target::Stdout).format_target(false).format_timestamp_millis().init();

    if cli_args.batch_scale < 0.1 || cli_args.batch_scale > 10.0 {
        panic!("Invalid batch-scale");
    }

    let database = KaspaDbClient::new(&cli_args.database_url).await.expect("Database connection FAILED");

    if cli_args.initialize_db {
        info!("Initializing database");
        database.drop_schema().await.expect("Unable to drop schema");
    }
    database.create_schema(cli_args.upgrade_db).await.expect("Unable to create schema");

    let kaspad = connect_kaspad(&cli_args.rpc_url, &cli_args.network).await.expect("Kaspad connection FAILED");

    start_processing(cli_args, kaspad, database).await.expect("Unreachable");
}

async fn start_processing(cli_args: CliArgs, kaspad: KaspaRpcClient, database: KaspaDbClient) -> Result<(), ()> {
    let block_dag_info = kaspad.get_block_dag_info().await.expect("Error when invoking GetBlockDagInfo");
    let net_bps = if block_dag_info.network.suffix.filter(|s| s.to_owned() == 11).is_some() { 10 } else { 1 };
    let net_tps_max = net_bps as u16 * 300;
    info!("Assuming {} block(s) per second for cache sizes", net_bps);

    let checkpoint: KaspaHash;
    if let Some(ignore_checkpoint) = cli_args.ignore_checkpoint.clone() {
        warn!("Checkpoint ignored due to user request (-i). This might lead to inconsistencies.");
        if ignore_checkpoint == "p" {
            checkpoint = block_dag_info.pruning_point_hash;
            info!("Starting from pruning_point {}", checkpoint);
        } else if ignore_checkpoint == "v" {
            checkpoint = *block_dag_info.virtual_parent_hashes.get(0).expect("Virtual parent not found");
            info!("Starting from virtual_parent {}", checkpoint);
        } else {
            checkpoint = KaspaHash::from_str(ignore_checkpoint.as_str()).expect("Supplied block hash is invalid");
            info!("Starting from user supplied block {}", checkpoint);
        }
    } else {
        if let Ok(saved_block_checkpoint) = load_block_checkpoint(&database).await {
            checkpoint = KaspaHash::from_str(saved_block_checkpoint.as_str()).expect("Saved checkpoint is invalid!");
            info!("Starting from checkpoint {}", checkpoint);
        } else {
            checkpoint = *block_dag_info.virtual_parent_hashes.get(0).expect("Virtual parent not found");
            warn!("Checkpoint not found, starting from virtual_parent {}", checkpoint);
        }
    }
    if cli_args.vcp_before_synced {
        warn!("VCP before synced is enabled. Starting VCP as soon as the filler has caught up to the previous run")
    }
    if cli_args.skip_input_resolve {
        warn!("Skip resolving inputs to addresses is enabled")
    }
    if cli_args.extra_data {
        info!("Extra data is enabled")
    }

    let run = Arc::new(AtomicBool::new(true));
    task::spawn(notify_on_signals(run.clone()));

    let base_buffer = 3000f64;
    let rpc_blocks_queue = Arc::new(ArrayQueue::new((base_buffer * cli_args.batch_scale) as usize));
    let rpc_txs_queue = Arc::new(ArrayQueue::new((base_buffer * cli_args.batch_scale) as usize));
    let db_blocks_queue = Arc::new(ArrayQueue::new((base_buffer * cli_args.batch_scale) as usize));
    let db_txs_queue = Arc::new(ArrayQueue::new((base_buffer * cli_args.batch_scale) as usize));

    let settings = Settings { cli_args: cli_args.clone(), net_bps, net_tps_max, checkpoint };
    let start_vcp = Arc::new(AtomicBool::new(false));
    let tasks = vec![
        task::spawn(fetch_blocks(settings.clone(), run.clone(), kaspad.clone(), rpc_blocks_queue.clone(), rpc_txs_queue.clone())),
        task::spawn(process_blocks(run.clone(), rpc_blocks_queue.clone(), db_blocks_queue.clone())),
        task::spawn(process_transactions(
            settings.clone(),
            run.clone(),
            rpc_txs_queue.clone(),
            db_txs_queue.clone(),
            database.clone(),
        )),
        task::spawn(insert_blocks_parents(
            settings.clone(),
            run.clone(),
            start_vcp.clone(),
            db_blocks_queue.clone(),
            database.clone(),
        )),
        task::spawn(insert_txs_ins_outs(settings.clone(), run.clone(), db_txs_queue.clone(), database.clone())),
        task::spawn(process_virtual_chain(settings.clone(), run.clone(), start_vcp.clone(), kaspad.clone(), database.clone())),
    ];
    try_join_all(tasks).await.unwrap();
    Ok(())
}
