use crate::checkpoint::{CheckpointBlock, CheckpointOrigin};
use crate::settings::Settings;
use crate::virtual_chain::accept_transactions::accept_transactions;
use crate::virtual_chain::add_chain_blocks::add_chain_blocks;
use crate::virtual_chain::remove_chain_blocks::remove_chain_blocks;
use crate::web::model::metrics::Metrics;
use chrono::DateTime;
use crossbeam_queue::ArrayQueue;
use kaspa_rpc_core::GetVirtualChainFromBlockV2Response;
use log::{debug, info, warn};
use mpsc::Receiver;
use simply_kaspa_cli::cli_args::CliDisable;
use simply_kaspa_database::client::KaspaDbClient;
use simply_kaspa_mapping::mapper::KaspaDbMapper;
use simply_kaspa_signal::signal_handler::SignalHandler;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tokio::sync::mpsc;
use tokio::time::sleep;

pub async fn process_virtual_chain(
    settings: Settings,
    signal_handler: SignalHandler,
    metrics: Arc<RwLock<Metrics>>,
    checkpoint_queue: Arc<ArrayQueue<CheckpointBlock>>,
    database: KaspaDbClient,
    mapper: KaspaDbMapper,
    mut receiver: Receiver<GetVirtualChainFromBlockV2Response>,
) {
    let batch_scale = settings.cli_args.batch_scale;
    let batch_concurrency = settings.cli_args.batch_concurrency;
    let disable_transaction_acceptance = settings.cli_args.is_disabled(CliDisable::TransactionAcceptance);

    let mut subnetwork_map = HashMap::new();
    let results = database.select_subnetworks().await.expect("Select subnetworks FAILED");
    for s in results {
        subnetwork_map.insert(s.subnetwork_id, s.id);
    }

    let mut tip_distance: u64 = 10;
    let mut tip_distance_timestamp: u128 = 0;
    let mut tip_distance_history: VecDeque<bool> = VecDeque::new();
    let tip_distance_window = (settings.cli_args.vcp_window * 1_000 / settings.cli_args.vcp_interval).max(1) as usize;

    let start_time = Instant::now();
    let mut synced = false;

    while let Some(res) = receiver.recv().await {
        let added_blocks_count = res.added_chain_block_hashes.len();
        let last_header = &res.chain_block_accepted_transactions.last().unwrap().chain_block_header;
        let checkpoint_block = CheckpointBlock {
            origin: CheckpointOrigin::Vcp,
            hash: last_header.hash.unwrap().into(),
            timestamp: last_header.timestamp.unwrap(),
            daa_score: last_header.daa_score.unwrap(),
            blue_score: last_header.blue_score.unwrap(),
        };

        let start_commit_time = Instant::now();
        let rows_removed = remove_chain_blocks(batch_scale, &res.removed_chain_block_hashes, &database).await;

        let has_reorg = rows_removed > 0;
        if tip_distance_history.len() == tip_distance_window {
            tip_distance_history.pop_back();
        }
        tip_distance_history.push_front(has_reorg);
        let reorgs_count = tip_distance_history.iter().filter(|&&x| x).count();
        if reorgs_count >= 3 {
            tip_distance += 1;
            tip_distance_timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis();
            // Increase distance if new reorgs occur within the window:
            tip_distance_history.pop_front();
            tip_distance_history.push_front(false);
            debug!("Increased vcp tip distance to {tip_distance}");
        } else if added_blocks_count < 200 && reorgs_count == 0 && tip_distance > 0 {
            tip_distance -= 1;
            tip_distance_timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis();
            if tip_distance_history.len() == tip_distance_window {
                // Make sure we don't decrease distance again until a complete window has passed:
                tip_distance_history.pop_front();
                tip_distance_history.push_front(true);
            }
            debug!("Decreased vcp tip distance to {tip_distance}");
        }

        if !disable_transaction_acceptance {
            let (rows_affected_tx_acc, rows_affected_tx, rows_affected_tx_addr) = accept_transactions(
                batch_scale,
                batch_concurrency,
                &settings,
                &res.chain_block_accepted_transactions,
                &database,
                &mapper,
                &mut subnetwork_map,
            )
            .await;
            let commit_time = Instant::now().duration_since(start_commit_time).as_millis();
            let tps = rows_affected_tx as f64 / commit_time as f64 * 1000f64;
            info!(
                "Committed {} accepted{} txs in {}ms ({:.1} tps, {} adr_tx). Last tx: {}",
                rows_affected_tx_acc,
                if rows_removed > 0 { format!(" and {} rejected", rows_removed) } else { String::new() },
                commit_time,
                tps,
                rows_affected_tx_addr,
                chrono::DateTime::from_timestamp_millis(checkpoint_block.timestamp as i64 / 1000 * 1000).unwrap()
            );
        } else {
            let rows_added = add_chain_blocks(batch_scale, &res.added_chain_block_hashes, &database).await;
            info!(
                "Committed {} added and {} removed chain blocks in {}ms. Last added: {}",
                rows_added,
                rows_removed,
                Instant::now().duration_since(start_commit_time).as_millis(),
                chrono::DateTime::from_timestamp_millis(checkpoint_block.timestamp as i64 / 1000 * 1000).unwrap()
            );
        }
        {
            let mut m = metrics.write().await;
            m.components.virtual_chain_processor.update_last_block(checkpoint_block.clone().into());
            m.components.virtual_chain_processor.tip_distance = Some(tip_distance);
            m.components.virtual_chain_processor.tip_distance_timestamp = Some(tip_distance_timestamp as u64);
            m.components.virtual_chain_processor.tip_distance_date_time =
                DateTime::from_timestamp_millis(tip_distance_timestamp as i64);
        }

        while checkpoint_queue.push(checkpoint_block.clone()).is_err() {
            warn!("Checkpoint queue is full");
            sleep(Duration::from_secs(1)).await;
            if signal_handler.is_shutdown() {
                return;
            }
        }

        if !synced && added_blocks_count < 200 {
            let time_to_sync = start_time.elapsed();
            info!(
                "\x1b[32mVirtual chain processor synced! (in {}:{:0>2}:{:0>2}s)\x1b[0m",
                time_to_sync.as_secs() / 3600,
                time_to_sync.as_secs() % 3600 / 60,
                time_to_sync.as_secs() % 60
            );
            synced = true;
        }
    }
}
