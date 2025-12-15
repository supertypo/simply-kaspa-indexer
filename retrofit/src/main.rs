use clap::{Parser, ValueEnum};
use indicatif::{ProgressBar, ProgressStyle};
use log::{info, warn};
use simply_kaspa_cli::filter_config::FilterConfig;
use simply_kaspa_database::client::KaspaDbClient;
use simply_kaspa_database::tag_cache::TagCache;
use sqlx::Row;
use std::collections::HashMap;

#[derive(Debug, Clone, ValueEnum)]
enum RetrofitMode {
    /// Only update transactions with NULL tag_id (default)
    NullOnly,
    /// Update all matching transactions, overwriting existing tags
    Overwrite,
    /// Dry-run: Report what would be changed without updating
    Report,
}

#[derive(Parser, Debug)]
#[command(name = "kaspa-payload-retrofit")]
#[command(about = "Apply filter rules to historical transaction data", long_about = None)]
struct Args {
    /// PostgreSQL database URL
    #[arg(long)]
    database_url: String,

    /// Path to filter configuration YAML file
    #[arg(long)]
    filter_config: String,

    /// Retrofit mode
    #[arg(long, value_enum, default_value = "null-only")]
    mode: RetrofitMode,

    /// Batch size for processing transactions (default: 1000)
    #[arg(long, default_value = "1000")]
    batch_size: usize,

    /// Limit number of transactions to process (optional)
    #[arg(long)]
    limit: Option<i64>,
}

#[derive(Debug)]
struct RetrofitMatch {
    transaction_id: Vec<u8>,
    old_tag_id: Option<i32>,
    new_tag_id: i32,
    matched_rule: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let args = Args::parse();

    info!("Loading filter configuration from {}...", args.filter_config);
    let mut filter_config = FilterConfig::from_file(&args.filter_config)?;

    // Build tries if available (optional optimization)
    filter_config.build_tries();

    info!("Connecting to database...");
    let db_client = KaspaDbClient::new(&args.database_url, 10).await?;

    info!("Loading tag cache...");
    let tag_cache = TagCache::load_from_database(db_client.pool()).await?;

    // Pre-populate tag IDs for all filter rules
    let mut rule_tag_ids: HashMap<String, i32> = HashMap::new();
    for rule in &filter_config.sorted_enabled_rules {
        let module = rule.module.as_deref().unwrap_or("default");
        if let Some(tag_id) = tag_cache.get_tag_id(&rule.tag, module) {
            rule_tag_ids.insert(rule.name.clone(), tag_id);
        } else {
            warn!("Tag '{}:{}' not found in database - rule '{}' will be skipped",
                  rule.tag, module, rule.name);
        }
    }

    info!("Querying transactions...");
    let query = match args.mode {
        RetrofitMode::NullOnly => {
            if let Some(limit) = args.limit {
                format!(
                    "SELECT transaction_id, payload, tag_id FROM transactions
                     WHERE payload IS NOT NULL AND tag_id IS NULL
                     ORDER BY block_time DESC LIMIT {}",
                    limit
                )
            } else {
                "SELECT transaction_id, payload, tag_id FROM transactions
                 WHERE payload IS NOT NULL AND tag_id IS NULL
                 ORDER BY block_time DESC"
                    .to_string()
            }
        }
        RetrofitMode::Overwrite | RetrofitMode::Report => {
            if let Some(limit) = args.limit {
                format!(
                    "SELECT transaction_id, payload, tag_id FROM transactions
                     WHERE payload IS NOT NULL
                     ORDER BY block_time DESC LIMIT {}",
                    limit
                )
            } else {
                "SELECT transaction_id, payload, tag_id FROM transactions
                 WHERE payload IS NOT NULL
                 ORDER BY block_time DESC"
                    .to_string()
            }
        }
    };

    let rows = sqlx::query(&query).fetch_all(db_client.pool()).await?;
    info!("Processing {} transactions...", rows.len());

    let pb = ProgressBar::new(rows.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("[{elapsed_precise}] {bar:40.cyan/blue} {pos}/{len} {msg}")
            .unwrap()
            .progress_chars("█▓▒░ "),
    );

    let mut matches: Vec<RetrofitMatch> = Vec::new();
    let mut matched_count = 0;
    let mut skipped_count = 0;

    for row in rows {
        let transaction_id: Vec<u8> = row.try_get(0)?;
        let payload: Vec<u8> = row.try_get(1)?;
        let old_tag_id: Option<i32> = row.try_get(2)?;

        // Apply filter rules
        if let Some((matched_rule, tag_id)) = apply_filters(&filter_config, &rule_tag_ids, &transaction_id, &payload) {
            // Check if we should update based on mode
            let should_update = match args.mode {
                RetrofitMode::NullOnly => old_tag_id.is_none(),
                RetrofitMode::Overwrite | RetrofitMode::Report => true,
            };

            if should_update {
                matches.push(RetrofitMatch {
                    transaction_id,
                    old_tag_id,
                    new_tag_id: tag_id,
                    matched_rule,
                });
                matched_count += 1;
            } else {
                skipped_count += 1;
            }
        }

        pb.inc(1);
    }

    pb.finish_with_message("Matching complete");

    info!("Matched {} transactions", matched_count);
    info!("Skipped {} already-tagged transactions", skipped_count);

    // Apply updates (unless in report mode)
    match args.mode {
        RetrofitMode::Report => {
            info!("Report mode - no updates will be applied");
            print_report(&matches);
        }
        RetrofitMode::NullOnly | RetrofitMode::Overwrite => {
            if matches.is_empty() {
                info!("No transactions to update");
                return Ok(());
            }

            info!("Applying updates in batches of {}...", args.batch_size);
            let update_pb = ProgressBar::new(matches.len() as u64);
            update_pb.set_style(
                ProgressStyle::default_bar()
                    .template("[{elapsed_precise}] {bar:40.green/blue} {pos}/{len} Updating...")
                    .unwrap()
                    .progress_chars("█▓▒░ "),
            );

            let mut updated = 0;
            for chunk in matches.chunks(args.batch_size) {
                let updated_batch = update_tags(&db_client, chunk).await?;
                updated += updated_batch;
                update_pb.inc(chunk.len() as u64);
            }

            update_pb.finish_with_message("Updates complete");
            info!("Successfully updated {} transaction tags", updated);
        }
    }

    Ok(())
}

fn apply_filters(
    config: &FilterConfig,
    rule_tag_ids: &HashMap<String, i32>,
    transaction_id: &[u8],
    payload: &[u8],
) -> Option<(String, i32)> {
    let txid_hex = hex::encode(transaction_id);

    for rule in &config.sorted_enabled_rules {
        let tag_id = match rule_tag_ids.get(&rule.name) {
            Some(&id) => id,
            None => continue, // Skip rules without tag_id
        };

        // Check TXID condition if present
        if let Some(ref txid_cond) = rule.conditions.txid {
            if !txid_hex.starts_with(&hex::encode(&txid_cond.decoded_prefix)) {
                continue; // TXID doesn't match
            }
        }

        // Check payload conditions if present (OR logic)
        if let Some(ref payload_conds) = rule.conditions.payload {
            let mut payload_match = false;
            for cond in payload_conds {
                if payload.starts_with(&cond.decoded_prefix) {
                    payload_match = true;
                    break;
                }
            }
            if !payload_match {
                continue; // No payload condition matched
            }
        }

        // If we got here, this rule matches
        return Some((rule.name.clone(), tag_id));
    }

    None
}

async fn update_tags(
    db_client: &KaspaDbClient,
    matches: &[RetrofitMatch],
) -> Result<usize, Box<dyn std::error::Error>> {
    if matches.is_empty() {
        return Ok(0);
    }

    // Build UPDATE query with CASE statement
    let mut query = String::from("UPDATE transactions SET tag_id = CASE ");
    let mut txids: Vec<Vec<u8>> = Vec::new();

    for m in matches {
        query.push_str(&format!("WHEN transaction_id = ${} THEN {} ", txids.len() + 1, m.new_tag_id));
        txids.push(m.transaction_id.clone());
    }

    query.push_str("END WHERE transaction_id IN (");
    for i in 0..txids.len() {
        if i > 0 {
            query.push_str(", ");
        }
        query.push_str(&format!("${}", i + 1));
    }
    query.push_str(")");

    let mut sqlx_query = sqlx::query(&query);
    for txid in &txids {
        sqlx_query = sqlx_query.bind(txid);
    }

    let result = sqlx_query.execute(db_client.pool()).await?;
    Ok(result.rows_affected() as usize)
}

fn print_report(matches: &[RetrofitMatch]) {
    if matches.is_empty() {
        println!("\nNo transactions would be updated.");
        return;
    }

    println!("\n=== Retrofit Report ===\n");
    println!("Total transactions to update: {}\n", matches.len());

    // Group by rule
    let mut by_rule: HashMap<String, usize> = HashMap::new();
    for m in matches {
        *by_rule.entry(m.matched_rule.clone()).or_insert(0) += 1;
    }

    println!("Matches by rule:");
    let mut rules: Vec<_> = by_rule.iter().collect();
    rules.sort_by(|a, b| b.1.cmp(a.1));
    for (rule, count) in rules {
        println!("  - {}: {} transactions", rule, count);
    }

    println!("\nSample matches (first 10):");
    for (i, m) in matches.iter().take(10).enumerate() {
        let old_tag = m.old_tag_id.map(|id| id.to_string()).unwrap_or_else(|| "NULL".to_string());
        println!(
            "  {}. {} | {} → {} ({})",
            i + 1,
            hex::encode(&m.transaction_id[..8]),
            old_tag,
            m.new_tag_id,
            m.matched_rule
        );
    }
}
