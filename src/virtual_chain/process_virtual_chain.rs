extern crate diesel;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use diesel::r2d2::{ConnectionManager, Pool};
use diesel::PgConnection;
use kaspa_hashes::Hash;
use kaspa_rpc_core::api::rpc::RpcApi;
use kaspa_wrpc_client::KaspaRpcClient;
use log::{debug, info};
use tokio::task;
use tokio::time::sleep;

use crate::kaspad::client::with_retry;
use crate::virtual_chain::update_chain_blocks::update_chain_blocks;
use crate::virtual_chain::update_transactions::update_transactions;

pub async fn process_virtual_chain(
    run: Arc<AtomicBool>,
    start_vcp: Arc<AtomicBool>,
    batch_scale: f64,
    checkpoint: Hash,
    kaspad: KaspaRpcClient,
    db_pool: Pool<ConnectionManager<PgConnection>>,
) {
    let start_time = Instant::now();
    let mut synced = false;
    let mut start_hash = checkpoint;

    while run.load(Ordering::Relaxed) && !start_vcp.load(Ordering::Relaxed) {
        debug!("Waiting for start notification");
        sleep(Duration::from_secs(5)).await;
    }

    while run.load(Ordering::Relaxed) {
        debug!("Getting virtual chain from start_hash {}", start_hash.to_string());
        let response = with_retry(|| kaspad.get_virtual_chain_from_block(start_hash, true))
            .await
            .expect("Error when invoking GetVirtualChainFromBlock");

        if !response.accepted_transaction_ids.is_empty() {
            let last_accepting = response.accepted_transaction_ids.last().unwrap().accepting_block_hash;
            let last_accepting_time =
                with_retry(|| kaspad.get_block(last_accepting, false)).await.expect("Error when invoking GetBlock").header.timestamp;

            let db_pool_clone = db_pool.clone();
            let removed_chain_block_hashes_clone = response.removed_chain_block_hashes.clone();
            let _ = task::spawn_blocking(move || {
                update_transactions(
                    batch_scale,
                    removed_chain_block_hashes_clone,
                    response.accepted_transaction_ids,
                    last_accepting_time,
                    db_pool_clone,
                )
            })
            .await;
            let db_pool_clone = db_pool.clone();
            let _ = task::spawn_blocking(move || {
                update_chain_blocks(batch_scale, response.added_chain_block_hashes, response.removed_chain_block_hashes, db_pool_clone)
            })
            .await;

            start_hash = last_accepting;

            if !synced {
                log_time_to_synced(start_time);
                synced = true;
            }
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
