use std::cmp::min;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

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
    run: Arc<AtomicBool>,
    batch_scale: f64,
    skip_input_resolve: bool,
    db_transactions_queue: Arc<
        ArrayQueue<(Option<Transaction>, BlockTransaction, Vec<TransactionInput>, Vec<TransactionOutput>, Vec<AddressTransaction>)>,
    >,
    database: KaspaDbClient,
) {
    let batch_size = min((1000f64 * batch_scale) as u16, 7500) as usize;
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
                // We used a HashSet first to filter some amount of duplicates locally, now we can switch back to vector:
                let transactions_len = transactions.len();
                let transactions_vec: Vec<Transaction> = transactions.into_iter().collect();
                let transaction_ids = transactions_vec.iter().map(|t| t.transaction_id.clone()).collect();
                let block_transactions_vec = block_tx.into_iter().collect();
                let inputs_vec = tx_inputs.into_iter().collect();
                let outputs_vec = tx_outputs.into_iter().collect();
                let addresses_vec = tx_addresses.into_iter().collect();

                let tx_handle = task::spawn(insert_txs(batch_size, transactions_vec, database.clone()));
                let tx_inputs_handle = task::spawn(insert_tx_inputs(batch_size, inputs_vec, database.clone()));
                let tx_outputs_handle = task::spawn(insert_tx_outputs(batch_size, outputs_vec, database.clone()));
                let tx_out_addr_handle = task::spawn(insert_output_tx_addr(batch_size, addresses_vec, database.clone()));

                let rows_affected_tx = tx_handle.await.unwrap();
                let rows_affected_tx_inputs = tx_inputs_handle.await.unwrap();
                let rows_affected_tx_outputs = tx_outputs_handle.await.unwrap();
                let mut rows_affected_tx_addresses = tx_out_addr_handle.await.unwrap();
                // ^Input address resolving can only happen after the transaction + inputs + outputs are committed
                if !skip_input_resolve {
                    rows_affected_tx_addresses += insert_input_tx_addr(batch_size, transaction_ids, database.clone()).await;
                }
                // ^All other transaction details needs to be committed before linking to blocks, to avoid incomplete checkpoints
                let rows_affected_block_tx = insert_block_txs(batch_size, block_transactions_vec, database.clone()).await;

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

async fn insert_txs(max_batch_size: usize, values: Vec<Transaction>, database: KaspaDbClient) -> u64 {
    let key = "transactions";
    let start_time = Instant::now();
    debug!("Processing {} {}", values.len(), key);
    let mut rows_affected = 0;
    for batch_values in values.chunks(max_batch_size) {
        rows_affected += database.insert_transactions(batch_values).await.expect(format!("Insert {} FAILED", key).as_str());
    }
    debug!("Committed {} {} in {}ms", rows_affected, key, Instant::now().duration_since(start_time).as_millis());
    return rows_affected;
}

async fn insert_tx_inputs(max_batch_size: usize, values: Vec<TransactionInput>, database: KaspaDbClient) -> u64 {
    let key = "transaction_inputs";
    let start_time = Instant::now();
    debug!("Processing {} {}", values.len(), key);
    let mut rows_affected = 0;
    for batch_values in values.chunks(max_batch_size) {
        rows_affected += database.insert_transaction_inputs(batch_values).await.expect(format!("Insert {} FAILED", key).as_str());
    }
    debug!("Committed {} {} in {}ms", rows_affected, key, Instant::now().duration_since(start_time).as_millis());
    return rows_affected;
}

async fn insert_tx_outputs(max_batch_size: usize, values: Vec<TransactionOutput>, database: KaspaDbClient) -> u64 {
    let key = "transactions_outputs";
    let start_time = Instant::now();
    debug!("Processing {} {}", values.len(), key);
    let mut rows_affected = 0;
    for batch_values in values.chunks(max_batch_size) {
        rows_affected += database.insert_transaction_outputs(batch_values).await.expect(format!("Insert {} FAILED", key).as_str());
    }
    debug!("Committed {} {} in {}ms", rows_affected, key, Instant::now().duration_since(start_time).as_millis());
    return rows_affected;
}

async fn insert_input_tx_addr(max_batch_size: usize, values: Vec<SqlHash>, database: KaspaDbClient) -> u64 {
    let key = "input addresses_transactions";
    let start_time = Instant::now();
    debug!("Processing {} transactions for {}", values.len(), key);
    let mut rows_affected = 0;
    for batch_values in values.chunks(max_batch_size) {
        rows_affected +=
            database.insert_address_transactions_from_inputs(batch_values).await.expect(format!("Insert {} FAILED", key).as_str());
    }
    debug!("Committed {} {} in {}ms", rows_affected, key, Instant::now().duration_since(start_time).as_millis());
    return rows_affected;
}

async fn insert_output_tx_addr(max_batch_size: usize, values: Vec<AddressTransaction>, database: KaspaDbClient) -> u64 {
    let key = "output addresses_transactions";
    let start_time = Instant::now();
    debug!("Processing {} {}", values.len(), key);
    let mut rows_affected = 0;
    for batch_values in values.chunks(max_batch_size) {
        rows_affected += database.insert_address_transactions(batch_values).await.expect(format!("Insert {} FAILED", key).as_str());
    }
    debug!("Committed {} {} in {}ms", rows_affected, key, Instant::now().duration_since(start_time).as_millis());
    return rows_affected;
}

async fn insert_block_txs(max_batch_size: usize, values: Vec<BlockTransaction>, database: KaspaDbClient) -> u64 {
    let key = "block/transaction mappings";
    let start_time = Instant::now();
    debug!("Processing {} {}", values.len(), key);
    let mut rows_affected = 0;
    for batch_values in values.chunks(max_batch_size) {
        rows_affected += database.insert_block_transactions(batch_values).await.expect(format!("Insert {} FAILED", key).as_str());
    }
    debug!("Committed {} {} in {}ms", rows_affected, key, Instant::now().duration_since(start_time).as_millis());
    return rows_affected;
}
