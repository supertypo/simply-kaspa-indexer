use crate::settings::Settings;
use crate::vars::save_block_checkpoint;
use crate::web::model::metrics::Metrics;
use crossbeam_queue::ArrayQueue;
use log::{debug, error, info, warn};
use simply_kaspa_cli::cli_args::CliDisable;
use simply_kaspa_database::client::KaspaDbClient;
use simply_kaspa_database::models::types::hash::Hash as SqlHash;
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tokio::time::sleep;

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum CheckpointOrigin {
    Blocks,
    Transactions,
    Vcp,
    Initial, // Only set at startup, not used for checkpoint processing
}

#[derive(Clone)]
pub struct CheckpointBlock {
    pub origin: CheckpointOrigin,
    pub hash: SqlHash,
    pub timestamp: u64,
    pub daa_score: u64,
    pub blue_score: u64,
}

pub async fn process_checkpoints(
    settings: Settings,
    run: Arc<AtomicBool>,
    metrics: Arc<RwLock<Metrics>>,
    checkpoint_queue: Arc<ArrayQueue<CheckpointBlock>>,
    database: KaspaDbClient,
) {
    let disable_transaction_processing = settings.cli_args.is_disabled(CliDisable::TransactionProcessing);

    const CHECKPOINT_SAVE_INTERVAL: u64 = 60;
    const CHECKPOINT_WARN_INTERVAL: u64 = 60;
    const CHECKPOINT_FAILED_TIMEOUT: u64 = 600;
    let mut checkpoint_last_saved = Instant::now();
    let mut checkpoint_last_warned = Instant::now();
    let mut checkpoint_candidate = None;

    let mut txs_processed: HashSet<SqlHash> = HashSet::new();

    let mut cp_ok_txs: bool = false;

    while run.load(Ordering::Relaxed) {
        if let Some(checkpoint_block) = checkpoint_queue.pop() {
            match checkpoint_block.origin {
                CheckpointOrigin::Blocks => {
                    if checkpoint_candidate.is_none()
                        && Instant::now().duration_since(checkpoint_last_saved).as_secs() > CHECKPOINT_SAVE_INTERVAL
                    {
                        debug!("Selected block_checkpoint candidate {}", hex::encode(checkpoint_block.hash.as_bytes()));
                        checkpoint_candidate = Some(checkpoint_block);
                        checkpoint_last_warned = Instant::now();
                    }
                }
                CheckpointOrigin::Transactions => {
                    txs_processed.insert(checkpoint_block.hash);
                }
                _ => {}
            }
            if let Some(checkpoint) = checkpoint_candidate {
                let checkpoint_string = hex::encode(checkpoint.hash.as_bytes());
                if !cp_ok_txs && (disable_transaction_processing || txs_processed.contains(&checkpoint.hash)) {
                    cp_ok_txs = true;
                    txs_processed = HashSet::new();
                }
                if cp_ok_txs {
                    info!("Saving block checkpoint {}", checkpoint_string);
                    save_block_checkpoint(&checkpoint_string, &database).await.unwrap();
                    let mut metrics = metrics.write().await;
                    metrics.block_checkpoint = Some(checkpoint.into());
                    checkpoint_last_saved = Instant::now();
                    checkpoint_candidate = None;
                } else if Instant::now().duration_since(checkpoint_last_warned).as_secs() > CHECKPOINT_WARN_INTERVAL {
                    warn!("Still unable to save block checkpoint {}", checkpoint_string);
                    checkpoint_last_warned = Instant::now();
                    checkpoint_candidate = Some(checkpoint);
                } else if Instant::now().duration_since(checkpoint_last_saved).as_secs() > CHECKPOINT_FAILED_TIMEOUT {
                    // Failsafe in the unlikely scenario that blocks is more than CHECKPOINT_SAVE_INTERVAL behind tx processing.
                    // Selecting a new candidate without clearing the processed hashmaps should allow it to eventually succeed,
                    // although it will be at the expense of increased memory use.
                    error!("Failed to synchronize on block checkpoint {}", checkpoint_string);
                    checkpoint_last_saved = Instant::now(); // Need to reset this to avoid a loop
                    checkpoint_candidate = None;
                } else {
                    checkpoint_candidate = Some(checkpoint);
                }
            }
        } else {
            sleep(Duration::from_millis(100)).await;
        }
    }
}
