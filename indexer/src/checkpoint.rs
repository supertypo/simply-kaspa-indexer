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
