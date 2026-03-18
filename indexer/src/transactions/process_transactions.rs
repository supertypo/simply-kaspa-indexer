use crate::blocks::fetch_blocks::TransactionData;
use crate::checkpoint::{CheckpointBlock, CheckpointOrigin};
use crate::settings::Settings;
use crate::web::model::metrics::Metrics;
use crossbeam_queue::ArrayQueue;
use futures_util::{StreamExt, stream};
use indexmap::IndexSet;
use kaspa_hashes::Hash as KaspaHash;
use log::{debug, info, trace, warn};
use moka::sync::Cache;
use simply_kaspa_cli::cli_args::{CliDisable, CliField};
use simply_kaspa_database::client::KaspaDbClient;
use simply_kaspa_database::models::address_transaction::AddressTransaction;
use simply_kaspa_database::models::block_transaction::BlockTransaction;
use simply_kaspa_database::models::script_transaction::ScriptTransaction;
use simply_kaspa_database::models::transaction::Transaction;
use simply_kaspa_mapping::mapper::KaspaDbMapper;
use simply_kaspa_signal::signal_handler::SignalHandler;
use std::cmp::min;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tokio::task;
use tokio::time::sleep;

pub async fn process_transactions(
    settings: Settings,
    signal_handler: SignalHandler,
    metrics: Arc<RwLock<Metrics>>,
    start_vcp: Arc<AtomicBool>,
    txs_queue: Arc<ArrayQueue<TransactionData>>,
    checkpoint_queue: Arc<ArrayQueue<CheckpointBlock>>,
    database: KaspaDbClient,
    mapper: KaspaDbMapper,
) {
    let ttl = settings.cli_args.cache_ttl;
    let cache_size = settings.net_tps_max as u64 * ttl * 2;
    let tx_id_cache: Cache<KaspaHash, ()> = Cache::builder().time_to_live(Duration::from_secs(ttl)).max_capacity(cache_size).build();

    let batch_scale = settings.cli_args.batch_scale;
    let batch_concurrency = settings.cli_args.batch_concurrency;
    let batch_size = (5000f64 * batch_scale) as usize;

    let disable_transactions = settings.cli_args.is_disabled(CliDisable::TransactionsTable);
    let disable_blocks_transactions = settings.cli_args.is_disabled(CliDisable::BlocksTransactionsTable);
    let disable_address_transactions = settings.cli_args.is_disabled(CliDisable::AddressesTransactionsTable);
    let disable_rejected_transactions = settings.cli_args.is_disabled(CliDisable::RejectedTransactions);
    let disable_rejected_non_cb_transactions = settings.cli_args.is_disabled(CliDisable::RejectedNonCbTransactions);
    let exclude_tx_out_script_public_key_address = settings.cli_args.is_excluded(CliField::TxOutScriptPublicKeyAddress);
    let exclude_tx_out_script_public_key = settings.cli_args.is_excluded(CliField::TxOutScriptPublicKey);

    let mut transactions = vec![];
    let mut block_tx = vec![];
    let mut tx_address_transactions: IndexSet<_> = IndexSet::new();
    let mut tx_script_transactions: IndexSet<_> = IndexSet::new();
    let mut checkpoint_blocks = vec![];
    let mut last_commit_time = Instant::now();

    if !disable_address_transactions {
        if !exclude_tx_out_script_public_key_address {
            info!("Using addresses_transactions for address transaction mapping");
        } else if !exclude_tx_out_script_public_key {
            info!("Using scripts_transactions for address transaction mapping");
        } else {
            info!("Address transaction mapping disabled");
        }
    } else {
        info!("Address transaction mapping disabled");
    }

    while !signal_handler.is_shutdown() {
        if let Some(transaction_data) = txs_queue.pop() {
            checkpoint_blocks.push(CheckpointBlock {
                origin: CheckpointOrigin::Transactions,
                hash: transaction_data.block_hash.into(),
                timestamp: transaction_data.block_timestamp,
                daa_score: transaction_data.block_daa_score,
                blue_score: transaction_data.block_blue_score,
            });
            for transaction in transaction_data.transactions {
                if !disable_rejected_transactions && (!disable_rejected_non_cb_transactions || transaction.subnetwork_id.is_builtin()) {
                    let transaction_id = transaction.verbose_data.as_ref().unwrap().transaction_id;
                    if tx_id_cache.contains_key(&transaction_id) {
                        trace!("Known transaction_id {}, keeping block relation only", transaction_id);
                    } else {
                        if !disable_transactions {
                            transactions.push(mapper.map_transaction(&transaction));
                        }
                        if !disable_address_transactions {
                            if !exclude_tx_out_script_public_key_address {
                                tx_address_transactions.extend(mapper.map_transaction_outputs_address(&transaction));
                            } else if !exclude_tx_out_script_public_key {
                                tx_script_transactions.extend(mapper.map_transaction_outputs_script(&transaction));
                            }
                        }
                        tx_id_cache.insert(transaction_id, ());
                    }
                }
                block_tx.push(mapper.map_block_transaction(&transaction));
            }

            if block_tx.len() >= batch_size || (!block_tx.is_empty() && Instant::now().duration_since(last_commit_time).as_secs() > 2)
            {
                if !disable_rejected_transactions && start_vcp.load(Ordering::Relaxed) {
                    loop {
                        if let Some(vcp) = &metrics.read().await.components.virtual_chain_processor.last_block {
                            if vcp.daa_score.saturating_sub(checkpoint_blocks.last().unwrap().daa_score) >= 3 * settings.net_bps as u64
                            {
                                break;
                            }
                        }
                        debug!("Transaction processor is waiting for virtual chain processor to catch up...");
                        sleep(Duration::from_millis(1000)).await;
                        if signal_handler.is_shutdown() {
                            return;
                        }
                    }
                }
                let start_commit_time = Instant::now();
                let transactions_len = transactions.len();

                let blocks_txs_handle = if !disable_blocks_transactions {
                    task::spawn(insert_block_txs(batch_scale, batch_concurrency, block_tx, database.clone()))
                } else {
                    task::spawn(async { 0 })
                };
                let tx_handle = task::spawn(insert_txs(batch_scale, batch_concurrency, transactions, false, database.clone()));
                let tx_addr_handle = if !exclude_tx_out_script_public_key_address {
                    task::spawn(insert_tx_addr(
                        batch_scale,
                        batch_concurrency,
                        tx_address_transactions.into_iter().collect(),
                        database.clone(),
                    ))
                } else {
                    task::spawn(insert_tx_script(
                        batch_scale,
                        batch_concurrency,
                        tx_script_transactions.into_iter().collect(),
                        database.clone(),
                    ))
                };
                let rows_affected_tx = tx_handle.await.unwrap();
                let rows_affected_block_tx = blocks_txs_handle.await.unwrap();
                let rows_affected_tx_addr = tx_addr_handle.await.unwrap();

                let last_checkpoint = checkpoint_blocks.last().unwrap().clone();
                let last_block_time = last_checkpoint.timestamp;

                let mut metrics = metrics.write().await;
                metrics.components.transaction_processor.update_last_block(last_checkpoint.into());
                drop(metrics);

                for checkpoint_block in checkpoint_blocks {
                    while checkpoint_queue.push(checkpoint_block.clone()).is_err() {
                        warn!("Checkpoint queue is full");
                        sleep(Duration::from_secs(1)).await;
                    }
                }
                let commit_time = Instant::now().duration_since(start_commit_time).as_millis();
                let tps = transactions_len as f64 / commit_time as f64 * 1000f64;
                info!(
                    "Committed {} new txs in {}ms ({:.1} tps, {} blk_tx, {} adr_tx). Last tx: {}",
                    rows_affected_tx,
                    commit_time,
                    tps,
                    rows_affected_block_tx,
                    rows_affected_tx_addr,
                    chrono::DateTime::from_timestamp_millis(last_block_time as i64 / 1000 * 1000).unwrap()
                );
                transactions = vec![];
                block_tx = vec![];
                tx_address_transactions = IndexSet::new();
                tx_script_transactions = IndexSet::new();
                checkpoint_blocks = vec![];
                last_commit_time = Instant::now();
            }
        } else {
            sleep(Duration::from_millis(100)).await;
        }
    }
}

pub async fn insert_txs(
    batch_scale: f64,
    batch_concurrency: i8,
    values: Vec<Transaction>,
    upsert_inputs: bool,
    database: KaspaDbClient,
) -> u64 {
    let batch_size = min((250f64 * batch_scale) as u16, 7000) as usize;
    let key = "transactions";
    let start_time = Instant::now();
    debug!("Processing {} {}", values.len(), key);
    let mut values = values;
    values.sort_by(|a, b| a.transaction_id.cmp(&b.transaction_id));
    let chunks: Vec<Vec<_>> = values.chunks(batch_size).map(|c| c.to_vec()).collect();
    let rows_affected = stream::iter(chunks.into_iter().map(|chunk| {
        let db = database.clone();
        async move { db.insert_transactions(&chunk, upsert_inputs).await.unwrap_or_else(|e| panic!("Insert {key} FAILED: {e}")) }
    }))
    .buffer_unordered(batch_concurrency as usize)
    .fold(0, |acc, rows| async move { acc + rows })
    .await;
    debug!("Committed {} {} in {}ms", rows_affected, key, start_time.elapsed().as_millis());
    rows_affected
}

pub async fn insert_tx_addr(batch_scale: f64, batch_concurrency: i8, values: Vec<AddressTransaction>, database: KaspaDbClient) -> u64 {
    let batch_size = min((650f64 * batch_scale) as u16, 21000) as usize;
    let key = "addresses_transactions";
    let start_time = Instant::now();
    debug!("Processing {} {}", values.len(), key);
    let mut values = values;
    values.sort_by(|a, b| a.address.cmp(&b.address).then(a.transaction_id.cmp(&b.transaction_id)));
    let chunks: Vec<Vec<_>> = values.chunks(batch_size).map(|c| c.to_vec()).collect();
    let rows_affected = stream::iter(chunks.into_iter().map(|chunk| {
        let db = database.clone();
        async move { db.insert_address_transactions(&chunk).await.unwrap_or_else(|e| panic!("Insert {key} FAILED: {e}")) }
    }))
    .buffer_unordered(batch_concurrency as usize)
    .fold(0, |acc, rows| async move { acc + rows })
    .await;
    debug!("Committed {} {} in {}ms", rows_affected, key, start_time.elapsed().as_millis());
    rows_affected
}

pub async fn insert_tx_script(
    batch_scale: f64,
    batch_concurrency: i8,
    values: Vec<ScriptTransaction>,
    database: KaspaDbClient,
) -> u64 {
    let batch_size = min((800f64 * batch_scale) as u16, 21000) as usize;
    let key = "scripts_transactions";
    let start_time = Instant::now();
    debug!("Processing {} {}", values.len(), key);
    let mut values = values;
    values.sort_by(|a, b| a.script_public_key.cmp(&b.script_public_key).then(a.transaction_id.cmp(&b.transaction_id)));
    let chunks: Vec<Vec<_>> = values.chunks(batch_size).map(|c| c.to_vec()).collect();
    let rows_affected = stream::iter(chunks.into_iter().map(|chunk| {
        let db = database.clone();
        async move { db.insert_script_transactions(&chunk).await.unwrap_or_else(|e| panic!("Insert {key} FAILED: {e}")) }
    }))
    .buffer_unordered(batch_concurrency as usize)
    .fold(0, |acc, rows| async move { acc + rows })
    .await;
    debug!("Committed {} {} in {}ms", rows_affected, key, start_time.elapsed().as_millis());
    rows_affected
}

async fn insert_block_txs(batch_scale: f64, batch_concurrency: i8, values: Vec<BlockTransaction>, database: KaspaDbClient) -> u64 {
    let batch_size = min((1300f64 * batch_scale) as u16, 32000) as usize;
    let key = "block/transaction mappings";
    let start_time = Instant::now();
    debug!("Processing {} {}", values.len(), key);
    let mut values = values;
    values.sort_by(|a, b| a.block_hash.cmp(&b.block_hash).then(a.transaction_id.cmp(&b.transaction_id)));
    let chunks: Vec<Vec<_>> = values.chunks(batch_size).map(|c| c.to_vec()).collect();
    let rows_affected = stream::iter(chunks.into_iter().map(|chunk| {
        let db = database.clone();
        async move { db.insert_block_transactions(&chunk).await.unwrap_or_else(|e| panic!("Insert {key} FAILED: {e}")) }
    }))
    .buffer_unordered(batch_concurrency as usize)
    .fold(0, |acc, rows| async move { acc + rows })
    .await;
    debug!("Committed {} {} in {}ms", rows_affected, key, start_time.elapsed().as_millis());
    rows_affected
}
