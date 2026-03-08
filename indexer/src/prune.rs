use crate::return_on_shutdown;
use crate::settings::Settings;
use crate::web::model::metrics::{Metrics, MetricsComponentDbPrunerResult};
use chrono::{DateTime, Timelike, Utc};
use log::{error, info, warn};
use serde_json::to_string_pretty;
use simply_kaspa_cli::cli_args::{CliDisable, CliField, PruningConfig};
use simply_kaspa_database::client::KaspaDbClient;
use simply_kaspa_signal::signal_handler::SignalHandler;
use std::collections::HashMap;
use std::error::Error;
use std::future::Future;
use std::ops::Sub;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{Duration, sleep};
use tokio_cron_scheduler::{Job, JobScheduler};

pub async fn pruner(
    settings: Settings,
    signal_handler: SignalHandler,
    metrics: Arc<RwLock<Metrics>>,
    database: KaspaDbClient,
) -> Result<(), Box<dyn Error>> {
    if let Some(cron) = settings.cli_args.pruning.prune_db.clone() {
        let pruning_config = settings.cli_args.pruning.clone().resolved();

        info!("Database pruning enabled:\n{}", to_string_pretty(&pruning_config).unwrap());
        {
            let mut metrics_rw = metrics.write().await;
            metrics_rw.components.db_pruner.enabled = true;
            metrics_rw.components.db_pruner.cron = Some(cron.clone());

            let mut retention = HashMap::new();
            retention.insert("block_parent".to_string(), format_duration(pruning_config.retention_block_parent));
            retention.insert("blocks_transactions".to_string(), format_duration(pruning_config.retention_blocks_transactions));
            retention.insert("blocks".to_string(), format_duration(pruning_config.retention_blocks));
            retention.insert("transactions".to_string(), format_duration(pruning_config.retention_transactions));
            retention.insert("addresses_transactions".to_string(), format_duration(pruning_config.retention_addresses_transactions));
            metrics_rw.components.db_pruner.retention = Some(retention);
        }

        let sh_clone = signal_handler.clone();
        let job = Job::new_async(format!("0 {}", cron), move |_, _| {
            Box::pin(prune(settings.clone(), pruning_config.clone(), sh_clone.clone(), metrics.clone(), database.clone()))
        })
        .unwrap();
        let scheduler = JobScheduler::new().await?;
        scheduler.add(job).await?;
        scheduler.start().await?;
        while !signal_handler.is_shutdown() {
            sleep(Duration::from_secs(2)).await;
        }
    } else {
        info!("Database pruning is disabled. Disk usage will grow indefinitely");
    }
    Ok(())
}

pub async fn prune(
    settings: Settings,
    pruning_config: PruningConfig,
    signal_handler: SignalHandler,
    metrics: Arc<RwLock<Metrics>>,
    database: KaspaDbClient,
) {
    let cli_args = settings.cli_args.clone();
    let net_bps = settings.net_bps as u64;
    let batch_size = cli_args.pruning.prune_batch_size;
    info!("\x1b[33mDatabase pruning started\x1b[0m");
    let mut step_errors = 0;
    let (checkpoint_blue_score, checkpoint_time) = {
        let mut metrics_rw = metrics.write().await;
        metrics_rw.components.db_pruner.running = Some(true);
        metrics_rw.components.db_pruner.start_time = Some(now());
        metrics_rw.components.db_pruner.results = Some(HashMap::new());
        let block = metrics_rw.checkpoint.block.as_ref().unwrap_or_else(|| panic!("Checkpoint block not available at prune time"));
        (block.blue_score, block.date_time)
    };

    if let Some(retention) = pruning_config.retention_block_parent {
        let db = database.clone();
        let retention = retention.min(pruning_config.retention_blocks.unwrap_or(Duration::MAX));
        let blue_score_lt = checkpoint_blue_score.saturating_sub(retention.as_secs() * net_bps) as i64;
        let cutoff_time = checkpoint_time.sub(retention);
        return_on_shutdown!(signal_handler.is_shutdown());
        step_errors += prune_step(
            "block_parent",
            metrics.clone(),
            |(blue_score, _)| async move { db.prune_block_parent(blue_score, batch_size).await },
            blue_score_lt,
            cutoff_time,
        )
        .await as i32;
    }

    if let Some(retention) = pruning_config.retention_blocks_transactions {
        let db = database.clone();
        return_on_shutdown!(signal_handler.is_shutdown());
        if !cli_args.is_disabled(CliDisable::BlocksTable) && !cli_args.is_excluded(CliField::BlockBlueScore) {
            let retention = retention.min(pruning_config.retention_blocks.unwrap_or(Duration::MAX));
            let blue_score_lt = checkpoint_blue_score.saturating_sub(retention.as_secs() * net_bps) as i64;
            let cutoff_time = checkpoint_time.sub(retention);
            step_errors += prune_step(
                "blocks_transactions (b)",
                metrics.clone(),
                |(blue_score, _)| async move { db.prune_blocks_transactions_using_blocks(blue_score, batch_size).await },
                blue_score_lt,
                cutoff_time,
            )
            .await as i32;
        } else {
            let retention = retention.min(pruning_config.retention_transactions.unwrap_or(Duration::MAX));
            let cutoff_time = checkpoint_time.sub(retention);
            step_errors += prune_step(
                "blocks_transactions (t)",
                metrics.clone(),
                |(_, time_ms)| async move { db.prune_blocks_transactions_using_transactions(time_ms, batch_size).await },
                0,
                cutoff_time,
            )
            .await as i32;
        }
    }

    if let Some(retention) = pruning_config.retention_blocks {
        let blue_score_lt = checkpoint_blue_score.saturating_sub(retention.as_secs() * net_bps) as i64;
        let cutoff_time = checkpoint_time.sub(retention);
        return_on_shutdown!(signal_handler.is_shutdown());
        let db = database.clone();
        step_errors += prune_step(
            "blocks",
            metrics.clone(),
            |(blue_score, _)| async move { db.prune_blocks(blue_score, batch_size).await },
            blue_score_lt,
            cutoff_time,
        )
        .await as i32;

        if cli_args.is_disabled(CliDisable::TransactionAcceptance)
            && !cli_args.is_disabled(CliDisable::BlocksTable)
            && !cli_args.is_excluded(CliField::BlockBlueScore)
        {
            return_on_shutdown!(signal_handler.is_shutdown());
            let db = database.clone();
            step_errors += prune_step(
                "transactions_acceptances (b)",
                metrics.clone(),
                |(blue_score, _)| async move { db.prune_transactions_acceptances_using_blocks(blue_score, batch_size).await },
                blue_score_lt,
                cutoff_time,
            )
            .await as i32;
        }
    }

    if let Some(retention) = pruning_config.retention_transactions {
        let cutoff_time = checkpoint_time.sub(retention);
        return_on_shutdown!(signal_handler.is_shutdown());
        let db = database.clone();

        let pruning_point_is_passed = {
            let metrics = metrics.read().await;
            metrics.checkpoint.block.as_ref().map(|b| b.timestamp > cutoff_time.timestamp_millis() as u64).unwrap_or(false)
        };

        if pruning_point_is_passed {
            step_errors += prune_step(
                "transactions",
                metrics.clone(),
                |(_, time_ms)| async move { db.prune_transactions(time_ms, batch_size).await },
                0,
                cutoff_time,
            )
            .await as i32;
        } else {
            warn!("Cannot prune transactions, pruning point is newer than last checkpoint")
        }
    }

    if let Some(retention) = pruning_config.retention_addresses_transactions {
        let cutoff_time = checkpoint_time.sub(retention);
        return_on_shutdown!(signal_handler.is_shutdown());
        let db = database.clone();
        if !cli_args.is_excluded(CliField::TxOutScriptPublicKeyAddress) {
            step_errors += prune_step(
                "addresses_transactions",
                metrics.clone(),
                |(_, time_ms)| async move { db.prune_addresses_transactions(time_ms, batch_size).await },
                0,
                cutoff_time,
            )
            .await as i32;
        } else {
            step_errors += prune_step(
                "scripts_transactions",
                metrics.clone(),
                |(_, time_ms)| async move { db.prune_scripts_transactions(time_ms, batch_size).await },
                0,
                cutoff_time,
            )
            .await as i32;
        }
    }

    if step_errors == 0 {
        info!("\x1b[32mDatabase pruning completed successfully!\x1b[0m");
    } else {
        warn!("\x1b[33mDatabase pruning completed with one or more errors\x1b[0m");
    }
    let mut metrics_rw = metrics.write().await;
    metrics_rw.components.db_pruner.running = Some(false);
    metrics_rw.components.db_pruner.completed_time = Some(now());
    metrics_rw.components.db_pruner.completed_successfully = Some(step_errors == 0);
}

pub async fn prune_step<F, Fut, E>(
    step_name: &'static str,
    metrics: Arc<RwLock<Metrics>>,
    db_call: F,
    cutoff_blue_score: i64,
    cutoff_time: DateTime<Utc>,
) -> bool
where
    F: FnOnce((i64, i64)) -> Fut,
    Fut: Future<Output = Result<u64, E>> + Send + 'static,
    E: Error + Send + Sync + 'static,
{
    if cutoff_blue_score > 0 {
        info!("Pruning {step_name} rows older than {cutoff_time} (bs: {cutoff_blue_score})");
    } else {
        info!("Pruning {step_name} rows older than {cutoff_time}");
    }
    let start_time = now();
    let mut metrics_result =
        MetricsComponentDbPrunerResult { start_time, cutoff_time, duration: None, success: None, rows_deleted: None };
    {
        let mut metrics_rw = metrics.write().await;
        metrics_rw.components.db_pruner.results.as_mut().unwrap().insert(step_name.to_string(), metrics_result.clone());
    }
    let step_result = db_call((cutoff_blue_score, cutoff_time.timestamp_millis())).await;

    let success = step_result.is_ok();
    metrics_result.success = Some(success);
    metrics_result.duration = Some(now().signed_duration_since(start_time).to_std().unwrap());

    match step_result {
        Ok(rows_affected) => {
            info!("Pruned {step_name}, {rows_affected} rows deleted");
            metrics_result.rows_deleted = Some(rows_affected);
        }
        Err(e) => {
            error!("Pruning {step_name} failed with error: {e}");
            metrics_result.rows_deleted = None;
        }
    }
    let mut metrics_rw = metrics.write().await;
    metrics_rw.components.db_pruner.results.as_mut().unwrap().insert(step_name.to_string(), metrics_result.clone());
    !success
}

fn format_duration(duration: Option<Duration>) -> Option<String> {
    duration.map(|d| humantime::format_duration(d).to_string())
}

fn now() -> DateTime<Utc> {
    Utc::now().with_nanosecond(0).unwrap()
}
