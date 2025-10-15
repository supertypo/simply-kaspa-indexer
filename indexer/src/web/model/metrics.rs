use crate::checkpoint::CheckpointBlock;
use crate::settings::Settings;
use bigdecimal::ToPrimitive;
use bytesize::ByteSize;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
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
    pub commit_id: String,
    pub settings: Option<Settings>,
    pub process: MetricsProcess,
    pub queues: MetricsQueues,
    pub checkpoint: MetricsCheckpoint,
    pub components: MetricsComponent,
    pub database: MetricsDb,
}

impl Metrics {
    pub fn new(name: String, version: String, commit_id: String) -> Self {
        Self {
            name,
            version,
            commit_id,
            settings: None,
            process: MetricsProcess::new(),
            queues: MetricsQueues::new(),
            checkpoint: MetricsCheckpoint::new(),
            components: MetricsComponent::new(),
            database: MetricsDb::new(),
        }
    }
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
    pub utxo_importer: MetricsComponentUtxoSetImporter,
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
            utxo_importer: MetricsComponentUtxoSetImporter::new(),
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
pub struct MetricsComponentUtxoSetImporter {
    pub enabled: bool,
    pub completed: Option<bool>,
    pub utxos_imported: Option<u64>,
    pub acceptances_committed: Option<u64>,
    pub outputs_committed: Option<u64>,
}

impl Default for MetricsComponentUtxoSetImporter {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricsComponentUtxoSetImporter {
    pub fn new() -> Self {
        Self { enabled: false, completed: None, utxos_imported: None, acceptances_committed: None, outputs_committed: None }
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
            database_size: Some(database_details.database_size.to_u64().unwrap_or(0)),
            database_size_pretty: Some(ByteSize(database_details.database_size.to_u64().unwrap_or(0)).to_string()),
            active_queries: Some(database_details.active_queries.to_u64().unwrap_or(0)),
            blocked_queries: Some(database_details.blocked_queries.to_u64().unwrap_or(0)),
            active_connections: Some(database_details.active_connections.to_u64().unwrap_or(0)),
            max_connections: Some(database_details.max_connections.to_u64().unwrap_or(0)),
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
            total_size: table_details.total_size.to_u64().unwrap_or(0),
            total_size_pretty: ByteSize(table_details.total_size.to_u64().unwrap_or(0)).to_string(),
            indexes_size: table_details.indexes_size.to_u64().unwrap_or(0),
            indexes_size_pretty: ByteSize(table_details.indexes_size.to_u64().unwrap_or(0)).to_string(),
            approximate_row_count: table_details.approximate_row_count.to_u64().unwrap_or(0),
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
