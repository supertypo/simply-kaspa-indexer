use crate::web::model::metrics::{Metrics, MetricsComponentDbPrunerResult};
use chrono::{DateTime, Utc};
use log::{error, info, warn};
use serde_json::to_string_pretty;
use simply_kaspa_cli::cli_args::{CliArgs, PruningConfig};
use simply_kaspa_database::client::KaspaDbClient;
use std::collections::HashMap;
use std::error::Error;
use std::future::Future;
use std::ops::Sub;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{sleep, Duration};
use tokio_cron_scheduler::{Job, JobScheduler};

pub async fn pruner(
    cli_args: CliArgs,
    run: Arc<AtomicBool>,
    metrics: Arc<RwLock<Metrics>>,
    database: KaspaDbClient,
) -> Result<(), Box<dyn Error>> {
    if let Some(cron) = cli_args.pruning.prune_db.clone() {
        let mut pruning_config = cli_args.pruning.clone();
        let default = pruning_config.retention;
        pruning_config.retention_block_parent = pruning_config.retention_block_parent.or(default);
        pruning_config.retention_blocks_transactions = pruning_config.retention_blocks_transactions.or(default);
        pruning_config.retention_blocks = pruning_config.retention_blocks.or(default);
        pruning_config.retention_transactions_acceptances = pruning_config.retention_transactions_acceptances.or(default);
        pruning_config.retention_transactions_outputs = pruning_config.retention_transactions_outputs.or(default);
        pruning_config.retention_transactions_inputs = pruning_config.retention_transactions_inputs.or(default);
        pruning_config.retention_transactions = pruning_config.retention_transactions.or(default);
        pruning_config.retention_addresses_transactions = pruning_config.retention_addresses_transactions.or(default);
        pruning_config.retention_scripts_transactions = pruning_config.retention_scripts_transactions.or(default);

        let cron = cron.replace("_", " ");
        info!("Database pruning enabled:\n{}", to_string_pretty(&pruning_config).unwrap());
        {
            let mut metrics_rw = metrics.write().await;
            metrics_rw.components.db_pruner.enabled = true;
            metrics_rw.components.db_pruner.cron = Some(cron.clone());

            let mut retention = HashMap::new();
            retention.insert("block_parent".to_string(), format_duration(pruning_config.retention_block_parent));
            retention.insert("blocks_transactions".to_string(), format_duration(pruning_config.retention_blocks_transactions));
            retention.insert("blocks".to_string(), format_duration(pruning_config.retention_blocks));
            retention
                .insert("transactions_acceptances".to_string(), format_duration(pruning_config.retention_transactions_acceptances));
            retention.insert("transactions_outputs".to_string(), format_duration(pruning_config.retention_transactions_outputs));
            retention.insert("transactions_inputs".to_string(), format_duration(pruning_config.retention_transactions_inputs));
            retention.insert("transactions".to_string(), format_duration(pruning_config.retention_transactions));
            retention.insert("addresses_transactions".to_string(), format_duration(pruning_config.retention_addresses_transactions));
            retention.insert("scripts_transactions".to_string(), format_duration(pruning_config.retention_scripts_transactions));
            metrics_rw.components.db_pruner.retention = Some(retention);
        }

        let run_clone = run.clone();
        let job = Job::new_async(format!("0 {}", cron), move |_, _| {
            Box::pin(prune(pruning_config.clone(), run_clone.clone(), metrics.clone(), database.clone()))
        })
        .unwrap();
        let scheduler = JobScheduler::new().await?;
        scheduler.add(job).await?;
        scheduler.start().await?;
        while run.load(Ordering::Relaxed) {
            sleep(Duration::from_secs(2)).await;
        }
    } else {
        info!("Database pruning is disabled. Disk usage will grow indefinitely");
    }
    Ok(())
}

pub async fn prune(pruning_config: PruningConfig, run: Arc<AtomicBool>, metrics: Arc<RwLock<Metrics>>, database: KaspaDbClient) {
    info!("\x1b[33mDatabase pruning started\x1b[0m");
    let common_start_time = Utc::now();
    let mut step_errors = 0;
    {
        let mut metrics_rw = metrics.write().await;
        metrics_rw.components.db_pruner.running = Some(true);
        metrics_rw.components.db_pruner.start_time = Some(common_start_time);
        metrics_rw.components.db_pruner.results = Some(HashMap::new());
    }

    if !run.load(Ordering::Relaxed) {
        return;
    }
    if let Some(retention) = pruning_config.retention_block_parent {
        let database_clone = database.clone();
        let step_pruning_point = common_start_time.sub(retention);
        step_errors += prune_step(
            "block_parent",
            metrics.clone(),
            |step_pruning_point| async move { database_clone.prune_block_parent(step_pruning_point).await },
            step_pruning_point,
        )
        .await as i32;
    }

    if !run.load(Ordering::Relaxed) {
        return;
    }
    if let Some(retention) = pruning_config.retention_blocks_transactions {
        let database_clone = database.clone();
        let step_pruning_point = common_start_time.sub(retention);
        step_errors += prune_step(
            "blocks_transactions",
            metrics.clone(),
            |step_pruning_point| async move { database_clone.prune_blocks_transactions(step_pruning_point).await },
            step_pruning_point,
        )
        .await as i32;
    }

    if !run.load(Ordering::Relaxed) {
        return;
    }
    if let Some(retention) = pruning_config.retention_blocks {
        let database_clone = database.clone();
        let step_pruning_point = common_start_time.sub(retention);
        step_errors += prune_step(
            "blocks",
            metrics.clone(),
            |step_pruning_point| async move { database_clone.prune_blocks(step_pruning_point).await },
            step_pruning_point,
        )
        .await as i32;
    }

    if !run.load(Ordering::Relaxed) {
        return;
    }
    if let Some(retention) = pruning_config.retention_transactions_acceptances {
        let database_clone = database.clone();
        let step_pruning_point = common_start_time.sub(retention);
        step_errors += prune_step(
            "transactions_acceptances",
            metrics.clone(),
            |step_pruning_point| async move { database_clone.prune_transactions_acceptances(step_pruning_point).await },
            step_pruning_point,
        )
        .await as i32;
    }

    if !run.load(Ordering::Relaxed) {
        return;
    }
    if let Some(retention) = pruning_config.retention_transactions_outputs {
        let database_clone = database.clone();
        let step_pruning_point = common_start_time.sub(retention);
        step_errors += prune_step(
            "spent transactions_outputs",
            metrics.clone(),
            |step_pruning_point| async move { database_clone.prune_spent_transactions_outputs(step_pruning_point).await },
            step_pruning_point,
        )
        .await as i32;
    }

    if !run.load(Ordering::Relaxed) {
        return;
    }
    if let Some(retention) = pruning_config.retention_transactions_inputs {
        let database_clone = database.clone();
        let step_pruning_point = common_start_time.sub(retention);
        step_errors += prune_step(
            "transactions_inputs",
            metrics.clone(),
            |step_pruning_point| async move { database_clone.prune_transactions_inputs(step_pruning_point).await },
            step_pruning_point,
        )
        .await as i32;
    }

    if !run.load(Ordering::Relaxed) {
        return;
    }
    if let Some(retention) = pruning_config.retention_transactions {
        let database_clone = database.clone();
        let step_pruning_point = common_start_time.sub(retention);
        step_errors += prune_step(
            "transactions",
            metrics.clone(),
            |step_pruning_point| async move { database_clone.prune_transactions(step_pruning_point).await },
            step_pruning_point,
        )
        .await as i32;
    }

    if !run.load(Ordering::Relaxed) {
        return;
    }
    if let Some(retention) = pruning_config.retention_addresses_transactions {
        let database_clone = database.clone();
        let step_pruning_point = common_start_time.sub(retention);
        step_errors += prune_step(
            "addresses_transactions",
            metrics.clone(),
            |step_pruning_point| async move { database_clone.prune_addresses_transactions(step_pruning_point).await },
            step_pruning_point,
        )
        .await as i32;
    }

    if !run.load(Ordering::Relaxed) {
        return;
    }
    if let Some(retention) = pruning_config.retention_scripts_transactions {
        let database_clone = database.clone();
        let step_pruning_point = common_start_time.sub(retention);
        step_errors += prune_step(
            "scripts_transactions",
            metrics.clone(),
            |step_pruning_point| async move { database_clone.prune_scripts_transactions(step_pruning_point).await },
            step_pruning_point,
        )
        .await as i32;
    }

    if step_errors == 0 {
        info!("\x1b[32mDatabase pruning completed successfully!\x1b[0m");
    } else {
        warn!("\x1b[33mDatabase pruning completed with one or more errors\x1b[0m");
    }
    let mut metrics_rw = metrics.write().await;
    metrics_rw.components.db_pruner.completed_time = Some(Utc::now());
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
    let start_time = Utc::now();
    let mut metrics_result = MetricsComponentDbPrunerResult {
        name: step_name.to_string(),
        start_time,
        cutoff_time,
        duration: None,
        success: None,
        rows_deleted: None,
    };
    {
        let mut metrics_rw = metrics.write().await;
        metrics_rw.components.db_pruner.results.as_mut().unwrap().insert(step_name.to_string(), metrics_result.clone());
    }
    let step_result = db_call(cutoff_time.timestamp_millis()).await;

    let success = step_result.is_ok();
    metrics_result.success = Some(success);
    metrics_result.duration = Some(Utc::now().signed_duration_since(start_time).to_std().unwrap());

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
