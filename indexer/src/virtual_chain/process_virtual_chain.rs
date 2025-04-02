use crate::checkpoint::{CheckpointBlock, CheckpointOrigin};
use crate::settings::Settings;
use crate::virtual_chain::accept_transactions::accept_transactions;
use crate::virtual_chain::add_chain_blocks::add_chain_blocks;
use crate::virtual_chain::remove_chain_blocks::remove_chain_blocks;
use crate::web::model::metrics::Metrics;
use crossbeam_queue::ArrayQueue;
use deadpool::managed::{Object, Pool};
use kaspa_rpc_core::api::rpc::RpcApi;
use log::{debug, error, info, warn};
use simply_kaspa_cli::cli_args::CliDisable;
use simply_kaspa_database::client::KaspaDbClient;
use simply_kaspa_kaspad::pool::manager::KaspadManager;
use std::collections::{HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tokio::time::sleep;

pub async fn process_virtual_chain(
    settings: Settings,
    run: Arc<AtomicBool>,
    metrics: Arc<RwLock<Metrics>>,
    start_vcp: Arc<AtomicBool>,
    checkpoint_queue: Arc<ArrayQueue<CheckpointBlock>>,
    kaspad_pool: Pool<KaspadManager, Object<KaspadManager>>,
    database: KaspaDbClient,
) {
    let batch_scale = settings.cli_args.batch_scale;
    let disable_transaction_acceptance = settings.cli_args.is_disabled(CliDisable::TransactionAcceptance);

    let poll_interval = Duration::from_secs(settings.cli_args.vcp_interval as u64);
    let err_delay = Duration::from_secs(5);

    let mut start_hash = settings.checkpoint;
    let start_time = Instant::now();
    let mut synced = false;

    let mut tip_distance = 5;
    let mut recent_removals = VecDeque::new();
    let tip_decrease_window = settings.cli_args.vcp_window.saturating_div(settings.cli_args.vcp_interval as u16).max(1) as usize;

    while run.load(Ordering::Relaxed) {
        if !start_vcp.load(Ordering::Relaxed) {
            debug!("Virtual chain processor waiting for start notification");
            sleep(err_delay).await;
            continue;
        }
        debug!("Getting virtual chain from start_hash {}", start_hash.to_string());
        match kaspad_pool.get().await {
            Ok(kaspad) => {
                match kaspad.get_virtual_chain_from_block(start_hash, !disable_transaction_acceptance).await {
                    Ok(res) => {
                        let start_request_time = Instant::now();
                        let added_blocks_count = res.added_chain_block_hashes.len();
                        if added_blocks_count > tip_distance {
                            let removed_chain_block_hashes = res.removed_chain_block_hashes.as_slice();
                            let added_chain_block_hashes = &res.added_chain_block_hashes[..added_blocks_count - tip_distance];
                            let accepted_transaction_ids = &res.accepted_transaction_ids[..added_blocks_count - tip_distance];
                            let last_accepting_block =
                                kaspad.get_block(*added_chain_block_hashes.last().unwrap(), false).await.unwrap();
                            let checkpoint_block = CheckpointBlock {
                                origin: CheckpointOrigin::Vcp,
                                hash: last_accepting_block.header.hash.into(),
                                timestamp: last_accepting_block.header.timestamp,
                                daa_score: last_accepting_block.header.daa_score,
                                blue_score: last_accepting_block.header.blue_score,
                            };
                            let start_commit_time = Instant::now();
                            let rows_removed = remove_chain_blocks(batch_scale, removed_chain_block_hashes, &database).await;
                            if !disable_transaction_acceptance {
                                let rows_added = accept_transactions(batch_scale, accepted_transaction_ids, &database).await;
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
                            if recent_removals.len() >= tip_decrease_window {
                                recent_removals.pop_front();
                            }
                            recent_removals.push_back(rows_removed > 0);
                            if rows_removed > 0 {
                                tip_distance += 1;
                                info!("Increased vcp tip distance to {tip_distance}");
                            } else if recent_removals.len() == tip_decrease_window && recent_removals.iter().all(|&x| !x) {
                                tip_distance = tip_distance.saturating_sub(1);
                                info!("Decreased vcp tip distance to {tip_distance}");
                            }
                            let mut metrics = metrics.write().await;
                            metrics.components.virtual_chain_processor.last_block = Some(checkpoint_block.clone().into());
                            metrics.components.virtual_chain_processor.tip_distance = tip_distance as u64;
                            drop(metrics);

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
                        error!("Failed getting virtual chain from start_hash {}: {}", start_hash.to_string(), e);
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
