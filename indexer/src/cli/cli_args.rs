use clap::{crate_description, crate_name, Arg, ArgMatches, Command};
use std::time::Duration;

#[derive(Clone)]
pub struct CliArgs {
    pub rpc_url: Option<String>,
    pub network: String,
    pub database_url: String,
    pub log_level: String,
    pub log_no_color: bool,
    pub batch_scale: f64,
    pub cache_ttl: Duration,
    pub ignore_checkpoint: Option<String>,
    pub vcp_before_synced: bool,
    pub skip_resolving_addresses: bool,
    pub extra_data: bool,
    pub upgrade_db: bool,
    pub initialize_db: bool,
}

pub fn get_cli_args() -> CliArgs {
    let matches = get_cli_matches();
    CliArgs {
        rpc_url: matches.get_one::<String>("rpc-url").map(|v| v.to_owned()),
        network: matches.get_one::<String>("network").expect("Missing network").to_owned(),
        database_url: matches.get_one::<String>("database-url").expect("Missing database-url").to_owned(),
        log_level: matches.get_one::<String>("log-level").expect("Missing log-level").to_owned(),
        log_no_color: matches.get_flag("log-no-color"),
        batch_scale: matches.get_one::<f64>("batch-scale").expect("Missing batch-scale").to_owned(),
        cache_ttl: Duration::from_secs(matches.get_one::<u64>("cache-ttl").expect("Missing cache-ttl").to_owned()),
        ignore_checkpoint: matches.get_one::<String>("ignore-checkpoint").map(|i| i.to_lowercase()),
        vcp_before_synced: matches.get_flag("vcp-before-synced"),
        skip_resolving_addresses: matches.get_flag("skip-resolving-addresses"),
        extra_data: matches.get_flag("extra-data"),
        upgrade_db: matches.get_flag("upgrade-db"),
        initialize_db: matches.get_flag("initialize-db"),
    }
}

fn get_cli_matches() -> ArgMatches {
    Command::new(crate_name!())
        .about(crate_description!())
        .arg(
            Arg::new("rpc-url")
                .short('s')
                .long("rpc-url")
                .help("The url to a kaspad instance, e.g 'ws://localhost:17110'. Leave empty to use the Kaspa PNN")
                .action(clap::ArgAction::Set),
        )
        .arg(
            Arg::new("network")
                .short('n')
                .long("network")
                .help("The network type and suffix, e.g. 'testnet-11'")
                .default_value("mainnet")
                .action(clap::ArgAction::Set),
        )
        .arg(
            Arg::new("database-url")
                .short('d')
                .long("database-url")
                .help("The url to a PostgreSQL instance")
                .default_value("postgres://postgres:postgres@localhost:5432/postgres")
                .action(clap::ArgAction::Set),
        )
        .arg(
            Arg::new("log-level")
                .short('l')
                .long("log-level")
                .help("error, warn, info, debug, trace, off")
                .default_value("info")
                .action(clap::ArgAction::Set),
        )
        .arg(Arg::new("log-no-color").long("no-color").help("Disable colored output").action(clap::ArgAction::SetTrue))
        .arg(
            Arg::new("batch-scale")
                .short('b')
                .long("batch-scale")
                .help("Batch size factor [0.1-10]. Adjusts internal queues and database batch sizes")
                .default_value("1.0")
                .action(clap::ArgAction::Set)
                .value_parser(clap::value_parser!(f64)),
        )
        .arg(
            Arg::new("cache-ttl")
                .short('t')
                .long("cache-ttl")
                .help("Cache ttl (in seconds). Adjusts tx and block caches for in-memory duplicate detection")
                .default_value("60")
                .action(clap::ArgAction::Set)
                .value_parser(clap::value_parser!(u64)),
        )
        .arg(
            Arg::new("ignore-checkpoint")
                .short('i')
                .long("ignore-checkpoint")
                .help("Ignore checkpoint and start from a specified block hash, 'p' for pruning point or 'v' for virtual")
                .action(clap::ArgAction::Set),
        )
        .arg(
            Arg::new("vcp-before-synced")
                .long("vcp-before-synced")
                .help("Start VCP as soon as the filler has passed the previous run")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("skip-resolving-addresses")
                .long("skip-resolving-addresses")
                .help("Reduces database load by not tracking an address's transactions")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("extra-data")
                .short('x')
                .long("extra-data")
                .help("Save the following extra data to the db: transaction.payload")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("upgrade-db")
                .short('u')
                .long("upgrade-db")
                .help("Auto-upgrades older db schemas. Use with care")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("initialize-db")
                .short('c')
                .long("initialize-db")
                .help("(Re-)initializes the database schema. Use with care")
                .action(clap::ArgAction::SetTrue),
        )
        .get_matches()
}
