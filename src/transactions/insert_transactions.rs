extern crate diesel;

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crossbeam_queue::ArrayQueue;
use diesel::pg::PgConnection;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::result::Error;
use diesel::{insert_into, sql_query, Connection, RunQueryDsl};
use itertools::Itertools;
use log::{debug, info, trace};
use tokio::task;
use tokio::time::sleep;

use crate::database::models::{AddressTransaction, BlockTransaction, Transaction, TransactionInput, TransactionOutput};
use crate::database::schema::transactions;
use crate::database::schema::{addresses_transactions, blocks_transactions, transactions_inputs, transactions_outputs};

// Max number of rows for insert statements:
const BATCH_INSERT_SIZE: usize = 3600;

pub async fn insert_txs_ins_outs(
    running: Arc<AtomicBool>,
    buffer_size: f64,
    db_transactions_queue: Arc<
        ArrayQueue<(Transaction, BlockTransaction, Vec<TransactionInput>, Vec<TransactionOutput>, Vec<AddressTransaction>)>,
    >,
    db_pool: Pool<ConnectionManager<PgConnection>>,
) {
    let max_set_size = (3000f64 * buffer_size) as usize; // Large sets helps us to filter duplicates during catch-up
    let mut transactions: HashSet<Transaction> = HashSet::with_capacity(max_set_size);
    let mut block_tx: HashSet<BlockTransaction> = HashSet::with_capacity(max_set_size);
    let mut tx_inputs: HashSet<TransactionInput> = HashSet::with_capacity(max_set_size * 2);
    let mut tx_outputs: HashSet<TransactionOutput> = HashSet::with_capacity(max_set_size * 2);
    let mut tx_addresses: HashSet<AddressTransaction> = HashSet::with_capacity(max_set_size * 2);
    let mut last_block_timestamp;
    let mut last_commit_time = Instant::now();

    while running.load(Ordering::Relaxed) {
        let transaction_option = db_transactions_queue.pop();
        if transaction_option.is_none() {
            sleep(Duration::from_millis(100)).await;
            continue;
        }
        let (transaction, block_transactions, inputs, outputs, addresses) = transaction_option.unwrap();
        last_block_timestamp = transaction.block_time;
        transactions.insert(transaction);
        block_tx.insert(block_transactions);
        tx_inputs.extend(inputs.into_iter());
        tx_outputs.extend(outputs.into_iter());
        tx_addresses.extend(addresses.into_iter());

        if block_tx.len() >= max_set_size || (block_tx.len() >= 1 && Instant::now().duration_since(last_commit_time).as_secs() > 2) {
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

            let db_pool_clone = db_pool.clone();
            let tx_handle = task::spawn_blocking(|| insert_transactions(transactions_vec, db_pool_clone));
            let db_pool_clone = db_pool.clone();
            let tx_inputs_handle = task::spawn_blocking(|| insert_transaction_inputs(inputs_vec, db_pool_clone));
            let db_pool_clone = db_pool.clone();
            let tx_outputs_handle = task::spawn_blocking(|| insert_transaction_outputs(outputs_vec, db_pool_clone));
            let db_pool_clone = db_pool.clone();
            let tx_out_addr_handle = task::spawn_blocking(|| insert_output_transaction_addresses(addresses_vec, db_pool_clone));

            let rows_affected_tx = tx_handle.await.unwrap();
            let rows_affected_tx_inputs = tx_inputs_handle.await.unwrap();
            let rows_affected_tx_outputs = tx_outputs_handle.await.unwrap();
            let mut rows_affected_tx_addresses = tx_out_addr_handle.await.unwrap();
            rows_affected_tx_addresses += insert_input_transaction_addresses(transaction_ids, db_pool.clone());
            // ^Needs to complete first to avoid incomplete block checkpoints
            let rows_affected_block_tx = insert_block_transaction(block_transactions_vec, db_pool.clone());

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

            transactions = HashSet::with_capacity(BATCH_INSERT_SIZE);
            block_tx = HashSet::with_capacity(BATCH_INSERT_SIZE);
            tx_inputs = HashSet::with_capacity(BATCH_INSERT_SIZE * 2);
            tx_outputs = HashSet::with_capacity(BATCH_INSERT_SIZE * 2);
            tx_addresses = HashSet::with_capacity(BATCH_INSERT_SIZE * 2);
            last_commit_time = Instant::now();
        }
    }
}

fn insert_input_transaction_addresses(values: Vec<Vec<u8>>, db_pool: Pool<ConnectionManager<PgConnection>>) -> usize {
    let key = "input addresses_transactions";
    let start_time = Instant::now();
    debug!("Processing {} transactions for {}", values.len(), key);
    let mut rows_affected = 0;
    let con = &mut db_pool.get().expect("Database connection FAILED");
    for batch_values in values.chunks(BATCH_INSERT_SIZE) {
        con.transaction(|con| {
            let query = format!(
                "INSERT INTO addresses_transactions (address, transaction_id, block_time) \
                    SELECT o.script_public_key_address, i.transaction_id, t.block_time \
                        FROM transactions_inputs i \
                        JOIN transactions t ON t.transaction_id = i.transaction_id \
                        JOIN transactions_outputs o ON o.transaction_id = i.previous_outpoint_hash AND o.index = i.previous_outpoint_index \
                    WHERE i.transaction_id IN ({0}) AND t.transaction_id IN ({0}) \
                    ON CONFLICT DO NOTHING", batch_values.iter().map(|v| format!("'\\x{}'", hex::encode(v))).join(", "));
            trace!("Executing {} query:\n{}", key, query);
            rows_affected += sql_query(query)
                .execute(con)
                .expect(format!("Commit {} FAILED", key).as_str());
            Ok::<_, Error>(())
        }).expect(format!("Commit {} FAILED", key).as_str());
    }
    debug!("Committed {} {} in {}ms", rows_affected, key, Instant::now().duration_since(start_time).as_millis());
    return rows_affected;
}

fn insert_output_transaction_addresses(values: Vec<AddressTransaction>, db_pool: Pool<ConnectionManager<PgConnection>>) -> usize {
    let key = "output addresses_transactions";
    let start_time = Instant::now();
    debug!("Processing {} {}", values.len(), key);
    let mut rows_affected = 0;
    let con = &mut db_pool.get().expect("Database connection FAILED");
    for batch_values in values.chunks(BATCH_INSERT_SIZE) {
        con.transaction(|con| {
            rows_affected += insert_into(addresses_transactions::dsl::addresses_transactions)
                .values(batch_values)
                .on_conflict_do_nothing() //Ignore conflicts as any conflicting rows will be identical
                .execute(con)
                .expect(format!("Commit {} FAILED", key).as_str());
            Ok::<_, Error>(())
        })
        .expect(format!("Commit {} FAILED", key).as_str());
    }
    debug!("Committed {} {} in {}ms", rows_affected, key, Instant::now().duration_since(start_time).as_millis());
    return rows_affected;
}

fn insert_transaction_outputs(values: Vec<TransactionOutput>, db_pool: Pool<ConnectionManager<PgConnection>>) -> usize {
    let key = "transactions_outputs";
    let start_time = Instant::now();
    debug!("Processing {} {}", values.len(), key);
    let mut rows_affected = 0;
    let con = &mut db_pool.get().expect("Database connection FAILED");
    for batch_values in values.chunks(BATCH_INSERT_SIZE) {
        con.transaction(|con| {
            rows_affected += insert_into(transactions_outputs::dsl::transactions_outputs)
                .values(batch_values)
                .on_conflict_do_nothing() //Ignore conflicts as any conflicting rows will be identical
                .execute(con)
                .expect(format!("Commit {} FAILED", key).as_str());
            Ok::<_, Error>(())
        })
        .expect(format!("Commit {} FAILED", key).as_str());
    }
    debug!("Committed {} {} in {}ms", rows_affected, key, Instant::now().duration_since(start_time).as_millis());
    return rows_affected;
}

fn insert_transaction_inputs(values: Vec<TransactionInput>, db_pool: Pool<ConnectionManager<PgConnection>>) -> usize {
    let key = "transaction_inputs";
    let start_time = Instant::now();
    debug!("Processing {} {}", values.len(), key);
    let mut rows_affected = 0;
    let con = &mut db_pool.get().expect("Database connection FAILED");
    for batch_values in values.chunks(BATCH_INSERT_SIZE) {
        con.transaction(|con| {
            rows_affected += insert_into(transactions_inputs::dsl::transactions_inputs)
                .values(batch_values)
                .on_conflict_do_nothing() //Ignore conflicts as any conflicting rows will be identical
                .execute(con)
                .expect(format!("Commit {} FAILED", key).as_str());
            Ok::<_, Error>(())
        })
        .expect(format!("Commit {} FAILED", key).as_str());
    }
    debug!("Committed {} {} in {}ms", rows_affected, key, Instant::now().duration_since(start_time).as_millis());
    return rows_affected;
}

fn insert_transactions(values: Vec<Transaction>, db_pool: Pool<ConnectionManager<PgConnection>>) -> usize {
    let key = "transactions";
    let start_time = Instant::now();
    debug!("Processing {} {}", values.len(), key);
    let mut rows_affected = 0;
    let con = &mut db_pool.get().expect("Database connection FAILED");
    for batch_values in values.chunks(BATCH_INSERT_SIZE) {
        con.transaction(|con| {
            rows_affected += insert_into(transactions::dsl::transactions)
                .values(batch_values)
                .on_conflict_do_nothing() //Ignore conflicts as any conflicting rows will be identical
                .execute(con)
                .expect(format!("Commit {} FAILED", key).as_str());
            Ok::<_, Error>(())
        })
        .expect(format!("Commit {} FAILED", key).as_str());
    }
    debug!("Committed {} {} in {}ms", rows_affected, key, Instant::now().duration_since(start_time).as_millis());
    return rows_affected;
}

fn insert_block_transaction(values: Vec<BlockTransaction>, db_pool: Pool<ConnectionManager<PgConnection>>) -> usize {
    let key = "block/transaction mappings";
    let start_time = Instant::now();
    debug!("Processing {} {}", values.len(), key);
    let mut rows_affected = 0;
    let con = &mut db_pool.get().expect("Database connection FAILED");
    for batch_values in values.chunks(BATCH_INSERT_SIZE) {
        con.transaction(|con| {
            rows_affected = insert_into(blocks_transactions::dsl::blocks_transactions)
                .values(batch_values)
                .on_conflict_do_nothing() //Ignore conflicts as any conflicting rows will be identical
                .execute(con)
                .expect(format!("Commit {} FAILED", key).as_str());
            Ok::<_, Error>(())
        })
        .expect(format!("Commit {} FAILED", key).as_str());
    }
    debug!("Committed {} {} in {}ms", rows_affected, key, Instant::now().duration_since(start_time).as_millis());
    return rows_affected;
}
