use crate::web::model::metrics::Metrics;
use chrono::{DateTime, Utc};
use log::{info, warn};
use simply_kaspa_cli::cli_args::CliArgs;
use simply_kaspa_database::client::KaspaDbClient;
use std::ops::Sub;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use tokio::time::{sleep, Duration};
use tokio_cron_scheduler::{Job, JobScheduler};

pub async fn pruner(
    cli_args: CliArgs,
    run: Arc<AtomicBool>,
    metrics: Arc<RwLock<Metrics>>,
    database: KaspaDbClient,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(cron) = cli_args.prune_db.clone() {
        warn!("\x1b[33mDatabase pruning enabled. Cron: '{cron}'. Retention: {} days\x1b[0m", cli_args.prune_db_retention_days);
        let mut metrics_rw = metrics.write().await;
        metrics_rw.components.db_pruner.enabled = true;
        metrics_rw.components.db_pruner.cron = Some(cron.clone());
        metrics_rw.components.db_pruner.retention_days = Some(cli_args.prune_db_retention_days);
        drop(metrics_rw);

        let run_clone = run.clone();
        let metrics_clone = metrics.clone();
        let database = database.clone();
        let job = Job::new_async(format!("0 {}", cron), move |_, _| {
            Box::pin(prune(run_clone.clone(), metrics_clone.clone(), cli_args.prune_db_retention_days, database.clone()))
        })
        .unwrap();
        let scheduler = JobScheduler::new().await?;
        scheduler.add(job).await?;
        scheduler.start().await?;
        while run.load(Ordering::Relaxed) {
            sleep(Duration::from_secs(2)).await;
        }
    } else {
        info!("Database pruning disabled");
    }
    Ok(())
}

pub async fn prune(run: Arc<AtomicBool>, metrics: Arc<RwLock<Metrics>>, retention_days: u16, database: KaspaDbClient) {
    let now = DateTime::from_timestamp_millis(Utc::now().timestamp_millis()).unwrap();
    let cutoff = now.sub(chrono::Duration::days(retention_days.into()));
    let mut metrics_rw = metrics.write().await;
    metrics_rw.components.db_pruner.running = Some(true);
    metrics_rw.components.db_pruner.last_cutoff_timestamp = Some(cutoff.timestamp_millis().try_into().unwrap());
    metrics_rw.components.db_pruner.last_cutoff_date_time = Some(cutoff);
    metrics_rw.components.db_pruner.last_run_timestamp = Some(now.timestamp_millis().try_into().unwrap());
    metrics_rw.components.db_pruner.last_run_date_time = Some(now);
    drop(metrics_rw);
    info!("\x1b[33mDatabase pruning started, retention_days = {retention_days}\x1b[0m");
    let start = Instant::now();
    let result = database.prune(run, cutoff.timestamp_millis()).await;
    let duration = Duration::from_millis(start.elapsed().as_millis().try_into().unwrap());
    let mut metrics_rw = metrics.write().await;
    metrics_rw.components.db_pruner.running = Some(false);
    metrics_rw.components.db_pruner.last_run_duration = Some(duration.as_millis().try_into().unwrap());
    metrics_rw.components.db_pruner.last_run_duration_pretty = Some(humantime::format_duration(duration).to_string());
    match result {
        Ok(total_rows_affected) => {
            info!("\x1b[32mDatabase pruning completed successfully!\x1b[0m");
            metrics_rw.components.db_pruner.last_rows_deleted = Some(total_rows_affected);
            metrics_rw.components.db_pruner.last_run_ok = Some(true);
        }
        Err(e) => {
            warn!("\x1b[33mDatabase pruning completed with error(s): {}\x1b[0m", e);
            metrics_rw.components.db_pruner.last_run_ok = Some(false);
        }
    };
}
