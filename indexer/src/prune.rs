use crate::return_on_shutdown;
use crate::web::model::metrics::{Metrics, MetricsComponentDbPrunerResult};
use chrono::{DateTime, Timelike, Utc};
use log::{error, info, warn};
use serde_json::to_string_pretty;
use simply_kaspa_cli::cli_args::{CliArgs, CliDisable, CliField, PruningConfig};
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
    cli_args: CliArgs,
    signal_handler: SignalHandler,
    metrics: Arc<RwLock<Metrics>>,
    database: KaspaDbClient,
) -> Result<(), Box<dyn Error>> {
    if let Some(cron) = cli_args.pruning.prune_db.clone() {
        let pruning_config = cli_args.pruning.clone().resolved();

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
            Box::pin(prune(cli_args.clone(), pruning_config.clone(), sh_clone.clone(), metrics.clone(), database.clone()))
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
    cli_args: CliArgs,
    pruning_config: PruningConfig,
    signal_handler: SignalHandler,
    metrics: Arc<RwLock<Metrics>>,
    database: KaspaDbClient,
) {
    info!("\x1b[33mDatabase pruning started\x1b[0m");
    let common_start_time = now();
    let mut step_errors = 0;
    {
        let mut metrics_rw = metrics.write().await;
        metrics_rw.components.db_pruner.running = Some(true);
        metrics_rw.components.db_pruner.start_time = Some(common_start_time);
        metrics_rw.components.db_pruner.results = Some(HashMap::new());
    }

    if let Some(retention) = pruning_config.retention_block_parent {
        let db = database.clone();
        let retention = retention.min(pruning_config.retention_blocks.unwrap_or(Duration::MAX));
        let step_pruning_point = common_start_time.sub(retention);
        return_on_shutdown!(signal_handler.is_shutdown());
        step_errors += prune_step(
            "block_parent",
            metrics.clone(),
            |step_pruning_point| async move { db.prune_block_parent(step_pruning_point, cli_args.pruning.prune_batch_size).await },
            step_pruning_point,
        )
        .await as i32;
    }

    if let Some(retention) = pruning_config.retention_blocks_transactions {
        let db = database.clone();
        return_on_shutdown!(signal_handler.is_shutdown());
        if !cli_args.is_disabled(CliDisable::BlocksTable) && !cli_args.is_excluded(CliField::BlockTimestamp) {
            let retention = retention.min(pruning_config.retention_blocks.unwrap_or(Duration::MAX));
            let step_pruning_point = common_start_time.sub(retention);
            step_errors += prune_step(
                "blocks_transactions (b)",
                metrics.clone(),
                |step_pruning_point| async move {
                    db.prune_blocks_transactions_using_blocks(step_pruning_point, cli_args.pruning.prune_batch_size).await
                },
                step_pruning_point,
            )
            .await as i32;
        } else {
            let retention = retention.min(pruning_config.retention_transactions.unwrap_or(Duration::MAX));
            let step_pruning_point = common_start_time.sub(retention);
            step_errors += prune_step(
                "blocks_transactions (t)",
                metrics.clone(),
                |step_pruning_point| async move {
                    db.prune_blocks_transactions_using_transactions(step_pruning_point, cli_args.pruning.prune_batch_size).await
                },
                step_pruning_point,
            )
            .await as i32;
        }
    }

    if let Some(retention) = pruning_config.retention_blocks {
        let step_pruning_point = common_start_time.sub(retention);
        return_on_shutdown!(signal_handler.is_shutdown());
        let db = database.clone();
        step_errors += prune_step(
            "blocks",
            metrics.clone(),
            |step_pruning_point| async move { db.prune_blocks(step_pruning_point, cli_args.pruning.prune_batch_size).await },
            step_pruning_point,
        )
        .await as i32;

        if cli_args.is_disabled(CliDisable::TransactionAcceptance)
            && !cli_args.is_disabled(CliDisable::BlocksTable)
            && !cli_args.is_excluded(CliField::BlockTimestamp)
        {
            return_on_shutdown!(signal_handler.is_shutdown());
            let db = database.clone();
            step_errors += prune_step(
                "transactions_acceptances (b)",
                metrics.clone(),
                |step_pruning_point| async move {
                    db.prune_transactions_acceptances_using_blocks(step_pruning_point, cli_args.pruning.prune_batch_size).await
                },
                step_pruning_point,
            )
            .await as i32;
        }
    }

    if let Some(retention) = pruning_config.retention_transactions {
        let step_pruning_point = common_start_time.sub(retention);
        return_on_shutdown!(signal_handler.is_shutdown());
        let db = database.clone();

        let pruning_point_is_passed = {
            let metrics = metrics.read().await;
            metrics.checkpoint.block.as_ref().map(|b| b.timestamp > step_pruning_point.timestamp_millis() as u64).unwrap_or(false)
        };

        if pruning_point_is_passed {
            step_errors += prune_step(
                "transactions",
                metrics.clone(),
                |step_pruning_point| async move { db.prune_transactions(step_pruning_point, cli_args.pruning.prune_batch_size).await },
                step_pruning_point,
            )
            .await as i32;
        } else {
            warn!("Cannot prune transactions, pruning point is newer than last checkpoint")
        }
    }

    if let Some(retention) = pruning_config.retention_addresses_transactions {
        let step_pruning_point = common_start_time.sub(retention);
        return_on_shutdown!(signal_handler.is_shutdown());
        let db = database.clone();
        if !cli_args.is_excluded(CliField::TxOutScriptPublicKeyAddress) {
            step_errors += prune_step(
                "addresses_transactions",
                metrics.clone(),
                |step_pruning_point| async move {
                    db.prune_addresses_transactions(step_pruning_point, cli_args.pruning.prune_batch_size).await
                },
                step_pruning_point,
            )
            .await as i32;
        } else {
            step_errors += prune_step(
                "scripts_transactions",
                metrics.clone(),
                |step_pruning_point| async move {
                    db.prune_scripts_transactions(step_pruning_point, cli_args.pruning.prune_batch_size).await
                },
                step_pruning_point,
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
    cutoff_time: DateTime<Utc>,
) -> bool
where
    F: FnOnce(i64) -> Fut,
    Fut: Future<Output = Result<u64, E>> + Send + 'static,
    E: Error + Send + Sync + 'static,
{
    info!("Pruning {step_name} rows older than {cutoff_time}");
    let start_time = now();
    let mut metrics_result =
        MetricsComponentDbPrunerResult { start_time, cutoff_time, duration: None, success: None, rows_deleted: None };
    {
        let mut metrics_rw = metrics.write().await;
        metrics_rw.components.db_pruner.results.as_mut().unwrap().insert(step_name.to_string(), metrics_result.clone());
    }
    let step_result = db_call(cutoff_time.timestamp_millis()).await;

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
