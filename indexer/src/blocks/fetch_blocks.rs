use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::settings::settings::Settings;
use chrono::Utc;
use crossbeam_queue::ArrayQueue;
use deadpool::managed::{Object, Pool};
use kaspa_hashes::Hash as KaspaHash;
use kaspa_rpc_core::api::rpc::RpcApi;
use kaspa_rpc_core::{GetBlocksResponse, RpcBlock, RpcTransaction};
use log::info;
use log::{debug, trace, warn};
use moka::sync::Cache;
use simply_kaspa_kaspad::pool::manager::KaspadManager;
use tokio::time::sleep;

#[derive(Debug)]
pub struct BlockData {
    pub block: RpcBlock,
    pub synced: bool,
}

pub struct KaspaBlocksFetcher {
    run: Arc<AtomicBool>,
    kaspad_pool: Pool<KaspadManager, Object<KaspadManager>>,
    blocks_queue: Arc<ArrayQueue<BlockData>>,
    txs_queue: Arc<ArrayQueue<Vec<RpcTransaction>>>,
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
        run: Arc<AtomicBool>,
        kaspad_pool: Pool<KaspadManager, Object<KaspadManager>>,
        blocks_queue: Arc<ArrayQueue<BlockData>>,
        txs_queue: Arc<ArrayQueue<Vec<RpcTransaction>>>,
    ) -> KaspaBlocksFetcher {
        let ttl = settings.cli_args.cache_ttl;
        let cache_size = settings.net_bps as u64 * ttl * 2;
        let block_cache: Cache<KaspaHash, ()> =
            Cache::builder().time_to_live(Duration::from_secs(ttl)).max_capacity(cache_size).build();
        KaspaBlocksFetcher {
            run,
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

    pub async fn start(&mut self) -> () {
        self.run.store(true, Ordering::Relaxed);
        self.fetch_blocks().await;
    }

    async fn fetch_blocks(&mut self) {
        let start_time = Instant::now();

        while self.run.load(Ordering::Relaxed) {
            let last_fetch_time = Instant::now();
            debug!("Getting blocks with low_hash {}", self.low_hash.to_string());
            match self.kaspad_pool.get().await {
                Ok(kaspad) => match kaspad.get_blocks(Some(self.low_hash), true, true).await {
                    Ok(response) => {
                        debug!("Received {} blocks", response.blocks.len());
                        trace!("Block hashes: \n{:#?}", response.block_hashes);
                        if !self.synced
                            && response.blocks.len() < 100
                            && Instant::now().duration_since(self.last_sync_check) >= Self::SYNC_CHECK_INTERVAL
                        {
                            info!("Getting tip hashes from BlockDagInfo for sync check");
                            if let Ok(block_dag_info) = kaspad.get_block_dag_info().await {
                                self.tip_hashes = HashSet::from_iter(block_dag_info.tip_hashes.into_iter());
                                self.last_sync_check = Instant::now();
                            }
                        }

                        let blocks_len = response.blocks.len();
                        let mut txs_len = 0;
                        if blocks_len > 1 {
                            txs_len = self.handle_blocks(start_time, response).await;
                        }
                        let fetch_time = Instant::now().duration_since(last_fetch_time).as_millis() as f64 / 1000f64;
                        debug!(
                            "Fetch blocks bps: {:.1}, tps: {:.1} ({:.1} txs/block)",
                            blocks_len as f64 / fetch_time,
                            txs_len as f64 / fetch_time,
                            txs_len as f64 / blocks_len as f64
                        );
                        if blocks_len < 50 {
                            sleep(Duration::from_secs(2)).await;
                        }
                    }
                    Err(_) => {
                        let _ = kaspad.disconnect().await;
                        sleep(Duration::from_secs(5)).await;
                    }
                },
                Err(_) => sleep(Duration::from_secs(5)).await,
            }
        }
    }

    async fn handle_blocks(&mut self, start_time: Instant, response: GetBlocksResponse) -> usize {
        let mut txs_len = 0;
        self.low_hash = response.blocks.last().unwrap().header.hash;
        let mut newest_block_timestamp = 0;
        for b in response.blocks {
            if self.synced && b.header.timestamp > newest_block_timestamp {
                newest_block_timestamp = b.header.timestamp;
            }
            txs_len += b.transactions.len();
            let block_hash = b.header.hash;
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
                trace!("Ignoring known block hash {}", block_hash.to_string());
                continue;
            }
            self.blocks_queue_space().await;
            self.txs_queue_space().await;
            let block_data = BlockData {
                block: RpcBlock { header: b.header, transactions: vec![], verbose_data: b.verbose_data },
                synced: self.synced,
            };
            self.blocks_queue.push(block_data).expect("Failed to enqueue block data");
            self.txs_queue.push(b.transactions).expect("Failed to enqueue transactions");
            self.block_cache.insert(block_hash, ());
        }
        self.lag_count = self.check_lag(self.synced, self.lag_count, newest_block_timestamp);
        txs_len
    }

    async fn txs_queue_space(&self) {
        let mut last_transactions_warn = Instant::now();
        while self.txs_queue.is_full() && self.run.load(Ordering::Relaxed) {
            if Instant::now().duration_since(last_transactions_warn).as_secs() >= 30 {
                warn!("RPC transactions queue is full");
                last_transactions_warn = Instant::now();
            }
            sleep(Duration::from_secs(1)).await;
        }
    }

    async fn blocks_queue_space(&self) {
        let mut last_blocks_warn = Instant::now();
        while self.blocks_queue.is_full() && self.run.load(Ordering::Relaxed) {
            if Instant::now().duration_since(last_blocks_warn).as_secs() >= 30 {
                warn!("RPC blocks queue is full");
                last_blocks_warn = Instant::now();
            }
            sleep(Duration::from_secs(1)).await;
        }
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
