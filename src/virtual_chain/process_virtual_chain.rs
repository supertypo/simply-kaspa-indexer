extern crate diesel;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::database::client::client::KaspaDbClient;
use kaspa_hashes::Hash;
use kaspa_rpc_core::api::rpc::RpcApi;
use kaspa_wrpc_client::KaspaRpcClient;
use log::{debug, info};
use tokio::time::sleep;

use crate::kaspad::client::with_retry;
use crate::virtual_chain::update_chain_blocks::update_chain_blocks;
use crate::virtual_chain::update_transactions::update_txs;

pub async fn process_virtual_chain(
    run: Arc<AtomicBool>,
    start_vcp: Arc<AtomicBool>,
    batch_scale: f64,
    checkpoint: Hash,
    kaspad: KaspaRpcClient,
    database: KaspaDbClient,
) {
    let start_time = Instant::now();
    let mut synced = false;
    let mut start_hash = checkpoint;

    while run.load(Ordering::Relaxed) {
        if !start_vcp.load(Ordering::Relaxed) {
            debug!("Waiting for start notification");
            sleep(Duration::from_secs(5)).await;
            continue;
        }
        debug!("Getting virtual chain from start_hash {}", start_hash.to_string());
        let res = with_retry(|| kaspad.get_virtual_chain_from_block(start_hash, true)).await.expect("GetVirtualChainFromBlock failed");

        if !res.accepted_transaction_ids.is_empty() {
            let last_accepting = res.accepted_transaction_ids.last().unwrap().accepting_block_hash;
            let timestamp = with_retry(|| kaspad.get_block(last_accepting, false)).await.expect("GetBlock failed").header.timestamp;
            update_txs(batch_scale, &res.removed_chain_block_hashes, &res.accepted_transaction_ids, timestamp, &database).await;
            update_chain_blocks(batch_scale, &res.added_chain_block_hashes, &res.removed_chain_block_hashes, &database).await;
            if !synced {
                log_time_to_synced(start_time);
                synced = true;
            }
            start_hash = last_accepting;
        }
        sleep(Duration::from_secs(2)).await;
    }
}

fn log_time_to_synced(start_time: Instant) {
    let time_to_sync = Instant::now().duration_since(start_time);
    info!(
        "\x1b[32mVirtual chain processor synced! (in {}:{:0>2}:{:0>2}s)\x1b[0m",
        time_to_sync.as_secs() / 3600,
        time_to_sync.as_secs() % 3600 / 60,
        time_to_sync.as_secs() % 60
    );
}
