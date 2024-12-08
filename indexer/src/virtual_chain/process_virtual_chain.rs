use crate::settings::settings::Settings;
use crate::virtual_chain::update_transactions::update_txs;
use deadpool::managed::{Object, Pool};
use kaspa_rpc_core::api::rpc::RpcApi;
use log::{debug, info};
use simply_kaspa_database::client::client::KaspaDbClient;
use simply_kaspa_kaspad::pool::manager::KaspadManager;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::sleep;

pub async fn process_virtual_chain(
    settings: Settings,
    run: Arc<AtomicBool>,
    start_vcp: Arc<AtomicBool>,
    kaspad_pool: Pool<KaspadManager, Object<KaspadManager>>,
    database: KaspaDbClient,
) {
    let batch_scale = settings.cli_args.batch_scale;
    let mut start_hash = settings.checkpoint;

    let start_time = Instant::now();
    let mut synced = false;

    while run.load(Ordering::Relaxed) {
        if !start_vcp.load(Ordering::Relaxed) {
            debug!("Virtual chain processor waiting for start notification");
            sleep(Duration::from_secs(5)).await;
            continue;
        }
        debug!("Getting virtual chain from start_hash {}", start_hash.to_string());
        match kaspad_pool.get().await {
            Ok(kaspad) => {
                match kaspad.get_virtual_chain_from_block(start_hash, true).await {
                    Ok(res) => {
                        let added_blocks_count = res.added_chain_block_hashes.len();
                        if !res.accepted_transaction_ids.is_empty() {
                            let last_accepting = res.accepted_transaction_ids.last().unwrap().accepting_block_hash;
                            let timestamp = kaspad.get_block(last_accepting, false).await.expect("GetBlock failed").header.timestamp;
                            update_txs(
                                batch_scale,
                                &res.removed_chain_block_hashes,
                                &res.accepted_transaction_ids,
                                timestamp,
                                &database,
                            )
                            .await;
                            start_hash = last_accepting;
                        }
                        // Default batch size is 1800 on 1 bps:
                        if !synced && added_blocks_count < 200 {
                            log_time_to_synced(start_time);
                            synced = true;
                        }
                    }
                    Err(_) => {
                        let _ = kaspad.disconnect().await;
                        sleep(Duration::from_secs(5)).await;
                    }
                }
            }
            Err(_) => sleep(Duration::from_secs(5)).await,
        }
        if synced {
            sleep(Duration::from_secs(2)).await;
        }
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
