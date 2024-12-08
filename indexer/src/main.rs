use crossbeam_queue::ArrayQueue;
use deadpool::managed::{Object, Pool};
use futures_util::future::try_join_all;
use kaspa_hashes::Hash as KaspaHash;
use kaspa_rpc_core::api::rpc::RpcApi;
use kaspa_wrpc_client::prelude::NetworkId;
use log::{info, warn};
use std::env;
use std::str::FromStr;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;
use tokio::task;

use simply_kaspa_database::client::client::KaspaDbClient;
use simply_kaspa_indexer::blocks::fetch_blocks::KaspaBlocksFetcher;
use simply_kaspa_indexer::blocks::process_blocks::process_blocks;
use simply_kaspa_indexer::cli::cli_args::{get_cli_args, CliArgs};
use simply_kaspa_indexer::settings::settings::Settings;
use simply_kaspa_indexer::signal::signal_handler::notify_on_signals;
use simply_kaspa_indexer::transactions::process_transactions::process_transactions;
use simply_kaspa_indexer::vars::vars::load_block_checkpoint;
use simply_kaspa_indexer::virtual_chain::process_virtual_chain::process_virtual_chain;
use simply_kaspa_kaspad::pool::manager::KaspadManager;
use simply_kaspa_mapping::mapper::mapper::KaspaDbMapper;

#[tokio::main]
async fn main() {
    println!();
    println!("**************************************************************");
    println!("******************** Simply Kaspa Indexer ********************");
    println!("**************************************************************");
    println!("https://hub.docker.com/r/supertypo/simply-kaspa-indexer");
    println!();
    let cli_args = get_cli_args();

    env::set_var("RUST_LOG", &cli_args.log_level);
    env::set_var("RUST_LOG_STYLE", if cli_args.log_no_color { "never" } else { "always" });
    env_logger::builder().target(env_logger::Target::Stdout).format_target(false).format_timestamp_millis().init();

    if cli_args.batch_scale < 0.1 || cli_args.batch_scale > 10.0 {
        panic!("Invalid batch-scale");
    }

    let network_id = NetworkId::from_str(&cli_args.network).unwrap();
    let kaspad_manager = KaspadManager { network_id, rpc_url: cli_args.rpc_url.clone() };
    let kaspad_pool: Pool<KaspadManager> = Pool::builder(kaspad_manager).max_size(10).build().unwrap();

    let database = KaspaDbClient::new(&cli_args.database_url).await.expect("Database connection FAILED");

    if cli_args.initialize_db {
        info!("Initializing database");
        database.drop_schema().await.expect("Unable to drop schema");
    }
    database.create_schema(cli_args.upgrade_db).await.expect("Unable to create schema");

    start_processing(cli_args, kaspad_pool, database).await.expect("Unreachable");
}

async fn start_processing(
    cli_args: CliArgs,
    kaspad_pool: Pool<KaspadManager, Object<KaspadManager>>,
    database: KaspaDbClient,
) -> Result<(), ()> {
    let mut block_dag_info = None;
    while block_dag_info.is_none() {
        if let Ok(kaspad) = kaspad_pool.get().await {
            if let Ok(bdi) = kaspad.get_block_dag_info().await {
                block_dag_info = Some(bdi);
            }
        }
        if block_dag_info.is_none() {
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    }
    let block_dag_info = block_dag_info.unwrap();
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
    if cli_args.skip_resolving_addresses {
        warn!("Skip resolving addresses is enabled")
    }
    if cli_args.extra_data {
        info!("Extra data is enabled")
    }

    let run = Arc::new(AtomicBool::new(true));
    task::spawn(notify_on_signals(run.clone()));

    let base_buffer_blocks = 1000f64;
    let base_buffer_txs = base_buffer_blocks * 20f64;
    let blocks_queue = Arc::new(ArrayQueue::new((base_buffer_blocks * cli_args.batch_scale) as usize));
    let txs_queue = Arc::new(ArrayQueue::new((base_buffer_txs * cli_args.batch_scale) as usize));

    let mapper = KaspaDbMapper::new(cli_args.extra_data);

    let settings = Settings { cli_args: cli_args.clone(), net_bps, net_tps_max, checkpoint };
    let start_vcp = Arc::new(AtomicBool::new(false));

    let mut block_fetcher =
        KaspaBlocksFetcher::new(settings.clone(), run.clone(), kaspad_pool.clone(), blocks_queue.clone(), txs_queue.clone());

    let tasks = vec![
        task::spawn(async move { block_fetcher.start().await }),
        task::spawn(process_blocks(
            settings.clone(),
            run.clone(),
            start_vcp.clone(),
            blocks_queue.clone(),
            database.clone(),
            mapper.clone(),
        )),
        task::spawn(process_transactions(settings.clone(), run.clone(), txs_queue.clone(), database.clone(), mapper.clone())),
        task::spawn(process_virtual_chain(settings.clone(), run.clone(), start_vcp.clone(), kaspad_pool.clone(), database.clone())),
    ];
    try_join_all(tasks).await.unwrap();
    Ok(())
}
