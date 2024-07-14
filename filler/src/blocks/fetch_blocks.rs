use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use chrono::Utc;
use crossbeam_queue::ArrayQueue;
use kaspa_hashes::Hash as KaspaHash;
use kaspa_rpc_core::{RpcBlock, RpcTransaction};
use kaspa_rpc_core::api::rpc::RpcApi;
use kaspa_wrpc_client::KaspaRpcClient;
use log::{debug, trace, warn};
use log::info;
use moka::sync::Cache;
use tokio::time::sleep;

use crate::kaspad::client::with_retry;
use crate::settings::settings::Settings;

pub async fn fetch_blocks(
    settings: Settings,
    run: Arc<AtomicBool>,
    kaspad: KaspaRpcClient,
    rpc_blocks_queue: Arc<ArrayQueue<(RpcBlock, bool)>>,
    rpc_transactions_queue: Arc<ArrayQueue<Vec<RpcTransaction>>>,
) {
    const SYNC_CHECK_INTERVAL: Duration = Duration::from_secs(30);
    let start_time = Instant::now();
    let mut low_hash = settings.checkpoint;
    let mut last_sync_check = Instant::now() - SYNC_CHECK_INTERVAL;
    let mut synced = false;
    let mut lag_count = 0;
    let mut tip_hashes = HashSet::new();

    let ttl = settings.cli_args.cache_ttl;
    let cache_size = settings.net_bps as u64 * ttl.as_secs() * 2;
    let block_cache: Cache<KaspaHash, ()> = Cache::builder().time_to_live(ttl).max_capacity(cache_size).build();

    while run.load(Ordering::Relaxed) {
        let last_fetch_time = Instant::now();
        debug!("Getting blocks with low_hash {}", low_hash.to_string());
        let response = with_retry(|| kaspad.get_blocks(Some(low_hash), true, true)).await.expect("Error when invoking GetBlocks");
        debug!("Received {} blocks", response.blocks.len());
        trace!("Block hashes: \n{:#?}", response.block_hashes);

        if !synced && response.blocks.len() < 100 {
            if Instant::now().duration_since(last_sync_check) >= SYNC_CHECK_INTERVAL {
                let block_dag_info = kaspad.get_block_dag_info().await.expect("Error when invoking GetBlockDagInfo");
                info!("Getting tip hashes from BlockDagInfo for sync check");
                tip_hashes = HashSet::from_iter(block_dag_info.tip_hashes.into_iter());
                last_sync_check = Instant::now();
            }
        }

        let blocks_len = response.blocks.len();
        let mut txs_len = 0;
        if blocks_len > 1 {
            low_hash = response.blocks.last().unwrap().header.hash;
            let mut newest_block_timestamp = 0;
            for b in response.blocks {
                if synced && b.header.timestamp > newest_block_timestamp {
                    newest_block_timestamp = b.header.timestamp;
                }
                txs_len += b.transactions.len();
                let block_hash = b.header.hash;
                if !synced && tip_hashes.contains(&block_hash) {
                    let time_to_sync = Instant::now().duration_since(start_time);
                    info!(
                        "\x1b[32mFound tip. Block fetcher synced! (in {}:{:0>2}:{:0>2}s)\x1b[0m",
                        time_to_sync.as_secs() / 3600,
                        time_to_sync.as_secs() % 3600 / 60,
                        time_to_sync.as_secs() % 60
                    );
                    synced = true;
                }
                if block_cache.contains_key(&block_hash) {
                    trace!("Ignoring known block hash {}", block_hash.to_string());
                    continue;
                }
                let mut last_blocks_warn = Instant::now();
                while rpc_blocks_queue.is_full() && run.load(Ordering::Relaxed) {
                    if Instant::now().duration_since(last_blocks_warn).as_secs() >= 30 {
                        warn!("RPC blocks queue is full");
                        last_blocks_warn = Instant::now();
                    }
                    sleep(Duration::from_secs(1)).await;
                }
                let mut last_transactions_warn = Instant::now();
                while rpc_transactions_queue.is_full() && run.load(Ordering::Relaxed) {
                    if Instant::now().duration_since(last_transactions_warn).as_secs() >= 30 {
                        warn!("RPC transactions queue is full");
                        last_transactions_warn = Instant::now();
                    }
                    sleep(Duration::from_secs(1)).await;
                }
                rpc_blocks_queue
                    .push((RpcBlock { header: b.header, transactions: vec![], verbose_data: b.verbose_data }, synced))
                    .unwrap();
                rpc_transactions_queue.push(b.transactions).unwrap();
                block_cache.insert(block_hash, ());
            }
            if synced {
                let skew_seconds = Utc::now().timestamp() - newest_block_timestamp as i64 / 1000;
                if skew_seconds >= 30 {
                    lag_count += 1;
                    if lag_count >= 15 {
                        warn!("\x1b[33mBlock fetcher is lagging behind. Newest block is {} seconds old\x1b[0m", skew_seconds);
                        lag_count = 0;
                    }
                } else {
                    lag_count = 0;
                }
            }
        }
        if blocks_len < 50 {
            sleep(Duration::from_secs(2)).await;
        }
        let fetch_time = Instant::now().duration_since(last_fetch_time).as_millis() as f64 / 1000f64;
        debug!(
            "Fetch blocks bps: {:.1}, tps: {:.1} ({:.1} txs/block)",
            blocks_len as f64 / fetch_time,
            txs_len as f64 / fetch_time,
            txs_len as f64 / blocks_len as f64
        );
    }
}
