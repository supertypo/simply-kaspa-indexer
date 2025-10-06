use clap::builder::TypedValueParser;
use clap::error::{Error, ErrorKind};
use clap::{Args, Parser, ValueEnum};
use serde::{Deserialize, Serialize};
use std::ffi::OsStr;
use std::time::Duration;
use utoipa::ToSchema;

#[derive(Clone, Debug, PartialEq, Eq, ValueEnum, ToSchema, Serialize, Deserialize)]
#[clap(rename_all = "snake_case")]
pub enum CliEnable {
    None,
    /// Enables dynamic VCP tip distance, reduces write load due to reorgs
    DynamicVcpTipDistance,
    /// Enables resolving transactions_inputs previous_outpoint
    TransactionsInputsResolve,
    /// Forces (pruning point) utxo set import on startup (otherwise only on empty db)
    ForceUtxoImport,
}

#[derive(Clone, Debug, PartialEq, Eq, ValueEnum, ToSchema, Serialize, Deserialize)]
#[clap(rename_all = "snake_case")]
pub enum CliDisable {
    None,
    /// Disables the virtual chain processor / the transactions_acceptances table
    VirtualChainProcessing,
    /// Disables transaction acceptance, marks chain blocks as long as VCP is not disabled
    TransactionAcceptance,
    /// Disables transaction processing / all transaction related tables
    TransactionProcessing,
    /// Disables the blocks table
    BlocksTable,
    /// Disables the block_parent table
    BlockParentTable,
    /// Disables the blocks_transactions table
    BlocksTransactionsTable,
    /// Disables the transactions table
    TransactionsTable,
    /// Disables the transactions_inputs table
    TransactionsInputsTable,
    /// Disables the transactions_outputs table
    TransactionsOutputsTable,
    /// Disables the addresses_transactions (or scripts_transactions) table
    AddressesTransactionsTable,
    /// Disables initial utxo set import
    InitialUtxoImport,
    /// Start VCP as soon as the filler has passed the previous run. Use with care
    VcpWaitForSync,
}

#[derive(Clone, Debug, PartialEq, Eq, ValueEnum, ToSchema, Serialize, Deserialize)]
#[clap(rename_all = "snake_case")]
pub enum CliField {
    None,
    BlockAcceptedIdMerkleRoot,
    BlockMergeSetBluesHashes,
    BlockMergeSetRedsHashes,
    BlockSelectedParentHash,
    BlockBits,
    BlockBlueWork,
    /// Used for sorting blocks
    BlockBlueScore,
    BlockDaaScore,
    BlockHashMerkleRoot,
    BlockNonce,
    BlockPruningPoint,
    BlockTimestamp,
    BlockUtxoCommitment,
    BlockVersion,
    /// Used for identifying tx type (coinbase/regular)
    TxSubnetworkId,
    TxHash,
    TxMass,
    TxPayload,
    /// Used for sorting transactions
    TxBlockTime,
    /// Used for identifying wallet address of sender
    TxInPreviousOutpoint,
    TxInSignatureScript,
    TxInSigOpCount,
    /// Excluding this will increase load for populating adress-/scripts_transactions
    TxInBlockTime,
    TxOutAmount,
    /// Excluding both this and script_public_key_address will disable adress-/scripts_transactions
    TxOutScriptPublicKey,
    /// Excluding this, scripts_transactions to be populated instead of adresses_transactions
    TxOutScriptPublicKeyAddress,
    TxOutBlockTime,
}

#[derive(Parser, Clone, Debug, ToSchema, Serialize, Deserialize)]
#[command(name = "simply-kaspa-indexer", version = env!("VERGEN_GIT_DESCRIBE"))]
#[serde(rename_all = "camelCase")]
pub struct CliArgs {
    #[clap(short = 's', long, help = "RPC url to a kaspad instance, e.g 'ws://localhost:17110'. Leave empty to use the Kaspa PNN")]
    pub rpc_url: Option<String>,
    #[clap(short = 'p', long, help = "P2P socket address to a kaspad instance, e.g 'localhost:16111'.")]
    pub p2p_url: Option<String>,
    #[clap(short, long, default_value = "mainnet", help = "The network type and suffix, e.g. 'testnet-11'")]
    pub network: String,
    #[clap(short, long, default_value = "postgres://postgres:postgres@localhost:5432/postgres", help = "PostgreSQL url")]
    pub database_url: String,
    #[clap(short, long, default_value = "localhost:8500", help = "Web server socket address")]
    pub listen: String,
    #[clap(long, default_value = "/", help = "Web server base path")]
    pub base_path: String,
    #[clap(long, default_value = "info", help = "error, warn, info, debug, trace, off")]
    pub log_level: String,
    #[clap(long, help = "Disable colored output")]
    pub log_no_color: bool,
    #[clap(short, long, default_value = "1.0", help = "Batch size factor [0.1-10]. Adjusts internal queues and database batch sizes")]
    pub batch_scale: f64,
    #[clap(long, default_value = "2", help = "Batch concurrency factor [1-10]. Per table batch concurrency")]
    pub batch_concurrency: i8,
    #[clap(short = 't', long, default_value = "60", help = "Cache ttl (secs). Adjusts tx/block caches for in-memory de-duplication")]
    pub cache_ttl: u64,
    #[clap(long, default_value = "1000", value_parser = clap::value_parser!(u64).range(100..=10000), help = "Poll interval for blocks (ms)")]
    pub block_interval: u64,
    #[clap(long, default_value = "1000", value_parser = clap::value_parser!(u64).range(100..=10000), help = "Poll interval for vcp (ms)")]
    pub vcp_interval: u64,
    #[clap(long, default_value = "600", value_parser = clap::value_parser!(u64).range(10..=86400), help = "Window size for automatic vcp tip distance adjustment (in seconds)")]
    pub vcp_window: u64,
    #[clap(short, long, help = "Ignore checkpoint and start from a specified block, 'p' for pruning point or 'v' for virtual")]
    pub ignore_checkpoint: Option<String>,
    #[clap(short, long, help = "Auto-upgrades older db schemas. Use with care")]
    pub upgrade_db: bool,
    #[clap(short = 'c', long, help = "(Re-)initializes the database schema. Use with care")]
    pub initialize_db: bool,
    #[clap(flatten)]
    pub pruning: PruningConfig,
    #[clap(long, help = "Enable optional functionality", value_enum, use_value_delimiter = true)]
    pub enable: Option<Vec<CliEnable>>,
    #[clap(long, help = "Disable specific functionality", value_enum, use_value_delimiter = true)]
    pub disable: Option<Vec<CliDisable>>,
    #[clap(
        long,
        help = "Exclude specific fields. If include_fields is specified this argument is ignored.",
        value_enum,
        use_value_delimiter = true
    )]
    pub exclude_fields: Option<Vec<CliField>>,
}

impl CliArgs {
    pub fn is_enabled(&self, feature: CliEnable) -> bool {
        self.enable.as_ref().is_some_and(|enable| enable.contains(&feature))
    }

    pub fn is_disabled(&self, feature: CliDisable) -> bool {
        self.disable.as_ref().is_some_and(|disable| disable.contains(&feature))
    }

    pub fn is_excluded(&self, field: CliField) -> bool {
        if let Some(exclude_fields) = self.exclude_fields.clone() { exclude_fields.contains(&field) } else { false }
    }

    pub fn version(&self) -> String {
        env!("VERGEN_GIT_DESCRIBE").to_string()
    }

    pub fn commit_id(&self) -> String {
        env!("VERGEN_GIT_SHA").to_string()
    }
}

#[derive(Debug, Clone, Args, ToSchema, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PruningConfig {
    #[clap(long, default_missing_value = "0 4 * * *", num_args = 0..=1, help = "Enables db pruning. Optional cron expression. Default: '0 4 * * *' = daily 04:00 (UTC)")]
    pub prune_db: Option<String>,
    #[clap(long, default_value = "100000", help = "Batch size for db pruning")]
    pub prune_batch_size: i32,
    #[clap(long, value_parser = HumantimeDurationParser, help = "Global data retention for db pruning. Ex: 60d, 24h, etc")]
    #[serde(with = "humantime_serde")]
    pub retention: Option<Duration>,
    #[clap(long, value_parser = HumantimeDurationParser, help = "Retention for block_parent table")]
    #[serde(with = "humantime_serde")]
    pub retention_block_parent: Option<Duration>,
    #[clap(long, value_parser = HumantimeDurationParser, help = "Retention for blocks_transactions table")]
    #[serde(with = "humantime_serde")]
    pub retention_blocks_transactions: Option<Duration>,
    #[clap(long, value_parser = HumantimeDurationParser, help = "Retention for blocks table")]
    #[serde(with = "humantime_serde")]
    pub retention_blocks: Option<Duration>,
    #[clap(long, value_parser = HumantimeDurationParser, help = "Retention for transactions_* tables")]
    #[serde(with = "humantime_serde")]
    pub retention_transactions: Option<Duration>,
    #[clap(long, value_parser = HumantimeDurationParser, help = "Retention for addresses_transactions, scripts_transactions tables")]
    #[serde(with = "humantime_serde")]
    pub retention_addresses_transactions: Option<Duration>,
}

impl PruningConfig {
    pub fn resolve(&self, specific: Option<Duration>) -> Option<Duration> {
        specific.or(self.retention)
    }

    pub fn resolved(mut self) -> Self {
        self.retention_block_parent = self.resolve(self.retention_block_parent);
        self.retention_blocks_transactions = self.resolve(self.retention_blocks_transactions);
        self.retention_blocks = self.resolve(self.retention_blocks);
        self.retention_transactions = self.resolve(self.retention_transactions);
        self.retention_addresses_transactions = self.resolve(self.retention_addresses_transactions);
        self
    }
}

#[derive(Clone)]
pub struct HumantimeDurationParser;

impl TypedValueParser for HumantimeDurationParser {
    type Value = Duration;

    fn parse_ref(&self, _cmd: &clap::Command, _arg: Option<&clap::Arg>, raw: &OsStr) -> Result<Self::Value, clap::Error> {
        let input = raw.to_string_lossy();
        humantime::parse_duration(&input)
            .map_err(|err| Error::raw(ErrorKind::ValueValidation, format!("Invalid duration '{}': {}", input, err)))
    }
}
