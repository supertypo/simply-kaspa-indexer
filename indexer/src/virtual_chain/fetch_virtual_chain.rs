use crate::settings::Settings;
use crate::web::model::metrics::Metrics;
use deadpool::managed::{Object, Pool};
use kaspa_rpc_core::api::rpc::RpcApi;
use kaspa_rpc_core::{GetVirtualChainFromBlockV2Response, RpcDataVerbosityLevel};
use log::{debug, error};
use mpsc::Sender;
use simply_kaspa_kaspad::manager::KaspadManager;
use simply_kaspa_signal::signal_handler::SignalHandler;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::sync::mpsc;
use tokio::time::sleep;

pub async fn fetch_virtual_chain(
    settings: Settings,
    signal_handler: SignalHandler,
    metrics: Arc<RwLock<Metrics>>,
    start_vcp: Arc<AtomicBool>,
    kaspad_pool: Pool<KaspadManager, Object<KaspadManager>>,
    sender: Sender<GetVirtualChainFromBlockV2Response>,
) {
    let poll_interval = Duration::from_millis(settings.cli_args.vcp_interval);
    let err_delay = Duration::from_secs(5);

    let mut start_hash = settings.checkpoint;

    loop {
        if signal_handler.is_shutdown() {
            return;
        }

        if !start_vcp.load(Ordering::Relaxed) {
            debug!("Virtual chain processor waiting for start notification");
            sleep(err_delay).await;
            continue;
        }

        debug!("Getting virtual chain from start_hash {}", start_hash);
        let tip_distance = metrics.read().await.components.virtual_chain_processor.tip_distance.unwrap_or(10);
        let kaspad = match kaspad_pool.get().await {
            Ok(k) => k,
            Err(e) => {
                error!("Failed getting kaspad connection from pool: {}", e);
                sleep(err_delay).await;
                continue;
            }
        };

        match kaspad.get_virtual_chain_from_block_v2(start_hash, Some(RpcDataVerbosityLevel::Full), Some(tip_distance)).await {
            Ok(res) => {
                let added_blocks_count = res.added_chain_block_hashes.len();
                if added_blocks_count > 0 {
                    start_hash = *res.added_chain_block_hashes.last().unwrap();
                    let last_daa_score = res.chain_block_accepted_transactions.last().unwrap().chain_block_header.daa_score.unwrap();

                    if sender.send(res).await.is_err() {
                        return; // persister dropped — shutdown
                    }

                    // Don't allow VCP to run ahead of blocks processor by more than 5 minutes
                    loop {
                        if last_daa_score.saturating_sub(
                            metrics.read().await.components.block_processor.last_block.as_ref().map(|b| b.daa_score).unwrap_or(0),
                        ) < 300 * settings.net_bps as u64
                        {
                            break;
                        }
                        debug!("Virtual chain processor is waiting for block_processor to catch up...");
                        sleep(poll_interval).await;
                        if signal_handler.is_shutdown() {
                            return;
                        }
                    }
                }

                if added_blocks_count < 200 {
                    sleep(poll_interval).await;
                }
            }
            Err(e) => {
                error!("Failed getting virtual chain from start_hash {}: {}", start_hash, e);
                sleep(err_delay).await;
            }
        }
    }
}
