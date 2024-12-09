use clap::Parser;

#[derive(Parser, Clone, Debug)]
pub struct CliArgs {
    #[clap(short = 's', long, help = "The url to a kaspad instance, e.g 'ws://localhost:17110'. Leave empty to use the Kaspa PNN")]
    pub rpc_url: Option<String>,
    #[clap(short = 'n', long, default_value = "mainnet", help = "The network type and suffix, e.g. 'testnet-11'")]
    pub network: String,
    #[clap(
        short = 'd',
        long,
        default_value = "postgres://postgres:postgres@localhost:5432/postgres",
        help = "The url to a PostgreSQL instance"
    )]
    pub database_url: String,
    #[clap(short = 'l', long, default_value = "info", help = "error, warn, info, debug, trace, off")]
    pub log_level: String,
    #[clap(long, help = "Disable colored output")]
    pub log_no_color: bool,
    #[clap(
        short = 'b',
        long,
        default_value = "1.0",
        help = "Batch size factor [0.1-10]. Adjusts internal queues and database batch sizes"
    )]
    pub batch_scale: f64,
    #[clap(
        short = 't',
        long,
        default_value = "60",
        help = "Cache ttl (in seconds). Adjusts tx and block caches for in-memory duplicate detection"
    )]
    pub cache_ttl: u64,
    #[clap(
        short = 'i',
        long,
        help = "Ignore checkpoint and start from a specified block hash, 'p' for pruning point or 'v' for virtual"
    )]
    pub ignore_checkpoint: Option<String>,
    #[clap(long, help = "Start VCP as soon as the filler has passed the previous run")]
    pub vcp_before_synced: bool,
    #[clap(long, help = "Reduces database load by not tracking an address's transactions")]
    pub skip_resolving_addresses: bool,
    #[clap(short = 'x', long, help = "Save the following extra data to the db: transaction.payload")]
    pub extra_data: bool,
    #[clap(short = 'u', long, help = "Auto-upgrades older db schemas. Use with care")]
    pub upgrade_db: bool,
    #[clap(short = 'c', long, help = "(Re-)initializes the database schema. Use with care")]
    pub initialize_db: bool,
}
