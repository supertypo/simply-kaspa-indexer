use std::cmp::min;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::settings::settings::Settings;
use crossbeam_queue::ArrayQueue;
use kaspa_database::client::client::KaspaDbClient;
use kaspa_database::models::address_transaction::AddressTransaction;
use kaspa_database::models::block_transaction::BlockTransaction;
use kaspa_database::models::transaction::Transaction;
use kaspa_database::models::transaction_input::TransactionInput;
use kaspa_database::models::transaction_output::TransactionOutput;
use kaspa_database::models::types::hash::Hash as SqlHash;
use log::{debug, info};
use tokio::task;
use tokio::time::sleep;

pub async fn insert_txs_ins_outs(
    settings: Settings,
    run: Arc<AtomicBool>,
    db_transactions_queue: Arc<
        ArrayQueue<(Option<Transaction>, BlockTransaction, Vec<TransactionInput>, Vec<TransactionOutput>, Vec<AddressTransaction>)>,
    >,
    database: KaspaDbClient,
) {
    let batch_scale = settings.cli_args.batch_scale;
    let batch_size = (5000f64 * batch_scale) as usize;
    let mut transactions = vec![];
    let mut block_tx = vec![];
    let mut tx_inputs = vec![];
    let mut tx_outputs = vec![];
    let mut tx_addresses = vec![];
    let mut last_block_timestamp = 0;
    let mut last_commit_time = Instant::now();

    while run.load(Ordering::Relaxed) {
        if let Some((transaction, block_transaction, inputs, outputs, addresses)) = db_transactions_queue.pop() {
            if let Some(t) = transaction {
                last_block_timestamp = t.block_time;
                transactions.push(t);
            }
            block_tx.push(block_transaction);
            tx_inputs.extend(inputs.into_iter());
            tx_outputs.extend(outputs.into_iter());
            tx_addresses.extend(addresses.into_iter());

            if block_tx.len() >= batch_size || (block_tx.len() >= 1 && Instant::now().duration_since(last_commit_time).as_secs() > 2) {
                let start_commit_time = Instant::now();
                debug!(
                    "Committing {} transactions ({} block/tx, {} inputs, {} outputs)",
                    transactions.len(),
                    block_tx.len(),
                    tx_inputs.len(),
                    tx_outputs.len()
                );
                let transactions_len = transactions.len();
                let transaction_ids = transactions.iter().map(|t| t.transaction_id.clone()).collect();

                let tx_handle = task::spawn(insert_txs(batch_scale, transactions, database.clone()));
                let tx_inputs_handle = task::spawn(insert_tx_inputs(batch_scale, tx_inputs, database.clone()));
                let tx_outputs_handle = task::spawn(insert_tx_outputs(batch_scale, tx_outputs, database.clone()));
                let rows_affected_tx = tx_handle.await.unwrap();
                let rows_affected_tx_inputs = tx_inputs_handle.await.unwrap();
                let rows_affected_tx_outputs = tx_outputs_handle.await.unwrap();

                let mut rows_affected_tx_addresses = 0;
                if !settings.cli_args.skip_resolving_addresses {
                    // ^Input address resolving can only happen after the transaction + inputs + outputs are committed
                    rows_affected_tx_addresses += insert_input_tx_addr(batch_scale, transaction_ids, database.clone()).await;
                    // ^Persisting inputs and outputs concurrently will deadlock
                    rows_affected_tx_addresses += insert_output_tx_addr(batch_scale, tx_addresses, database.clone()).await;
                }

                // ^All other transaction details needs to be committed before linking to blocks, to avoid incomplete checkpoints
                let rows_affected_block_tx = insert_block_txs(batch_scale, block_tx, database.clone()).await;

                let commit_time = Instant::now().duration_since(start_commit_time).as_millis();
                let tps = transactions_len as f64 / commit_time as f64 * 1000f64;
                info!(
                    "Committed {} new txs in {}ms ({:.1} tps, {} blk_tx, {} tx_in, {} tx_out, {} adr_tx). Last tx: {}",
                    rows_affected_tx,
                    commit_time,
                    tps,
                    rows_affected_block_tx,
                    rows_affected_tx_inputs,
                    rows_affected_tx_outputs,
                    rows_affected_tx_addresses,
                    chrono::DateTime::from_timestamp_millis(last_block_timestamp / 1000 * 1000).unwrap()
                );

                transactions = vec![];
                block_tx = vec![];
                tx_inputs = vec![];
                tx_outputs = vec![];
                tx_addresses = vec![];
                last_commit_time = Instant::now();
            }
        } else {
            sleep(Duration::from_millis(100)).await;
        }
    }
}

async fn insert_txs(batch_scale: f64, values: Vec<Transaction>, database: KaspaDbClient) -> u64 {
    let batch_size = min((400f64 * batch_scale) as u16, 8000) as usize; // 2^16 / fields
    let key = "transactions";
    let start_time = Instant::now();
    debug!("Processing {} {}", values.len(), key);
    let mut rows_affected = 0;
    for batch_values in values.chunks(batch_size) {
        rows_affected += database.insert_transactions(batch_values).await.expect(format!("Insert {} FAILED", key).as_str());
    }
    debug!("Committed {} {} in {}ms", rows_affected, key, Instant::now().duration_since(start_time).as_millis());
    return rows_affected;
}

async fn insert_tx_inputs(batch_scale: f64, values: Vec<TransactionInput>, database: KaspaDbClient) -> u64 {
    let batch_size = min((400f64 * batch_scale) as u16, 8000) as usize; // 2^16 / fields
    let key = "transaction_inputs";
    let start_time = Instant::now();
    debug!("Processing {} {}", values.len(), key);
    let mut rows_affected = 0;
    for batch_values in values.chunks(batch_size) {
        rows_affected += database.insert_transaction_inputs(batch_values).await.expect(format!("Insert {} FAILED", key).as_str());
    }
    debug!("Committed {} {} in {}ms", rows_affected, key, Instant::now().duration_since(start_time).as_millis());
    return rows_affected;
}

async fn insert_tx_outputs(batch_scale: f64, values: Vec<TransactionOutput>, database: KaspaDbClient) -> u64 {
    let batch_size = min((500f64 * batch_scale) as u16, 10000) as usize; // 2^16 / fields
    let key = "transactions_outputs";
    let start_time = Instant::now();
    debug!("Processing {} {}", values.len(), key);
    let mut rows_affected = 0;
    for batch_values in values.chunks(batch_size) {
        rows_affected += database.insert_transaction_outputs(batch_values).await.expect(format!("Insert {} FAILED", key).as_str());
    }
    debug!("Committed {} {} in {}ms", rows_affected, key, Instant::now().duration_since(start_time).as_millis());
    return rows_affected;
}

async fn insert_input_tx_addr(batch_scale: f64, values: Vec<SqlHash>, database: KaspaDbClient) -> u64 {
    let batch_size = min((400f64 * batch_scale) as u16, 8000) as usize;
    let key = "input addresses_transactions";
    let start_time = Instant::now();
    debug!("Processing {} transactions for {}", values.len(), key);
    let mut rows_affected = 0;
    for batch_values in values.chunks(batch_size) {
        rows_affected +=
            database.insert_address_transactions_from_inputs(batch_values).await.expect(format!("Insert {} FAILED", key).as_str());
    }
    debug!("Committed {} {} in {}ms", rows_affected, key, Instant::now().duration_since(start_time).as_millis());
    return rows_affected;
}

async fn insert_output_tx_addr(batch_scale: f64, values: Vec<AddressTransaction>, database: KaspaDbClient) -> u64 {
    let batch_size = min((500f64 * batch_scale) as u16, 20000) as usize; // 2^16 / fields
    let key = "output addresses_transactions";
    let start_time = Instant::now();
    debug!("Processing {} {}", values.len(), key);
    let mut rows_affected = 0;
    for batch_values in values.chunks(batch_size) {
        rows_affected += database.insert_address_transactions(batch_values).await.expect(format!("Insert {} FAILED", key).as_str());
    }
    debug!("Committed {} {} in {}ms", rows_affected, key, Instant::now().duration_since(start_time).as_millis());
    return rows_affected;
}

async fn insert_block_txs(batch_scale: f64, values: Vec<BlockTransaction>, database: KaspaDbClient) -> u64 {
    let batch_size = min((800f64 * batch_scale) as u16, 30000) as usize; // 2^16 / fields
    let key = "block/transaction mappings";
    let start_time = Instant::now();
    debug!("Processing {} {}", values.len(), key);
    let mut rows_affected = 0;
    for batch_values in values.chunks(batch_size) {
        rows_affected += database.insert_block_transactions(batch_values).await.expect(format!("Insert {} FAILED", key).as_str());
    }
    debug!("Committed {} {} in {}ms", rows_affected, key, Instant::now().duration_since(start_time).as_millis());
    return rows_affected;
}
