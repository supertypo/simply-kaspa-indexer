use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::settings::Settings;
use crate::web::model::metrics::{Metrics, MetricsBlock};
use chrono::{DateTime, Utc};
use crossbeam_queue::ArrayQueue;
use deadpool::managed::{Object, Pool};
use kaspa_hashes::Hash as KaspaHash;
use kaspa_rpc_core::api::rpc::RpcApi;
use kaspa_rpc_core::{RpcBlock, RpcTransaction};
use log::{debug, trace, warn};
use log::{error, info};
use moka::sync::Cache;
use simply_kaspa_cli::cli_args::CliDisable;
use simply_kaspa_kaspad::manager::KaspadManager;
use simply_kaspa_signal::signal_handler::SignalHandler;
use tokio::sync::RwLock;
use tokio::time::sleep;

#[derive(Debug)]
pub struct BlockData {
    pub block: RpcBlock,
    pub synced: bool,
}

#[derive(Debug)]
pub struct TransactionData {
    pub transactions: Vec<RpcTransaction>,
    pub block_hash: KaspaHash,
    pub block_timestamp: u64,
    pub block_daa_score: u64,
    pub block_blue_score: u64,
}

pub struct KaspaBlocksFetcher {
    disable_transaction_processing: bool,
    poll_interval: Duration,
    signal_handler: SignalHandler,
    metrics: Arc<RwLock<Metrics>>,
    kaspad_pool: Pool<KaspadManager, Object<KaspadManager>>,
    blocks_queue: Arc<ArrayQueue<BlockData>>,
    txs_queue: Arc<ArrayQueue<TransactionData>>,
    low_hash: KaspaHash,
    last_sync_check: Instant,
    synced: bool,
    lag_count: i32,
    tip_hashes: HashSet<KaspaHash>,
    block_cache: Cache<KaspaHash, ()>,
}

impl KaspaBlocksFetcher {
    const SYNC_CHECK_INTERVAL: Duration = Duration::from_secs(30);

    pub fn new(
        settings: Settings,
        signal_handler: SignalHandler,
        metrics: Arc<RwLock<Metrics>>,
        kaspad_pool: Pool<KaspadManager, Object<KaspadManager>>,
        blocks_queue: Arc<ArrayQueue<BlockData>>,
        txs_queue: Arc<ArrayQueue<TransactionData>>,
    ) -> KaspaBlocksFetcher {
        let ttl = settings.cli_args.cache_ttl;
        let cache_size = settings.net_bps as u64 * ttl * 2;
        let block_cache: Cache<KaspaHash, ()> =
            Cache::builder().time_to_live(Duration::from_secs(ttl)).max_capacity(cache_size).build();
        KaspaBlocksFetcher {
            disable_transaction_processing: settings.cli_args.is_disabled(CliDisable::TransactionProcessing),
            poll_interval: Duration::from_millis(settings.cli_args.block_interval),
            signal_handler,
            metrics,
            kaspad_pool,
            blocks_queue,
            txs_queue,
            low_hash: settings.checkpoint,
            last_sync_check: Instant::now() - Self::SYNC_CHECK_INTERVAL,
            synced: false,
            lag_count: 0,
            tip_hashes: HashSet::new(),
            block_cache,
        }
    }

    pub async fn start(&mut self) {
        let start_time = Instant::now();

        while !self.signal_handler.is_shutdown() {
            let last_fetch_time = Instant::now();
            debug!("Getting blocks with low_hash {}", self.low_hash);
            match self.kaspad_pool.get().await {
                Ok(kaspad) => match kaspad.get_blocks(Some(self.low_hash), true, !self.disable_transaction_processing).await {
                    Ok(response) => {
                        debug!("Received {} blocks", response.blocks.len());
                        trace!("Block hashes: \n{:#?}", response.block_hashes);
                        let blocks = response.blocks;
                        let blocks_len = blocks.len();
                        if !self.synced
                            && blocks_len < 100
                            && Instant::now().duration_since(self.last_sync_check) >= Self::SYNC_CHECK_INTERVAL
                        {
                            info!("Getting tip hashes from BlockDagInfo for sync check");
                            if let Ok(block_dag_info) = kaspad.get_block_dag_info().await {
                                self.tip_hashes = HashSet::from_iter(block_dag_info.tip_hashes.into_iter());
                                self.last_sync_check = Instant::now();
                            }
                        }
                        let mut txs_len = 0;
                        if blocks_len > 1 {
                            let last_block = blocks.last().unwrap().clone();
                            txs_len = self.handle_blocks(start_time, blocks).await;

                            let mut metrics = self.metrics.write().await;
                            metrics.queues.blocks = self.blocks_queue.len() as u64;
                            metrics.queues.transactions = self.txs_queue.len() as u64;
                            metrics.components.block_fetcher.update_last_block(MetricsBlock {
                                hash: last_block.verbose_data.unwrap().hash.to_string(),
                                timestamp: last_block.header.timestamp,
                                date_time: DateTime::from_timestamp_millis(last_block.header.timestamp as i64).unwrap(),
                                daa_score: last_block.header.daa_score,
                                blue_score: last_block.header.blue_score,
                            });
                        }
                        let fetch_time = Instant::now().duration_since(last_fetch_time).as_millis() as f64 / 1000f64;
                        debug!(
                            "Fetch blocks bps: {:.1}, tps: {:.1} ({:.1} txs/block)",
                            blocks_len as f64 / fetch_time,
                            txs_len as f64 / fetch_time,
                            txs_len as f64 / blocks_len as f64
                        );
                        if blocks_len < 50 {
                            sleep(self.poll_interval).await;
                        }
                    }
                    Err(e) => {
                        error!("Failed getting blocks with low_hash {}: {}", self.low_hash, e);
                        sleep(Duration::from_secs(5)).await;
                    }
                },
                Err(e) => {
                    error!("Failed getting kaspad connection from pool: {}", e);
                    sleep(Duration::from_secs(5)).await
                }
            }
        }
    }

    async fn handle_blocks(&mut self, start_time: Instant, blocks: Vec<RpcBlock>) -> usize {
        let mut txs_len = 0;
        let mut newest_block_timestamp = 0;
        let mut block_hash = self.low_hash;
        for b in blocks {
            if self.synced && b.header.timestamp > newest_block_timestamp {
                newest_block_timestamp = b.header.timestamp;
            }
            txs_len += b.transactions.len();
            block_hash = b.header.hash;
            if !self.synced && self.tip_hashes.contains(&block_hash) {
                let time_to_sync = Instant::now().duration_since(start_time);
                info!(
                    "\x1b[32mFound tip. Block fetcher synced! (in {}:{:0>2}:{:0>2}s)\x1b[0m",
                    time_to_sync.as_secs() / 3600,
                    time_to_sync.as_secs() % 3600 / 60,
                    time_to_sync.as_secs() % 60
                );
                self.synced = true;
            }
            if self.block_cache.contains_key(&block_hash) {
                trace!("Ignoring known block hash {}", block_hash);
                continue;
            }
            let mut transaction_data = TransactionData {
                transactions: b.transactions,
                block_hash: b.header.hash,
                block_timestamp: b.header.timestamp,
                block_daa_score: b.header.daa_score,
                block_blue_score: b.header.blue_score,
            };
            let mut block_data = BlockData {
                block: RpcBlock { header: b.header, transactions: vec![], verbose_data: b.verbose_data },
                synced: self.synced,
            };
            while !self.signal_handler.is_shutdown() {
                match self.blocks_queue.push(block_data) {
                    Ok(_) => break,
                    Err(v) => {
                        block_data = v;
                        sleep(Duration::from_millis(100)).await;
                    }
                }
            }
            while !self.signal_handler.is_shutdown() {
                match self.txs_queue.push(transaction_data) {
                    Ok(_) => break,
                    Err(v) => {
                        transaction_data = v;
                        sleep(Duration::from_millis(100)).await;
                    }
                }
            }
            self.block_cache.insert(block_hash, ());
        }
        self.low_hash = block_hash;
        self.lag_count = self.check_lag(self.synced, self.lag_count, newest_block_timestamp);
        txs_len
    }

    fn check_lag(&self, synced: bool, lag_count: i32, newest_block_timestamp: u64) -> i32 {
        if synced {
            let skew_seconds = Utc::now().timestamp() - newest_block_timestamp as i64 / 1000;
            if skew_seconds >= 30 {
                return if lag_count >= 15 {
                    warn!("\x1b[33mBlock fetcher is lagging behind. Newest block is {} seconds old\x1b[0m", skew_seconds);
                    0
                } else {
                    lag_count + 1
                };
            }
        }
        0
    }
}
