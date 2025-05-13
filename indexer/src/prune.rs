use chrono::{Duration, Utc};
use log::{error, info, warn};
use simply_kaspa_cli::cli_args::CliArgs;
use simply_kaspa_database::client::KaspaDbClient;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio_cron_scheduler::{Job, JobScheduler};

pub async fn pruner(run: Arc<AtomicBool>, cli_args: CliArgs, database: KaspaDbClient) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(cron) = cli_args.prune_db.clone() {
        info!("\x1b[33mDatabase pruning enabled, cron: {cron}\x1b[0m");
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
    let block_time_lt =
        Utc::now().checked_sub_signed(Duration::days(cli_args.prune_db_retention_days.into())).unwrap().timestamp_millis();

    info!("\x1b[33mDatabase pruning started, retention_days = {retention_days}\x1b[0m");

    let mut errors = 0;
    if run.load(Ordering::Relaxed) {
        info!("Pruning block_parent (block_time_lt = {block_time_lt})");
        match database.delete_old_block_parent(block_time_lt).await {
            Ok(rows_affected) => info!("Pruned block_parent, {} rows deleted", rows_affected),
            Err(e) => {
                errors += 1;
                error!("Pruning block_parent failed: {e}")
            }
        }
    }
    if run.load(Ordering::Relaxed) {
        info!("Pruning blocks_transactions (block_time_lt = {block_time_lt})");
        match database.delete_old_blocks_transactions(block_time_lt).await {
            Ok(rows_affected) => info!("Pruned blocks_transactions, {} rows deleted", rows_affected),
            Err(e) => {
                errors += 1;
                error!("Pruning blocks_transactions failed: {e}")
            }
        }
    }
    if run.load(Ordering::Relaxed) {
        info!("Pruning blocks (block_time_lt = {block_time_lt})");
        match database.delete_old_blocks(block_time_lt).await {
            Ok(rows_affected) => info!("Pruned blocks, {} rows deleted", rows_affected),
            Err(e) => {
                errors += 1;
                error!("Pruning blocks failed: {e}")
            }
        }
    }
    if run.load(Ordering::Relaxed) {
        info!("Pruning transactions_acceptances (block_time_lt = {block_time_lt})");
        match database.delete_old_transactions_acceptances(block_time_lt).await {
            Ok(rows_affected) => info!("Pruned transactions_acceptances, {} rows deleted", rows_affected),
            Err(e) => {
                errors += 1;
                error!("Pruning transactions_acceptances failed: {e}")
            }
        }
    }
    if run.load(Ordering::Relaxed) {
        info!("Pruning spent transactions_outputs (block_time_lt = {block_time_lt})");
        match database.delete_old_transactions_outputs(block_time_lt).await {
            Ok(rows_affected) => info!("Pruned spent transactions_outputs, {} rows deleted", rows_affected),
            Err(e) => {
                errors += 1;
                error!("Pruning spent transactions_outputs failed: {e}")
            }
        }
    }
    if run.load(Ordering::Relaxed) {
        info!("Pruning transactions_inputs (block_time_lt = {block_time_lt})");
        match database.delete_old_transactions_inputs(block_time_lt).await {
            Ok(rows_affected) => info!("Pruned transactions_inputs, {} rows deleted", rows_affected),
            Err(e) => {
                errors += 1;
                error!("Pruning transactions_inputs failed: {e}")
            }
        }
    }
    if run.load(Ordering::Relaxed) {
        info!("Pruning transactions (block_time_lt = {block_time_lt})");
        match database.delete_old_transactions(block_time_lt).await {
            Ok(rows_affected) => info!("Pruned transactions, {} rows deleted", rows_affected),
            Err(e) => {
                errors += 1;
                error!("Pruning transactions failed: {e}")
            }
        }
    }
    if run.load(Ordering::Relaxed) {
        info!("Pruning addresses_transactions (block_time_lt = {block_time_lt})");
        match database.delete_old_addresses_transactions(block_time_lt).await {
            Ok(rows_affected) => info!("Pruned addresses_transactions, {} rows deleted", rows_affected),
            Err(e) => {
                errors += 1;
                error!("Pruning addresses_transactions failed: {e}")
            }
        }
    }
    if run.load(Ordering::Relaxed) {
        info!("Pruning scripts_transactions (block_time_lt = {block_time_lt})");
        match database.delete_old_scripts_transactions(block_time_lt).await {
            Ok(rows_affected) => info!("Pruned scripts_transactions, {} rows deleted", rows_affected),
            Err(e) => {
                errors += 1;
                error!("Pruning scripts_transactions failed: {e}")
            }
        }
    }
    if errors == 0 {
        info!("\x1b[32mDatabase pruning completed successfully!\x1b[0m");
    } else {
        warn!("\x1b[33mDatabase pruning completed with {} errors\x1b[0m", errors);
    }
}
