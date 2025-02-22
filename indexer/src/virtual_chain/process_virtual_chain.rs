use crate::checkpoint::{CheckpointBlock, CheckpointOrigin};
use crate::settings::Settings;
use crate::vars::save_vcp_checkpoint;
use crate::virtual_chain::accept_transactions::accept_transactions;
use crate::virtual_chain::add_chain_blocks::add_chain_blocks;
use crate::virtual_chain::remove_chain_blocks::remove_chain_blocks;
use crate::web::model::metrics::Metrics;
use deadpool::managed::{Object, Pool};
use kaspa_rpc_core::api::rpc::RpcApi;
use log::{debug, error, info};
use simply_kaspa_cli::cli_args::CliDisable;
use simply_kaspa_database::client::KaspaDbClient;
use simply_kaspa_kaspad::pool::manager::KaspadManager;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tokio::time::sleep;

pub async fn process_virtual_chain(
    settings: Settings,
    run: Arc<AtomicBool>,
    metrics: Arc<RwLock<Metrics>>,
    kaspad_pool: Pool<KaspadManager, Object<KaspadManager>>,
    database: KaspaDbClient,
) {
    let batch_scale = settings.cli_args.batch_scale;
    let disable_transaction_acceptance = settings.cli_args.is_disabled(CliDisable::TransactionAcceptance);
    let mut start_hash = settings.vcp_checkpoint;

    let start_time = Instant::now();
    let mut synced = false;

    while run.load(Ordering::Relaxed) {
        debug!("Getting virtual chain from start_hash {}", start_hash.to_string());
        match kaspad_pool.get().await {
            Ok(kaspad) => {
                match kaspad.get_virtual_chain_from_block(start_hash, !disable_transaction_acceptance).await {
                    Ok(res) => {
                        let added_blocks_count = res.added_chain_block_hashes.len();
                        if added_blocks_count > 0 {
                            let last_accepting_block =
                                kaspad.get_block(*res.added_chain_block_hashes.last().unwrap(), false).await.unwrap();
                            let checkpoint_block = CheckpointBlock {
                                origin: CheckpointOrigin::Vcp,
                                hash: last_accepting_block.header.hash.into(),
                                timestamp: last_accepting_block.header.timestamp,
                                daa_score: last_accepting_block.header.daa_score,
                                blue_score: last_accepting_block.header.blue_score,
                            };
                            let rows_removed = remove_chain_blocks(batch_scale, &res.removed_chain_block_hashes, &database).await;
                            if !disable_transaction_acceptance {
                                let rows_added = accept_transactions(batch_scale, &res.accepted_transaction_ids, &database).await;
                                info!(
                                    "Committed {} accepted and {} rejected transactions. Last accepted: {}",
                                    rows_added,
                                    rows_removed,
                                    chrono::DateTime::from_timestamp_millis(checkpoint_block.timestamp as i64 / 1000 * 1000).unwrap()
                                );
                            } else {
                                let rows_added = add_chain_blocks(batch_scale, &res.added_chain_block_hashes, &database).await;
                                info!(
                                    "Committed {} added and {} removed chain blocks. Last added: {}",
                                    rows_added,
                                    rows_removed,
                                    chrono::DateTime::from_timestamp_millis(checkpoint_block.timestamp as i64 / 1000 * 1000).unwrap()
                                );
                            }
                            let mut metrics = metrics.write().await;
                            metrics.components.virtual_chain_processor.last_block = Some(checkpoint_block.clone().into());
                            metrics.vcp_checkpoint = Some(checkpoint_block.into());
                            drop(metrics);

                            start_hash = last_accepting_block.header.hash;
                            save_vcp_checkpoint(&hex::encode(start_hash.as_bytes()), &database).await.unwrap();
                        }
                        // Default batch size is 1800 on 1 bps:
                        if !synced && added_blocks_count < 200 {
                            log_time_to_synced(start_time);
                            synced = true;
                        }
                    }
                    Err(e) => {
                        error!("Failed getting virtual chain from start_hash {}: {}", start_hash.to_string(), e);
                        sleep(Duration::from_secs(5)).await;
                    }
                }
            }
            Err(e) => {
                error!("Failed getting kaspad connection from pool: {}", e);
                sleep(Duration::from_secs(5)).await
            }
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
