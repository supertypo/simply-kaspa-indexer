use crate::checkpoint::{CheckpointBlock, CheckpointOrigin};
use crate::settings::Settings;
use crate::virtual_chain::accept_transactions::accept_transactions;
use crate::virtual_chain::add_chain_blocks::add_chain_blocks;
use crate::virtual_chain::remove_chain_blocks::remove_chain_blocks;
use crate::web::model::metrics::Metrics;
use chrono::DateTime;
use crossbeam_queue::ArrayQueue;
use deadpool::managed::{Object, Pool};
use kaspa_rpc_core::api::rpc::RpcApi;
use log::{debug, error, info, trace, warn};
use simply_kaspa_cli::cli_args::{CliDisable, CliEnable};
use simply_kaspa_database::client::KaspaDbClient;
use simply_kaspa_kaspad::manager::KaspadManager;
use simply_kaspa_signal::signal_handler::SignalHandler;
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tokio::time::sleep;

pub async fn process_virtual_chain(
    settings: Settings,
    signal_handler: SignalHandler,
    metrics: Arc<RwLock<Metrics>>,
    start_vcp: Arc<AtomicBool>,
    checkpoint_queue: Arc<ArrayQueue<CheckpointBlock>>,
    kaspad_pool: Pool<KaspadManager, Object<KaspadManager>>,
    database: KaspaDbClient,
) {
    let batch_scale = settings.cli_args.batch_scale;
    let batch_concurrency = settings.cli_args.batch_concurrency;
    let disable_transaction_acceptance = settings.cli_args.is_disabled(CliDisable::TransactionAcceptance);

    let poll_interval = Duration::from_millis(settings.cli_args.vcp_interval);
    let err_delay = Duration::from_secs(5);

    let mut start_hash = settings.checkpoint;
    let start_time = Instant::now();
    let mut synced = false;

    let dynamic_tip_distance = settings.cli_args.is_enabled(CliEnable::DynamicVcpTipDistance);
    let mut tip_distance = if dynamic_tip_distance { 10 } else { 0 };
    let mut tip_distance_timestamp = 0;
    let mut tip_distance_history = VecDeque::new();
    let tip_distance_window = (settings.cli_args.vcp_window * 1_000 / settings.cli_args.vcp_interval).max(1) as usize;

    while !signal_handler.is_shutdown() {
        if !start_vcp.load(Ordering::Relaxed) {
            debug!("Virtual chain processor waiting for start notification");
            sleep(err_delay).await;
            continue;
        }
        debug!("Getting virtual chain from start_hash {}", start_hash);
        match kaspad_pool.get().await {
            Ok(kaspad) => {
                match kaspad.get_virtual_chain_from_block(start_hash, !disable_transaction_acceptance).await {
                    Ok(res) => {
                        let start_request_time = Instant::now();
                        let added_blocks_count = res.added_chain_block_hashes.len();
                        if added_blocks_count > tip_distance {
                            let removed_chain_block_hashes = res.removed_chain_block_hashes.as_slice();
                            let added_chain_block_hashes = &res.added_chain_block_hashes[..added_blocks_count - tip_distance];
                            let last_accepting_block =
                                kaspad.get_block(*added_chain_block_hashes.last().unwrap(), false).await.unwrap();
                            let checkpoint_block = CheckpointBlock {
                                origin: CheckpointOrigin::Vcp,
                                hash: last_accepting_block.header.hash.into(),
                                timestamp: last_accepting_block.header.timestamp,
                                daa_score: last_accepting_block.header.daa_score,
                                blue_score: last_accepting_block.header.blue_score,
                            };
                            loop {
                                if let Some(b) = &metrics.read().await.components.block_processor.last_block {
                                    // Don't allow VCP to run ahead of blocks processor by more than 1 minute
                                    if checkpoint_block.daa_score.saturating_sub(b.daa_score) < 60 * settings.net_bps as u64 {
                                        break;
                                    }
                                }
                                trace!("Virtual chain processor is waiting for block_processor to catch up...");
                                sleep(poll_interval).await;
                                if signal_handler.is_shutdown() {
                                    return;
                                }
                            }
                            let start_commit_time = Instant::now();
                            let rows_removed = remove_chain_blocks(batch_scale, removed_chain_block_hashes, &database).await;
                            if !disable_transaction_acceptance {
                                let accepted_transaction_ids = &res.accepted_transaction_ids[..added_blocks_count - tip_distance];
                                let rows_added =
                                    accept_transactions(batch_scale, batch_concurrency, accepted_transaction_ids, &database).await;
                                info!(
                                    "Committed {} accepted and {} rejected transactions in {}ms. Last accepted: {}",
                                    rows_added,
                                    rows_removed,
                                    Instant::now().duration_since(start_commit_time).as_millis(),
                                    chrono::DateTime::from_timestamp_millis(checkpoint_block.timestamp as i64 / 1000 * 1000).unwrap()
                                );
                            } else {
                                let rows_added = add_chain_blocks(batch_scale, added_chain_block_hashes, &database).await;
                                info!(
                                    "Committed {} added and {} removed chain blocks in {}ms. Last added: {}",
                                    rows_added,
                                    rows_removed,
                                    Instant::now().duration_since(start_commit_time).as_millis(),
                                    chrono::DateTime::from_timestamp_millis(checkpoint_block.timestamp as i64 / 1000 * 1000).unwrap()
                                );
                            }

                            if dynamic_tip_distance {
                                if tip_distance_history.len() == tip_distance_window {
                                    tip_distance_history.pop_back();
                                }
                                tip_distance_history.push_front(rows_removed > 0);
                                let reorgs_count = tip_distance_history.iter().filter(|&&x| x).count();
                                if reorgs_count >= 3 {
                                    tip_distance += 1;
                                    tip_distance_timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis();
                                    // Increase distance if new reorgs occur within the window:
                                    tip_distance_history.pop_front();
                                    tip_distance_history.push_front(false);
                                    debug!("Increased vcp tip distance to {tip_distance}");
                                } else if synced && reorgs_count == 0 && tip_distance > 0 {
                                    tip_distance -= 1;
                                    tip_distance_timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis();
                                    if tip_distance_history.len() == tip_distance_window {
                                        // Make sure we don't decrease distance again until a complete window has passed:
                                        tip_distance_history.pop_front();
                                        tip_distance_history.push_front(true);
                                    }
                                    debug!("Decreased vcp tip distance to {tip_distance}");
                                }
                            }
                            {
                                let mut metrics = metrics.write().await;
                                metrics.components.virtual_chain_processor.update_last_block(checkpoint_block.clone().into());
                                metrics.components.virtual_chain_processor.tip_distance = Some(tip_distance as u64);
                                metrics.components.virtual_chain_processor.tip_distance_timestamp =
                                    dynamic_tip_distance.then_some(tip_distance_timestamp as u64);
                                metrics.components.virtual_chain_processor.tip_distance_date_time = dynamic_tip_distance
                                    .then_some(DateTime::from_timestamp_millis(tip_distance_timestamp as i64))
                                    .flatten();
                            }

                            while checkpoint_queue.push(checkpoint_block.clone()).is_err() {
                                warn!("Checkpoint queue is full");
                                sleep(Duration::from_secs(1)).await;
                            }
                            start_hash = last_accepting_block.header.hash;
                        }
                        // Default batch size is 1800 on 1 bps:
                        if !synced && added_blocks_count < 200 {
                            log_time_to_synced(start_time);
                            synced = true;
                        }
                        if synced {
                            sleep(poll_interval.saturating_sub(Instant::now().duration_since(start_request_time))).await;
                        }
                    }
                    Err(e) => {
                        error!("Failed getting virtual chain from start_hash {}: {}", start_hash, e);
                        sleep(err_delay).await;
                    }
                }
            }
            Err(e) => {
                error!("Failed getting kaspad connection from pool: {}", e);
                sleep(err_delay).await
            }
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
