extern crate diesel;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use diesel::r2d2::{ConnectionManager, Pool};
use diesel::PgConnection;
use kaspa_rpc_core::api::rpc::RpcApi;
use kaspa_wrpc_client::KaspaRpcClient;
use log::{debug, info};
use tokio::task;
use tokio::time::sleep;

use crate::kaspad::client::with_retry;
use crate::virtual_chain::update_chain_blocks::update_chain_blocks;
use crate::virtual_chain::update_transactions::update_transactions;

pub async fn process_virtual_chain(
    running: Arc<AtomicBool>,
    start_vcp: Arc<AtomicBool>,
    buffer_size: f64,
    checkpoint_hash: String,
    kaspad_client: KaspaRpcClient,
    db_pool: Pool<ConnectionManager<PgConnection>>,
) {
    let start_time = Instant::now();
    let mut synced = false;
    let mut checkpoint_hash = hex::decode(checkpoint_hash.as_bytes()).unwrap();

    while running.load(Ordering::Relaxed) && !start_vcp.load(Ordering::Relaxed) {
        debug!("Waiting for start notification");
        sleep(Duration::from_secs(5)).await;
    }

    while running.load(Ordering::Relaxed) {
        debug!("Getting virtual chain from start_hash {}", hex::encode(checkpoint_hash.clone()));
        let response = with_retry(|| {
            kaspad_client.get_virtual_chain_from_block(kaspa_hashes::Hash::from_slice(checkpoint_hash.as_slice()), true)
        })
        .await
        .expect("Error when invoking GetVirtualChainFromBlock");

        let last_accepted_block_hash = response.accepted_transaction_ids.last().map(|ta| ta.accepting_block_hash.clone());

        let mut last_accepted_block_time: Option<u64> = None;
        if let Some(block_hash) = last_accepted_block_hash.clone() {
            last_accepted_block_time = Some(
                with_retry(|| kaspad_client.get_block(block_hash, false))
                    .await
                    .expect("Error when invoking GetBlock")
                    .header
                    .timestamp,
            );
        }

        let db_pool_clone = db_pool.clone();
        let removed_chain_block_hashes_clone = response.removed_chain_block_hashes.clone();
        let _ = task::spawn_blocking(move || {
            update_transactions(
                buffer_size,
                removed_chain_block_hashes_clone,
                response.accepted_transaction_ids,
                last_accepted_block_time.clone(),
                db_pool_clone,
            )
        })
        .await;
        let db_pool_clone = db_pool.clone();
        let _ = task::spawn_blocking(move || {
            update_chain_blocks(buffer_size, response.added_chain_block_hashes, response.removed_chain_block_hashes, db_pool_clone)
        })
        .await;

        if let Some(new_checkpoint_hash) = last_accepted_block_hash {
            checkpoint_hash = new_checkpoint_hash.as_bytes().to_vec();
        }

        if !synced {
            let time_to_sync = Instant::now().duration_since(start_time);
            info!(
                "\x1b[32mVirtual chain processor synced! (in {}:{:0>2}:{:0>2}s)\x1b[0m",
                time_to_sync.as_secs() / 3600,
                time_to_sync.as_secs() % 3600 / 60,
                time_to_sync.as_secs() % 60
            );
            synced = true;
        }

        sleep(Duration::from_secs(3)).await;
    }
}
