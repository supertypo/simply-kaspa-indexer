use crate::checkpoint::CheckpointBlock;
use crate::settings::Settings;
use bytesize::ByteSize;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use simply_kaspa_database::client::KaspaDbClient;
use simply_kaspa_database::models::query::api::{ApiToccataCovenantMetrics, ApiToccataLaneMetrics, ApiToccataMetrics};
use simply_kaspa_database::models::query::database_details::DatabaseDetails;
use simply_kaspa_database::models::query::table_details::TableDetails;
use std::collections::HashMap;
use std::time::Duration;
use utoipa::ToSchema;

#[derive(ToSchema, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Metrics {
    pub name: String,
    pub version: String,
    pub schema_version: u8,
    pub commit_id: String,
    pub settings: Option<Settings>,
    pub process: MetricsProcess,
    pub queues: MetricsQueues,
    pub checkpoint: MetricsCheckpoint,
    pub components: MetricsComponent,
    pub database: MetricsDb,
    pub toccata: MetricsToccata,
}

impl Metrics {
    pub fn new(name: String, version: String, commit_id: String) -> Self {
        Self {
            name,
            version,
            schema_version: KaspaDbClient::SCHEMA_VERSION,
            commit_id,
            settings: None,
            process: MetricsProcess::new(),
            queues: MetricsQueues::new(),
            checkpoint: MetricsCheckpoint::new(),
            components: MetricsComponent::new(),
            database: MetricsDb::new(),
            toccata: MetricsToccata::new(),
        }
    }
}

#[derive(ToSchema, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricsToccata {
    pub rollup_updated_at: Option<i64>,
    pub tx_version_1: bool,
    pub storage_mass: bool,
    pub compute_budget: bool,
    pub covenant_binding: bool,
    pub utxo_covenant_id: bool,
    pub subnetwork_id: bool,
    pub gas: bool,
    pub get_block_reward_info: bool,
    pub get_seq_commit_lane_proof: bool,
    pub minimum_relay_fee_sompi_per_gram: u64,
    pub tx_v1_count: u64,
    pub block_v2_count: u64,
    pub covenant_tx_count: u64,
    pub covenant_input_count: u64,
    pub covenant_output_count: u64,
    pub covenant_utxo_count: u64,
    pub covenant_id_count: u64,
    pub active_user_lanes: u64,
    pub user_lane_tx_count: u64,
    pub gas_total: u64,
    pub seq_commit_block_count: u64,
    pub storage_mass_max: u64,
    pub storage_mass_avg: u64,
    pub compute_mass_max: u64,
    pub transient_mass_max: u64,
    pub low_fee_rejections: u64,
    pub zk_precompile_tx_count: u64,
    pub groth16_tx_count: u64,
    pub risc0_tx_count: u64,
    pub zk_proof_failures: u64,
    pub bridge_lockbox_count: u64,
    pub bridge_unlock_count: u64,
    pub token_candidate_count: u64,
    pub nft_candidate_count: u64,
    pub lane_proof_failures: u64,
    pub top_covenants: Vec<MetricsToccataCovenant>,
    pub top_lanes: Vec<MetricsToccataLane>,
    pub top_zk_proofs: Vec<MetricsToccataZkProof>,
    pub bridge_lockboxes: Vec<MetricsToccataBridgeLockbox>,
}

impl Default for MetricsToccata {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricsToccata {
    pub fn new() -> Self {
        Self {
            tx_version_1: true,
            storage_mass: false,
            compute_budget: true,
            covenant_binding: true,
            utxo_covenant_id: true,
            subnetwork_id: true,
            gas: false,
            get_block_reward_info: false,
            get_seq_commit_lane_proof: false,
            rollup_updated_at: None,
            minimum_relay_fee_sompi_per_gram: 100,
            tx_v1_count: 0,
            block_v2_count: 0,
            covenant_tx_count: 0,
            covenant_input_count: 0,
            covenant_output_count: 0,
            covenant_utxo_count: 0,
            covenant_id_count: 0,
            active_user_lanes: 0,
            user_lane_tx_count: 0,
            gas_total: 0,
            seq_commit_block_count: 0,
            storage_mass_max: 0,
            storage_mass_avg: 0,
            compute_mass_max: 0,
            transient_mass_max: 0,
            low_fee_rejections: 0,
            zk_precompile_tx_count: 0,
            groth16_tx_count: 0,
            risc0_tx_count: 0,
            zk_proof_failures: 0,
            bridge_lockbox_count: 0,
            bridge_unlock_count: 0,
            token_candidate_count: 0,
            nft_candidate_count: 0,
            lane_proof_failures: 0,
            top_covenants: Vec::new(),
            top_lanes: Vec::new(),
            top_zk_proofs: Vec::new(),
            bridge_lockboxes: Vec::new(),
        }
    }

    pub fn update_from_indexer(&mut self, metrics: ApiToccataMetrics) {
        self.rollup_updated_at = metrics.rollup_updated_at;
        self.tx_v1_count = metrics.tx_v1_count.max(0) as u64;
        self.block_v2_count = metrics.block_v2_count.max(0) as u64;
        self.covenant_tx_count = metrics.covenant_tx_count.max(0) as u64;
        self.covenant_input_count = metrics.covenant_input_count.max(0) as u64;
        self.covenant_output_count = metrics.covenant_output_count.max(0) as u64;
        self.covenant_utxo_count = metrics.covenant_output_count.max(0) as u64;
        self.covenant_id_count = metrics.covenant_id_count.max(0) as u64;
        self.user_lane_tx_count = metrics.user_lane_tx_count.max(0) as u64;
        self.active_user_lanes = metrics.active_user_lanes.max(0) as u64;
        self.seq_commit_block_count = metrics.seq_commit_block_count.max(0) as u64;
        self.top_covenants = metrics.top_covenants.into_iter().map(MetricsToccataCovenant::from).collect();
        self.top_lanes = metrics.top_lanes.into_iter().map(MetricsToccataLane::from).collect();
    }
}

#[derive(ToSchema, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricsToccataCovenant {
    pub covenant_id: String,
    pub tx_count: u64,
    pub utxo_count: u64,
    pub input_count: u64,
    pub output_count: u64,
    pub token_like: bool,
    pub nft_like: bool,
    pub latest_tx_id: Option<String>,
}

impl From<ApiToccataCovenantMetrics> for MetricsToccataCovenant {
    fn from(covenant: ApiToccataCovenantMetrics) -> Self {
        Self {
            covenant_id: covenant.covenant_id,
            tx_count: covenant.tx_count.max(0) as u64,
            utxo_count: covenant.output_count.max(0) as u64,
            input_count: covenant.input_count.max(0) as u64,
            output_count: covenant.output_count.max(0) as u64,
            token_like: false,
            nft_like: covenant.output_count == 1,
            latest_tx_id: covenant.latest_tx_id.map(|tx_id| tx_id.to_string()),
        }
    }
}

#[derive(ToSchema, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricsToccataLane {
    pub lane_key: String,
    pub tx_count: u64,
    pub gas_total: u64,
    pub seq_commit_block_count: u64,
    pub lane_proof_ok: bool,
    pub latest_block_hash: Option<String>,
    pub latest_tx_id: Option<String>,
}

impl From<ApiToccataLaneMetrics> for MetricsToccataLane {
    fn from(lane: ApiToccataLaneMetrics) -> Self {
        Self {
            lane_key: lane.lane_key,
            tx_count: lane.tx_count.max(0) as u64,
            gas_total: 0,
            seq_commit_block_count: 0,
            lane_proof_ok: false,
            latest_block_hash: None,
            latest_tx_id: lane.latest_tx_id.map(|tx_id| tx_id.to_string()),
        }
    }
}

#[derive(ToSchema, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricsToccataZkProof {
    pub proof_type: String,
    pub tx_count: u64,
    pub failure_count: u64,
    pub latest_tx_id: Option<String>,
}

#[derive(ToSchema, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricsToccataBridgeLockbox {
    pub label: String,
    pub covenant_id: Option<String>,
    pub locked_amount_sompi: u64,
    pub unlock_tx_count: u64,
    pub proof_type: String,
    pub latest_tx_id: Option<String>,
}

#[derive(ToSchema, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricsProcess {
    #[schema(example = "5.1")]
    pub cpu_used_percent: f32,
    #[schema(example = "26284032")]
    pub memory_used: u64,
    #[schema(example = "26.3 MB")]
    pub memory_used_pretty: Option<String>,
    #[schema(example = "9177317376")]
    pub memory_free: u64,
    #[schema(example = "9.2 GB")]
    pub memory_free_pretty: Option<String>,
    pub uptime: u64,
    #[schema(example = "43m 35s")]
    pub uptime_pretty: Option<String>,
}

impl Default for MetricsProcess {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricsProcess {
    pub fn new() -> Self {
        Self {
            cpu_used_percent: 0.0,
            memory_used: 0,
            memory_used_pretty: None,
            memory_free: 0,
            memory_free_pretty: None,
            uptime: 0,
            uptime_pretty: None,
        }
    }
}

#[derive(ToSchema, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricsQueues {
    #[schema(example = "57")]
    pub blocks: u64,
    #[schema(example = "1000")]
    pub blocks_capacity: u64,
    #[schema(example = "225")]
    pub transactions: u64,
    #[schema(example = "20000")]
    pub transactions_capacity: u64,
}

impl Default for MetricsQueues {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricsQueues {
    pub fn new() -> Self {
        Self { blocks: 0, blocks_capacity: 0, transactions: 0, transactions_capacity: 0 }
    }
}

#[derive(ToSchema, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricsCheckpoint {
    #[schema(example = "Vcp")]
    pub origin: Option<String>,
    pub block: Option<MetricsBlock>,
}

impl Default for MetricsCheckpoint {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricsCheckpoint {
    pub fn new() -> Self {
        Self { origin: None, block: None }
    }
}

#[derive(ToSchema, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricsComponent {
    pub block_fetcher: MetricsComponentBlockFetcher,
    pub block_processor: MetricsComponentBlockProcessor,
    pub transaction_processor: MetricsComponentTransactionProcessor,
    pub virtual_chain_processor: MetricsComponentVirtualChainProcessor,
    pub db_pruner: MetricsComponentDbPruner,
}

impl Default for MetricsComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricsComponent {
    pub fn new() -> Self {
        Self {
            block_fetcher: MetricsComponentBlockFetcher::new(),
            block_processor: MetricsComponentBlockProcessor::new(),
            transaction_processor: MetricsComponentTransactionProcessor::new(),
            virtual_chain_processor: MetricsComponentVirtualChainProcessor::new(),
            db_pruner: MetricsComponentDbPruner::new(),
        }
    }
}

#[derive(ToSchema, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricsComponentBlockFetcher {
    pub last_block: Option<MetricsBlock>,
}

impl Default for MetricsComponentBlockFetcher {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricsComponentBlockFetcher {
    pub fn new() -> Self {
        Self { last_block: None }
    }

    pub fn update_last_block(&mut self, last_block: MetricsBlock) {
        if self.last_block.as_ref().map(|b| b.daa_score < last_block.daa_score).unwrap_or(true) {
            self.last_block = Some(last_block);
        }
    }
}

#[derive(ToSchema, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricsComponentBlockProcessor {
    pub last_block: Option<MetricsBlock>,
}

impl Default for MetricsComponentBlockProcessor {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricsComponentBlockProcessor {
    pub fn new() -> Self {
        Self { last_block: None }
    }

    pub fn update_last_block(&mut self, last_block: MetricsBlock) {
        if self.last_block.as_ref().map(|b| b.daa_score < last_block.daa_score).unwrap_or(true) {
            self.last_block = Some(last_block);
        }
    }
}

#[derive(ToSchema, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricsComponentTransactionProcessor {
    pub enabled: bool,
    pub last_block: Option<MetricsBlock>,
}

impl Default for MetricsComponentTransactionProcessor {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricsComponentTransactionProcessor {
    pub fn new() -> Self {
        Self { enabled: false, last_block: None }
    }

    pub fn update_last_block(&mut self, last_block: MetricsBlock) {
        if self.last_block.as_ref().map(|b| b.daa_score < last_block.daa_score).unwrap_or(true) {
            self.last_block = Some(last_block);
        }
    }
}

#[derive(ToSchema, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricsComponentVirtualChainProcessor {
    pub enabled: bool,
    pub only_blocks: bool,
    #[schema(example = "6")]
    pub tip_distance: Option<u64>,
    #[schema(example = "1738706345528")]
    pub tip_distance_timestamp: Option<u64>,
    #[schema(example = "2025-04-03T22:47:33.938Z")]
    pub tip_distance_date_time: Option<DateTime<Utc>>,
    pub last_block: Option<MetricsBlock>,
}

impl Default for MetricsComponentVirtualChainProcessor {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricsComponentVirtualChainProcessor {
    pub fn new() -> Self {
        Self {
            enabled: false,
            only_blocks: false,
            tip_distance: None,
            tip_distance_timestamp: None,
            tip_distance_date_time: None,
            last_block: None,
        }
    }

    pub fn update_last_block(&mut self, last_block: MetricsBlock) {
        if self.last_block.as_ref().map(|b| b.daa_score < last_block.daa_score).unwrap_or(true) {
            self.last_block = Some(last_block);
        }
    }
}

#[derive(ToSchema, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricsComponentDbPruner {
    pub enabled: bool,
    pub cron: Option<String>,
    pub running: Option<bool>,
    pub start_time: Option<DateTime<Utc>>,
    pub retention: Option<HashMap<String, Option<String>>>,
    pub results: Option<HashMap<String, MetricsComponentDbPrunerResult>>,
    pub completed_time: Option<DateTime<Utc>>,
    pub completed_successfully: Option<bool>,
}

impl Default for MetricsComponentDbPruner {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricsComponentDbPruner {
    pub fn new() -> Self {
        Self {
            enabled: false,
            cron: None,
            running: None,
            start_time: None,
            retention: None,
            results: None,
            completed_time: None,
            completed_successfully: None,
        }
    }
}

#[derive(ToSchema, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricsComponentDbPrunerResult {
    pub start_time: DateTime<Utc>,
    pub cutoff_time: DateTime<Utc>,
    pub success: Option<bool>,
    #[serde(with = "humantime_serde")]
    pub duration: Option<Duration>,
    pub rows_deleted: Option<u64>,
}

#[derive(ToSchema, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricsDb {
    #[schema(example = "postgres")]
    pub database_name: Option<String>,
    #[schema(example = "public")]
    pub schema_name: Option<String>,
    #[schema(example = "1901425123")]
    pub database_size: Option<u64>,
    #[schema(example = "1.9 GB")]
    pub database_size_pretty: Option<String>,
    #[schema(example = "13")]
    pub active_queries: Option<u64>,
    #[schema(example = "0")]
    pub blocked_queries: Option<u64>,
    #[schema(example = "7")]
    pub active_connections: Option<u64>,
    #[schema(example = "100")]
    pub max_connections: Option<u64>,
    pub tables: Option<Vec<MetricsDbTable>>,
}

impl Default for MetricsDb {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricsDb {
    pub fn new() -> Self {
        Self {
            database_name: None,
            schema_name: None,
            database_size: None,
            database_size_pretty: None,
            active_queries: None,
            blocked_queries: None,
            active_connections: None,
            max_connections: None,
            tables: None,
        }
    }
}

impl From<DatabaseDetails> for MetricsDb {
    fn from(database_details: DatabaseDetails) -> Self {
        Self {
            database_name: Some(database_details.database_name),
            schema_name: Some(database_details.schema_name),
            database_size: Some(database_details.database_size as u64),
            database_size_pretty: Some(ByteSize(database_details.database_size as u64).to_string()),
            active_queries: Some(database_details.active_queries as u64),
            blocked_queries: Some(database_details.blocked_queries as u64),
            active_connections: Some(database_details.active_connections as u64),
            max_connections: Some(database_details.max_connections as u64),
            tables: None,
        }
    }
}

#[derive(ToSchema, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricsDbTable {
    #[schema(example = "addresses_transactions")]
    pub name: String,
    #[schema(example = "394731520")]
    pub total_size: u64,
    #[schema(example = "394.7 MB")]
    pub total_size_pretty: String,
    #[schema(example = "229638144")]
    pub indexes_size: u64,
    #[schema(example = "229.6 MB")]
    pub indexes_size_pretty: String,
    #[schema(example = "1213732")]
    pub approximate_row_count: u64,
}

impl From<TableDetails> for MetricsDbTable {
    fn from(table_details: TableDetails) -> Self {
        Self {
            name: table_details.name,
            total_size: table_details.total_size as u64,
            total_size_pretty: ByteSize(table_details.total_size as u64).to_string(),
            indexes_size: table_details.indexes_size as u64,
            indexes_size_pretty: ByteSize(table_details.indexes_size as u64).to_string(),
            approximate_row_count: table_details.approximate_row_count as u64,
        }
    }
}

#[derive(ToSchema, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricsBlock {
    #[schema(example = "f47db1a79f707fc139bdbefc98b4859217a6922b42acb7b552d9021fea2e7800")]
    pub hash: String,
    #[schema(example = "1738706345528")]
    pub timestamp: u64,
    #[schema(example = "2025-02-04T21:59:05.528Z")]
    pub date_time: DateTime<Utc>,
    #[schema(example = "102414204")]
    pub daa_score: u64,
    #[schema(example = "100804248")]
    pub blue_score: u64,
}

impl From<CheckpointBlock> for MetricsBlock {
    fn from(checkpoint_block: CheckpointBlock) -> Self {
        Self {
            hash: checkpoint_block.hash.to_string(),
            timestamp: checkpoint_block.timestamp,
            date_time: DateTime::from_timestamp_millis(checkpoint_block.timestamp as i64).unwrap(),
            daa_score: checkpoint_block.daa_score,
            blue_score: checkpoint_block.blue_score,
        }
    }
}
