use clap::Parser;
use crossbeam_queue::ArrayQueue;
use deadpool::managed::{Object, Pool};
use futures_util::future::try_join_all;
use kaspa_hashes::Hash as KaspaHash;
use kaspa_rpc_core::api::rpc::RpcApi;
use kaspa_wrpc_client::prelude::{NetworkId, NetworkType};
use log::{error, info, trace, warn};
use simply_kaspa_cli::cli_args::{CliArgs, CliDisable, CliEnable};
use simply_kaspa_cli::filter_config::FilterConfig;
use simply_kaspa_database::client::KaspaDbClient;
use simply_kaspa_database::tag_cache::TagCache;
use simply_kaspa_indexer::blocks::fetch_blocks::KaspaBlocksFetcher;
use simply_kaspa_indexer::blocks::process_blocks::process_blocks;
use simply_kaspa_indexer::checkpoint::{CheckpointBlock, CheckpointOrigin, process_checkpoints};
use simply_kaspa_indexer::prune::pruner;
use simply_kaspa_indexer::settings::Settings;
use simply_kaspa_indexer::transactions::process_transactions::process_transactions;
use simply_kaspa_indexer::utxo_import::utxo_set_importer::UtxoSetImporter;
use simply_kaspa_indexer::vars::load_block_checkpoint;
use simply_kaspa_indexer::virtual_chain::process_virtual_chain::process_virtual_chain;
use simply_kaspa_indexer::mapping::mapper::KaspaDbMapper;
use simply_kaspa_indexer::web::model::metrics::Metrics;
use simply_kaspa_indexer::web::web_server::WebServer;
use simply_kaspa_kaspad::manager::KaspadManager;
use simply_kaspa_signal::signal_handler::SignalHandler;
use std::env;
use std::str::FromStr;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::task;
use std::sync::RwLock as StdRwLock;

#[tokio::main]
async fn main() {
    println!();
    println!("**************************************************************");
    println!("******************** Simply Kaspa Indexer ********************");
    println!("--------------------------------------------------------------");
    println!("----- https://github.com/supertypo/simply-kaspa-indexer/ -----");
    println!("--------------------------------------------------------------");
    let cli_args = CliArgs::parse();
    configure_logging(&cli_args);

    trace!("{:?}", cli_args);
    if cli_args.batch_scale < 0.1 || cli_args.batch_scale > 10.0 {
        panic!("Invalid batch-scale");
    }
    if cli_args.batch_concurrency < 1 || cli_args.batch_concurrency > 10 {
        panic!("Invalid batch-concurrency");
    }
    info!("{} {}", env!("CARGO_PKG_NAME"), cli_args.version());

    let network_id = NetworkId::from_str(&cli_args.network).unwrap();
    let kaspad_manager = KaspadManager { network_id, rpc_url: cli_args.rpc_url.clone() };
    let kaspad_pool: Pool<KaspadManager> = Pool::builder(kaspad_manager).max_size(10).build().unwrap();

    let pool_size = cli_args.batch_concurrency as u32 * 10;
    let database = KaspaDbClient::new(&cli_args.database_url, pool_size).await.expect("Database connection FAILED");

    if cli_args.initialize_db {
        info!("Initializing database");
        database.drop_schema().await.expect("Unable to drop schema");
    }
    let seqcom_enabled = cli_args.is_enabled(CliEnable::SeqCom);
    database.create_schema(cli_args.upgrade_db, seqcom_enabled).await.expect("Unable to create schema");

    start_processing(cli_args, kaspad_pool, database).await;
}

async fn start_processing(cli_args: CliArgs, kaspad_pool: Pool<KaspadManager, Object<KaspadManager>>, database: KaspaDbClient) {
    let signal_handler = SignalHandler::new().spawn();

    let block_dag_info = loop {
        if signal_handler.is_shutdown() {
            return;
        }
        if let Ok(kaspad) = kaspad_pool.get().await
            && let Ok(bdi) = kaspad.get_block_dag_info().await
        {
            break bdi;
        }
        tokio::time::sleep(Duration::from_secs(5)).await;
    };

    let net_bps = match block_dag_info.network {
        NetworkId { network_type: NetworkType::Mainnet, suffix: None } => 10,
        _ => 10,
    };
    let net_tps_max = net_bps as u16 * 300;
    info!("Assuming {} block(s) per second for cache sizes", net_bps);

    if let Some(enable) = &cli_args.enable {
        info!("Enable functionality is set, the following functionality will be enabled: {:?}", enable);
    }
    if let Some(disable) = &cli_args.disable {
        info!("Disable functionality is set, the following functionality will be disabled: {:?}", disable);
    }
    if let Some(exclude_fields) = &cli_args.exclude_fields {
        info!("Exclude fields is set, the following fields will be excluded: {:?}", exclude_fields);
    }

    let mut utxo_set_import = cli_args.is_enabled(CliEnable::ForceUtxoImport);
    let checkpoint: KaspaHash;
    if let Some(ignore_checkpoint) = cli_args.ignore_checkpoint.clone() {
        warn!("Checkpoint ignored due to user request (-i). This might lead to inconsistencies.");
        if ignore_checkpoint == "p" {
            checkpoint = block_dag_info.pruning_point_hash;
            info!("Starting from pruning_point {}", checkpoint);
        } else if ignore_checkpoint == "v" {
            checkpoint = *block_dag_info.virtual_parent_hashes.first().expect("Virtual parent not found");
            info!("Starting from virtual_parent {}", checkpoint);
        } else {
            checkpoint = KaspaHash::from_str(ignore_checkpoint.as_str()).expect("Supplied block hash is invalid");
            info!("Starting from user supplied block {}", checkpoint);
        }
    } else if let Ok(saved_block_checkpoint) = load_block_checkpoint(&database).await {
        checkpoint = KaspaHash::from_str(saved_block_checkpoint.as_str()).expect("Saved checkpoint is invalid!");
        info!("Starting from checkpoint {}", checkpoint);
    } else if cli_args.is_disabled(CliDisable::InitialUtxoImport) {
        checkpoint = *block_dag_info.virtual_parent_hashes.first().expect("Virtual parent not found");
        warn!("Checkpoint not found, starting from virtual_parent {}", checkpoint);
    } else {
        utxo_set_import = true;
        checkpoint = block_dag_info.pruning_point_hash;
        warn!("Checkpoint not found, starting from pruning_point {}", checkpoint);
    }

    let checkpoint_block = match kaspad_pool.get().await.unwrap().get_block(checkpoint, false).await {
        Ok(block) => Some(CheckpointBlock {
            origin: CheckpointOrigin::Initial,
            hash: block.header.hash.into(),
            timestamp: block.header.timestamp,
            daa_score: block.header.daa_score,
            blue_score: block.header.blue_score,
        }),
        Err(_) => None,
    };

    let disable_vcp_wait_for_sync = cli_args.is_disabled(CliDisable::VcpWaitForSync) || utxo_set_import;

    let queue_capacity = (cli_args.batch_scale * 1000f64) as usize;
    let blocks_queue = Arc::new(ArrayQueue::new(queue_capacity));
    let txs_queue = Arc::new(ArrayQueue::new(queue_capacity));
    let checkpoint_queue = Arc::new(ArrayQueue::new(30000));

    // Bootstrap tag providers and build TagCache if filter config is provided
    let (tag_cache, filter_config) = if let Some(ref config_path) = cli_args.filter_config {
        match FilterConfig::from_file(config_path) {
            Ok(mut config) => {
                info!("Bootstrapping tag providers from filter config");

                // Build tries if --enable trie_matching
                if cli_args.is_enabled(CliEnable::TrieMatching) {
                    info!("Building prefix tries for fast matching (10+ rules)");
                    config.build_tries();
                    info!("Tries built: {} TXID nodes, {} payload nodes",
                          config.txid_trie.as_ref().map(|t| t.node_count()).unwrap_or(0),
                          config.payload_trie.as_ref().map(|t| t.node_count()).unwrap_or(0));
                }

                // Bootstrap tag providers to database and build cache
                let tag_cache = TagCache::new();
                for rule in &config.sorted_enabled_rules {
                    // Extract prefix from rule conditions for tag_provider record
                    let prefix = if let Some(ref txid_cond) = rule.conditions.txid {
                        txid_cond.prefix.clone()
                    } else if let Some(ref payload_conds) = rule.conditions.payload {
                        payload_conds.first().map(|c| c.prefix.clone()).unwrap_or_default()
                    } else {
                        String::new()
                    };

                    let module = rule.module.as_deref().unwrap_or("default");
                    match tag_cache.upsert_tag(
                        &rule.tag,
                        module,
                        &prefix,
                        rule.repository.as_deref(),
                        Some(&rule.name), // Use rule name as description
                        rule.category.as_deref(),
                        database.pool()
                    ).await {
                        Ok(tag_id) => {
                            info!("Tag provider '{}:{}' → tag_id {}", rule.tag, module, tag_id);
                        }
                        Err(e) => {
                            error!("Failed to upsert tag provider '{}:{}': {}", rule.tag, module, e);
                        }
                    }
                }

                info!("TagCache initialized with {} tag providers", tag_cache.len());

                // Wrap config in Arc<RwLock> for hot reload capability
                let filter_config = Arc::new(StdRwLock::new(config));

                (Some(tag_cache), Some(filter_config))
            }
            Err(e) => {
                error!("Failed to load filter config: {}", e);
                panic!("Invalid filter configuration");
            }
        }
    } else {
        (None, None)
    };

    let mapper = KaspaDbMapper::new(cli_args.clone(), tag_cache, filter_config.clone());

    // Spawn config reload handler if filter config is enabled
    if let Some(config_arc) = filter_config {
        let mut reload_rx = signal_handler.subscribe_reload();
        let config_path = cli_args.filter_config.clone().unwrap();
        let trie_enabled = cli_args.is_enabled(CliEnable::TrieMatching);

        task::spawn(async move {
            loop {
                match reload_rx.recv().await {
                    Ok(_) => {
                        info!("Reloading filter config from {}...", config_path);
                        match FilterConfig::from_file(&config_path) {
                            Ok(mut new_config) => {
                                // Build tries if enabled
                                if trie_enabled {
                                    new_config.build_tries();
                                    info!("Tries rebuilt: {} TXID nodes, {} payload nodes",
                                          new_config.txid_trie.as_ref().map(|t| t.node_count()).unwrap_or(0),
                                          new_config.payload_trie.as_ref().map(|t| t.node_count()).unwrap_or(0));
                                }

                                // Acquire write lock and replace config
                                match config_arc.write() {
                                    Ok(mut config) => {
                                        *config = new_config;
                                        info!("Filter config reloaded successfully with {} enabled rules",
                                              config.sorted_enabled_rules.len());
                                    }
                                    Err(e) => {
                                        error!("Failed to acquire write lock for config reload: {}", e);
                                    }
                                }
                            }
                            Err(e) => {
                                error!("Failed to reload filter config: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Reload channel error: {}", e);
                        break;
                    }
                }
            }
        });
    }

    let settings = Settings { cli_args: cli_args.clone(), net_bps, net_tps_max, checkpoint, disable_vcp_wait_for_sync };
    let start_vcp = Arc::new(AtomicBool::new(false));

    let mut metrics = Metrics::new(env!("CARGO_PKG_NAME").to_string(), cli_args.version(), cli_args.commit_id());
    let mut settings_clone = settings.clone();
    settings_clone.cli_args.rpc_url = settings_clone.cli_args.rpc_url.map(|_| "**hidden**".to_string());
    settings_clone.cli_args.p2p_url = settings_clone.cli_args.p2p_url.map(|_| "**hidden**".to_string());
    settings_clone.cli_args.database_url = "**hidden**".to_string();
    metrics.settings = Some(settings_clone);
    metrics.queues.blocks_capacity = blocks_queue.capacity() as u64;
    metrics.queues.transactions_capacity = txs_queue.capacity() as u64;
    metrics.checkpoint.origin = checkpoint_block.as_ref().map(|c| format!("{:?}", c.origin));
    metrics.checkpoint.block = checkpoint_block.map(|c| c.into());
    metrics.components.transaction_processor.enabled = !settings.cli_args.is_disabled(CliDisable::TransactionProcessing);
    metrics.components.virtual_chain_processor.enabled = !settings.cli_args.is_disabled(CliDisable::VirtualChainProcessing);
    metrics.components.virtual_chain_processor.only_blocks = settings.cli_args.is_disabled(CliDisable::TransactionAcceptance);
    let metrics = Arc::new(RwLock::new(metrics));

    let webserver =
        Arc::new(WebServer::new(settings.clone(), signal_handler.clone(), metrics.clone(), kaspad_pool.clone(), database.clone()));
    let webserver_task = task::spawn(async move { webserver.run().await.unwrap() });

    if utxo_set_import {
        let importer = UtxoSetImporter::new(
            cli_args.clone(),
            signal_handler.clone(),
            metrics.clone(),
            block_dag_info.pruning_point_hash,
            database.clone(),
        );
        importer.start().await;
    }

    let mut block_fetcher = KaspaBlocksFetcher::new(
        settings.clone(),
        signal_handler.clone(),
        metrics.clone(),
        kaspad_pool.clone(),
        blocks_queue.clone(),
        txs_queue.clone(),
    );

    let mut tasks = vec![
        webserver_task,
        task::spawn(async move { block_fetcher.start().await }),
        task::spawn(process_blocks(
            settings.clone(),
            signal_handler.clone(),
            metrics.clone(),
            start_vcp.clone(),
            blocks_queue.clone(),
            checkpoint_queue.clone(),
            database.clone(),
            mapper.clone(),
        )),
        task::spawn(process_checkpoints(
            settings.clone(),
            signal_handler.clone(),
            metrics.clone(),
            checkpoint_queue.clone(),
            database.clone(),
        )),
    ];
    if !settings.cli_args.is_disabled(CliDisable::TransactionProcessing) {
        tasks.push(task::spawn(process_transactions(
            settings.clone(),
            signal_handler.clone(),
            metrics.clone(),
            txs_queue.clone(),
            checkpoint_queue.clone(),
            database.clone(),
            mapper.clone(),
        )))
    }
    if !settings.cli_args.is_disabled(CliDisable::VirtualChainProcessing) {
        tasks.push(task::spawn(process_virtual_chain(
            settings.clone(),
            signal_handler.clone(),
            metrics.clone(),
            start_vcp.clone(),
            checkpoint_queue.clone(),
            kaspad_pool.clone(),
            database.clone(),
        )))
    }

    tasks.push(task::spawn(async move {
        if let Err(e) = pruner(cli_args.clone(), signal_handler.clone(), metrics.clone(), database.clone()).await {
            error!("Database pruner failed: {e}");
        }
    }));

    try_join_all(tasks).await.unwrap();
}

fn configure_logging(cli_args: &CliArgs) {
    env_logger::Builder::new()
        .target(env_logger::Target::Stdout)
        .format_target(false)
        .format_timestamp_millis()
        .parse_filters(&cli_args.log_level)
        .write_style(if cli_args.log_no_color { env_logger::WriteStyle::Never } else { env_logger::WriteStyle::Always })
        .init();
}
