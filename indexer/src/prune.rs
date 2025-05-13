use chrono::{Duration, Utc};
use log::{info, warn};
use simply_kaspa_cli::cli_args::CliArgs;
use simply_kaspa_database::client::KaspaDbClient;
use std::ops::Sub;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio_cron_scheduler::{Job, JobScheduler};

pub async fn pruner(run: Arc<AtomicBool>, cli_args: CliArgs, database: KaspaDbClient) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(cron) = cli_args.prune_db.clone() {
        warn!("\x1b[33mDatabase pruning enabled. Cron: '{cron}'. Retention: {} days\x1b[0m", cli_args.prune_db_retention_days);
        let cron = format!("0 {}", cron); // Add required second-of-minute value
        let run_clone = run.clone();
        let cli_args_clone = cli_args.clone();
        let database = database.clone();
        let job =
            Job::new_async(cron, move |_, _| Box::pin(prune(run_clone.clone(), cli_args_clone.clone(), database.clone()))).unwrap();
        let scheduler = JobScheduler::new().await?;
        scheduler.add(job).await?;
        scheduler.start().await?;
        while run.load(Ordering::Relaxed) {
            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        }
    } else {
        info!("Database pruning disabled");
    }
    Ok(())
}

pub async fn prune(run: Arc<AtomicBool>, cli_args: CliArgs, database: KaspaDbClient) {
    let retention_days = cli_args.prune_db_retention_days;
    info!("\x1b[33mDatabase pruning started, retention_days = {retention_days}\x1b[0m");
    let block_time_lt = Utc::now().sub(Duration::days(cli_args.prune_db_retention_days.into())).timestamp_millis();
    match database.prune(run, block_time_lt).await {
        Ok(_) => info!("\x1b[32mDatabase pruning completed successfully!\x1b[0m"),
        Err(e) => warn!("\x1b[33mDatabase pruning completed with error(s): {}\x1b[0m", e),
    };
}
