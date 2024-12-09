use clap::Parser;

#[derive(Parser, Clone, Debug)]
pub struct CliArgs {
    #[clap(short = 's', long, help = "The url to a kaspad instance, e.g 'ws://localhost:17110'. Leave empty to use the Kaspa PNN")]
    pub rpc_url: Option<String>,
    #[clap(short, long, default_value = "mainnet", help = "The network type and suffix, e.g. 'testnet-11'")]
    pub network: String,
    #[clap(short, long, default_value = "postgres://postgres:postgres@localhost:5432/postgres", help = "PostgreSQL url")]
    pub database_url: String,
    #[clap(short, long, default_value = "info", help = "error, warn, info, debug, trace, off")]
    pub log_level: String,
    #[clap(long, help = "Disable colored output")]
    pub log_no_color: bool,
    #[clap(short, long, default_value = "1.0", help = "Batch size factor [0.1-10]. Adjusts internal queues and database batch sizes")]
    pub batch_scale: f64,
    #[clap(short = 't', long, default_value = "60", help = "Cache ttl (secs). Adjusts tx/block caches for in-memory de-duplication")]
    pub cache_ttl: u64,
    #[clap(short, long, help = "Ignore checkpoint and start from a specified block, 'p' for pruning point or 'v' for virtual")]
    pub ignore_checkpoint: Option<String>,
    #[clap(long, help = "Start VCP as soon as the filler has passed the previous run. Use with care")]
    pub vcp_before_synced: bool,
    #[clap(long, help = "Reduces database load by not tracking an address's transactions in a separate table")]
    pub skip_resolving_addresses: bool,
    #[clap(long, help = "Reduces database load by not tracking block relations in a separate table")]
    pub skip_block_relations: bool,
    #[clap(short, long, help = "Auto-upgrades older db schemas. Use with care")]
    pub upgrade_db: bool,
    #[clap(short = 'c', long, help = "(Re-)initializes the database schema. Use with care")]
    pub initialize_db: bool,
    #[clap(
        long,
        help = "Exclude specific (non-required) fields, e.g 'block.version,tx.payload'.
        If include_fields is specified this argument is ignored.",
        use_value_delimiter = true
    )]
    pub exclude_fields: Option<Vec<String>>,
    #[clap(
        long,
        help = "Only include specific (non-required) fields, e.g 'block.nonce,tx.mass,tx_in.sig_op_count' or 'none'.
        Be aware that the required fields can change, so take care when upgrading and specify every field you need.",
        use_value_delimiter = true
    )]
    pub include_fields: Option<Vec<String>>,
}
