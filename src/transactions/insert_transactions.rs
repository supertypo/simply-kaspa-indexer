use std::cmp::min;
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crossbeam_queue::ArrayQueue;
use log::{debug, info};
use tokio::task;
use tokio::time::sleep;

use crate::database::client::client::KaspaDbClient;
use crate::database::models::address_transaction::AddressTransaction;
use crate::database::models::block_transaction::BlockTransaction;
use crate::database::models::sql_hash::SqlHash;
use crate::database::models::transaction::Transaction;
use crate::database::models::transaction_input::TransactionInput;
use crate::database::models::transaction_output::TransactionOutput;

pub async fn insert_txs_ins_outs(
    run: Arc<AtomicBool>,
    batch_scale: f64,
    db_transactions_queue: Arc<
        ArrayQueue<(Transaction, BlockTransaction, Vec<TransactionInput>, Vec<TransactionOutput>, Vec<AddressTransaction>)>,
    >,
    database: KaspaDbClient,
) {
    let batch_size = min((1000f64 * batch_scale) as u16, 7500);
    let set_size = 3 * batch_size as usize; // Large sets helps us to filter duplicates during catch-up
    let mut transactions: HashSet<Transaction> = HashSet::with_capacity(set_size);
    let mut block_tx: HashSet<BlockTransaction> = HashSet::with_capacity(set_size);
    let mut tx_inputs: HashSet<TransactionInput> = HashSet::with_capacity(set_size * 2);
    let mut tx_outputs: HashSet<TransactionOutput> = HashSet::with_capacity(set_size * 2);
    let mut tx_addresses: HashSet<AddressTransaction> = HashSet::with_capacity(set_size * 2);
    let mut last_block_timestamp;
    let mut last_commit_time = Instant::now();

    while run.load(Ordering::Relaxed) {
        if let Some((transaction, block_transactions, inputs, outputs, addresses)) = db_transactions_queue.pop() {
            last_block_timestamp = transaction.block_time;
            transactions.insert(transaction);
            block_tx.insert(block_transactions);
            tx_inputs.extend(inputs.into_iter());
            tx_outputs.extend(outputs.into_iter());
            tx_addresses.extend(addresses.into_iter());

            if block_tx.len() >= set_size || (block_tx.len() >= 1 && Instant::now().duration_since(last_commit_time).as_secs() > 2) {
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
                rows_affected_tx_addresses += insert_input_tx_addr(batch_size, transaction_ids, database.clone()).await;
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

                transactions = HashSet::with_capacity(set_size);
                block_tx = HashSet::with_capacity(set_size);
                tx_inputs = HashSet::with_capacity(set_size * 2);
                tx_outputs = HashSet::with_capacity(set_size * 2);
                tx_addresses = HashSet::with_capacity(set_size * 2);
                last_commit_time = Instant::now();
            }
        } else {
            sleep(Duration::from_millis(100)).await;
        }
    }
}

async fn insert_txs(max_batch_size: u16, values: Vec<Transaction>, database: KaspaDbClient) -> u64 {
    let key = "transactions";
    let start_time = Instant::now();
    debug!("Processing {} {}", values.len(), key);
    let mut rows_affected = 0;
    for batch_values in values.chunks(max_batch_size as usize) {
        rows_affected += database.insert_transactions(batch_values).await.expect(format!("Insert {} FAILED", key).as_str());
    }
    debug!("Committed {} {} in {}ms", rows_affected, key, Instant::now().duration_since(start_time).as_millis());
    return rows_affected;
}

async fn insert_tx_inputs(max_batch_size: u16, values: Vec<TransactionInput>, database: KaspaDbClient) -> u64 {
    let key = "transaction_inputs";
    let start_time = Instant::now();
    debug!("Processing {} {}", values.len(), key);
    let mut rows_affected = 0;
    for batch_values in values.chunks(max_batch_size as usize) {
        rows_affected += database.insert_transaction_inputs(batch_values).await.expect(format!("Insert {} FAILED", key).as_str());
    }
    debug!("Committed {} {} in {}ms", rows_affected, key, Instant::now().duration_since(start_time).as_millis());
    return rows_affected;
}

async fn insert_tx_outputs(max_batch_size: u16, values: Vec<TransactionOutput>, database: KaspaDbClient) -> u64 {
    let key = "transactions_outputs";
    let start_time = Instant::now();
    debug!("Processing {} {}", values.len(), key);
    let mut rows_affected = 0;
    for batch_values in values.chunks(max_batch_size as usize) {
        rows_affected += database.insert_transaction_outputs(batch_values).await.expect(format!("Insert {} FAILED", key).as_str());
    }
    debug!("Committed {} {} in {}ms", rows_affected, key, Instant::now().duration_since(start_time).as_millis());
    return rows_affected;
}

async fn insert_input_tx_addr(max_batch_size: u16, values: Vec<SqlHash>, database: KaspaDbClient) -> u64 {
    let key = "input addresses_transactions";
    let start_time = Instant::now();
    debug!("Processing {} transactions for {}", values.len(), key);
    let mut rows_affected = 0;
    for batch_values in values.chunks(max_batch_size as usize) {
        rows_affected +=
            database.insert_address_transactions_from_inputs(batch_values).await.expect(format!("Insert {} FAILED", key).as_str());
    }
    debug!("Committed {} {} in {}ms", rows_affected, key, Instant::now().duration_since(start_time).as_millis());
    return rows_affected;
}

async fn insert_output_tx_addr(max_batch_size: u16, values: Vec<AddressTransaction>, database: KaspaDbClient) -> u64 {
    let key = "output addresses_transactions";
    let start_time = Instant::now();
    debug!("Processing {} {}", values.len(), key);
    let mut rows_affected = 0;
    for batch_values in values.chunks(max_batch_size as usize) {
        rows_affected += database.insert_address_transactions(batch_values).await.expect(format!("Insert {} FAILED", key).as_str());
    }
    debug!("Committed {} {} in {}ms", rows_affected, key, Instant::now().duration_since(start_time).as_millis());
    return rows_affected;
}

async fn insert_block_txs(max_batch_size: u16, values: Vec<BlockTransaction>, database: KaspaDbClient) -> u64 {
    let key = "block/transaction mappings";
    let start_time = Instant::now();
    debug!("Processing {} {}", values.len(), key);
    let mut rows_affected = 0;
    for batch_values in values.chunks(max_batch_size as usize) {
        rows_affected += database.insert_block_transactions(batch_values).await.expect(format!("Insert {} FAILED", key).as_str());
    }
    debug!("Committed {} {} in {}ms", rows_affected, key, Instant::now().duration_since(start_time).as_millis());
    return rows_affected;
}
