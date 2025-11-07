use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use log::info;
use simply_kaspa_database::client::KaspaDbClient;
use sqlx::Row;
use std::collections::HashMap;
use std::fs;

#[derive(Parser, Debug)]
#[command(name = "kaspa-payload-analyzer")]
#[command(about = "Analyze transaction payloads to discover protocol patterns", long_about = None)]
struct Args {
    /// PostgreSQL database URL
    #[arg(long)]
    database_url: String,

    /// Prefix length for text patterns (default: 20)
    #[arg(long, default_value = "20")]
    text_prefix_length: usize,

    /// Prefix length for hex patterns (default: 8)
    #[arg(long, default_value = "8")]
    hex_prefix_length: usize,

    /// Minimum occurrence count to include in results (default: 10)
    #[arg(long, default_value = "10")]
    min_count: usize,

    /// Limit number of transactions to analyze (optional)
    #[arg(long)]
    limit: Option<i64>,

    /// Generate YAML filter rules for discovered patterns
    #[arg(long)]
    generate_rules: bool,

    /// Output file path (optional, prints to stdout if not specified)
    #[arg(long)]
    output: Option<String>,

    /// Analyze only transactions with NULL tag_id
    #[arg(long, default_value = "true")]
    untagged_only: bool,
}

#[derive(Debug, Clone)]
struct PayloadPattern {
    prefix: String,
    is_text: bool,
    count: usize,
    sample_txids: Vec<String>,
    sample_payloads: Vec<Vec<u8>>, // Store sample payloads for display
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let args = Args::parse();

    info!("Connecting to database...");
    let db_client = KaspaDbClient::new(&args.database_url, 5).await?;

    info!("Querying transactions with payloads...");
    let query = if args.untagged_only {
        if let Some(limit) = args.limit {
            format!(
                "SELECT transaction_id, payload FROM transactions
                 WHERE payload IS NOT NULL AND tag_id IS NULL
                 ORDER BY block_time DESC LIMIT {}",
                limit
            )
        } else {
            "SELECT transaction_id, payload FROM transactions
             WHERE payload IS NOT NULL AND tag_id IS NULL
             ORDER BY block_time DESC"
                .to_string()
        }
    } else {
        if let Some(limit) = args.limit {
            format!(
                "SELECT transaction_id, payload FROM transactions
                 WHERE payload IS NOT NULL
                 ORDER BY block_time DESC LIMIT {}",
                limit
            )
        } else {
            "SELECT transaction_id, payload FROM transactions
             WHERE payload IS NOT NULL
             ORDER BY block_time DESC"
                .to_string()
        }
    };

    let rows = sqlx::query(&query).fetch_all(db_client.pool()).await?;

    info!("Analyzing {} transactions with payloads...", rows.len());

    let pb = ProgressBar::new(rows.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("[{elapsed_precise}] {bar:40.cyan/blue} {pos}/{len} {msg}")
            .unwrap()
            .progress_chars("█▓▒░ "),
    );

    let mut text_patterns: HashMap<String, (Vec<String>, Vec<Vec<u8>>)> = HashMap::new();
    let mut hex_patterns: HashMap<String, (Vec<String>, Vec<Vec<u8>>)> = HashMap::new();

    for row in rows {
        let txid: Vec<u8> = row.try_get(0)?;
        let payload: Vec<u8> = row.try_get(1)?;

        let txid_hex = hex::encode(&txid);

        // Try to decode as UTF-8 text
        if let Ok(text) = String::from_utf8(payload.clone()) {
            // Check if it's printable ASCII/UTF-8
            if text.chars().all(|c| !c.is_control() || c == '\n' || c == '\t') {
                // Extract text prefix
                let prefix = if text.len() > args.text_prefix_length {
                    text.chars().take(args.text_prefix_length).collect()
                } else {
                    text.clone()
                };

                let entry = text_patterns
                    .entry(prefix)
                    .or_insert_with(|| (Vec::new(), Vec::new()));
                entry.0.push(txid_hex.clone());
                if entry.1.len() < 3 {
                    entry.1.push(payload.clone());
                }
            } else {
                // Contains control characters, treat as binary
                let hex_str = hex::encode(&payload);
                let prefix = if hex_str.len() > args.hex_prefix_length {
                    hex_str[..args.hex_prefix_length].to_string()
                } else {
                    hex_str
                };

                let entry = hex_patterns
                    .entry(prefix)
                    .or_insert_with(|| (Vec::new(), Vec::new()));
                entry.0.push(txid_hex.clone());
                if entry.1.len() < 3 {
                    entry.1.push(payload.clone());
                }
            }
        } else {
            // Binary data - extract hex prefix
            let hex_str = hex::encode(&payload);
            let prefix = if hex_str.len() > args.hex_prefix_length {
                hex_str[..args.hex_prefix_length].to_string()
            } else {
                hex_str
            };

            let entry = hex_patterns
                .entry(prefix)
                .or_insert_with(|| (Vec::new(), Vec::new()));
            entry.0.push(txid_hex.clone());
            if entry.1.len() < 3 {
                entry.1.push(payload.clone());
            }
        }

        pb.inc(1);
    }

    pb.finish_with_message("Analysis complete");

    // Convert to PayloadPattern structs and sort by count
    let mut patterns: Vec<PayloadPattern> = Vec::new();

    for (prefix, (txids, payloads)) in text_patterns {
        if txids.len() >= args.min_count {
            patterns.push(PayloadPattern {
                prefix: prefix.clone(),
                is_text: true,
                count: txids.len(),
                sample_txids: txids.into_iter().take(5).collect(),
                sample_payloads: payloads,
            });
        }
    }

    for (prefix, (txids, payloads)) in hex_patterns {
        if txids.len() >= args.min_count {
            patterns.push(PayloadPattern {
                prefix: prefix.clone(),
                is_text: false,
                count: txids.len(),
                sample_txids: txids.into_iter().take(5).collect(),
                sample_payloads: payloads,
            });
        }
    }

    patterns.sort_by(|a, b| b.count.cmp(&a.count));

    // Generate output
    let output = if args.generate_rules {
        generate_yaml_rules(&patterns)
    } else {
        generate_report(&patterns)
    };

    // Write output
    if let Some(output_path) = args.output {
        fs::write(&output_path, output)?;
        info!("Results written to {}", output_path);
    } else {
        println!("{}", output);
    }

    info!(
        "Found {} patterns with at least {} occurrences",
        patterns.len(),
        args.min_count
    );

    Ok(())
}

fn generate_report(patterns: &[PayloadPattern]) -> String {
    let mut report = String::new();
    report.push_str("# Payload Pattern Analysis Report\n\n");
    report.push_str(&format!("Total patterns found: {}\n\n", patterns.len()));

    report.push_str("## Patterns by Frequency\n\n");

    for (idx, pattern) in patterns.iter().enumerate() {
        report.push_str(&format!("### Pattern #{} - {} occurrences\n", idx + 1, pattern.count));
        report.push_str(&format!("**Type**: {}\n", if pattern.is_text { "Text (UTF-8)" } else { "Binary (Hex)" }));

        // Show prefix in both formats
        if pattern.is_text {
            report.push_str(&format!("**Prefix (text)**: `{}`\n", pattern.prefix));
            report.push_str(&format!("**Prefix (hex)**: `{}`\n", hex::encode(pattern.prefix.as_bytes())));
        } else {
            report.push_str(&format!("**Prefix (hex)**: `{}`\n", pattern.prefix));
            // Decode hex prefix as text (lossy conversion)
            if let Ok(decoded) = hex::decode(&pattern.prefix) {
                let text = String::from_utf8_lossy(&decoded);
                report.push_str(&format!("**Prefix (text)**: `{}`\n", text.escape_default()));
            }
        }

        // Show sample payloads
        if !pattern.sample_payloads.is_empty() {
            report.push_str("\n**Sample Payload(s)**:\n");
            for (i, payload) in pattern.sample_payloads.iter().take(2).enumerate() {
                report.push_str(&format!("\nSample {}:\n", i + 1));

                // Show hex (truncated to 128 chars)
                let hex_str = hex::encode(payload);
                let hex_display = if hex_str.len() > 128 {
                    format!("{}... ({} bytes total)", &hex_str[..128], payload.len())
                } else {
                    hex_str
                };
                report.push_str(&format!("- Hex: `{}`\n", hex_display));

                // Always show text (lossy conversion for binary data)
                let text = String::from_utf8_lossy(payload);
                let text_display = if text.len() > 200 {
                    format!("{}... ({} chars)", text.chars().take(200).collect::<String>().escape_default(), text.len())
                } else {
                    text.escape_default().to_string()
                };
                report.push_str(&format!("- Text: `{}`\n", text_display));
            }
        }

        report.push_str("\n**Sample Transaction IDs**:\n");
        for txid in &pattern.sample_txids {
            report.push_str(&format!("- {}\n", txid));
        }
        report.push_str("\n---\n\n");
    }

    report
}

fn generate_yaml_rules(patterns: &[PayloadPattern]) -> String {
    let mut yaml = String::new();
    yaml.push_str("# Auto-generated filter rules from payload analysis\n");
    yaml.push_str("# Review and customize before using in production\n\n");
    yaml.push_str("version: \"1.0\"\n\n");
    yaml.push_str("settings:\n");
    yaml.push_str("  default_store_payload: false\n\n");
    yaml.push_str("rules:\n");

    for (idx, pattern) in patterns.iter().enumerate() {
        let priority = 300 - idx;
        let tag_name = format!("pattern_{}", idx + 1);

        yaml.push_str(&format!("  # Pattern: {} ({} occurrences)\n", pattern.prefix, pattern.count));
        yaml.push_str(&format!("  - name: {}\n", tag_name));
        yaml.push_str(&format!("    tag: {}\n", tag_name));
        yaml.push_str("    module: discovered\n");
        yaml.push_str("    category: unknown\n");
        yaml.push_str(&format!("    priority: {}\n", priority));
        yaml.push_str("    enabled: false  # Set to true after review\n");
        yaml.push_str("    store_payload: true\n");
        yaml.push_str("    conditions:\n");
        yaml.push_str("      payload:\n");

        if pattern.is_text {
            yaml.push_str(&format!("        - prefix: \"{}\"\n", pattern.prefix.escape_default()));
        } else {
            yaml.push_str(&format!("        - prefix: \"hex:{}\"\n", pattern.prefix));
        }

        yaml.push_str("\n");
    }

    yaml
}
