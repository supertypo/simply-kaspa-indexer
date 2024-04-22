extern crate diesel;

use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use crossbeam_queue::ArrayQueue;
use diesel::{Connection, insert_into, RunQueryDsl};
use diesel::pg::PgConnection;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::result::Error;
use log::{debug, info};
use tokio::time::sleep;

use crate::database::models::{BlockTransaction, Transaction, TransactionInput, TransactionOutput};
use crate::database::schema::{blocks_transactions, transactions_inputs, transactions_outputs};
use crate::database::schema::transactions;

// A large queue helps us to filter duplicates during catch-up:
const SET_SIZE: usize = 30000;
// Max number of rows for insert statements:
const INSERT_SIZE: usize = 9000;

pub async fn insert_txs_ins_outs(db_transactions_queue: Arc<ArrayQueue<(Transaction, BlockTransaction, Vec<TransactionInput>, Vec<TransactionOutput>)>>,
                                 db_pool: Pool<ConnectionManager<PgConnection>>) -> Result<(), ()> {
    let mut transactions: HashSet<Transaction> = HashSet::with_capacity(SET_SIZE);
    let mut block_tx: HashSet<BlockTransaction> = HashSet::with_capacity(SET_SIZE);
    let mut tx_inputs: HashSet<TransactionInput> = HashSet::with_capacity(SET_SIZE * 2);
    let mut tx_outputs: HashSet<TransactionOutput> = HashSet::with_capacity(SET_SIZE * 2);
    let mut last_block_timestamp;
    let mut last_commit_time = SystemTime::now();
    loop {
        let transaction_option = db_transactions_queue.pop();
        if transaction_option.is_none() {
            sleep(Duration::from_millis(100)).await;
            continue;
        }
        let transaction_tuple = transaction_option.unwrap();
        let transaction = transaction_tuple.0;
        last_block_timestamp = transaction.block_time.unwrap();
        transactions.insert(transaction);
        block_tx.insert(transaction_tuple.1);
        tx_inputs.extend(transaction_tuple.2.into_iter());
        tx_outputs.extend(transaction_tuple.3.into_iter());

        if block_tx.len() >= SET_SIZE || (block_tx.len() >= 1 && SystemTime::now().duration_since(last_commit_time).unwrap().as_secs() > 2) {
            debug!("Committing {} transactions ({} block/tx, {} inputs, {} outputs)",
                transactions.len(), block_tx.len(), tx_inputs.len(), tx_outputs.len());
            // We used a HashSet first to filter some amount of duplicates locally, now we can switch back to vector:
            let transactions_vec = transactions.into_iter().collect();
            let block_transactions_vec = block_tx.into_iter().collect();
            let inputs_vec = tx_inputs.into_iter().collect();
            let outputs_vec = tx_outputs.into_iter().collect();

            let (rows_affected_tx, rows_affected_tx_inputs, rows_affected_tx_outputs) = tokio::join!(
                insert_transactions(transactions_vec, db_pool.clone()),
                insert_transaction_inputs(inputs_vec, db_pool.clone()),
                insert_transaction_outputs(outputs_vec, db_pool.clone()));
            // Delay block/tx mapping until all is in place to make sure we don't get incomplete block checkpoints
            let rows_affected_block_tx = insert_block_transaction(block_transactions_vec, db_pool.clone()).await;

            let dv = (10000 / SystemTime::now().duration_since(last_commit_time).unwrap().as_millis()) as f64 / 10f64;
            info!("Committed {} transactions ({:.1} tps, {} block_txs, {} inputs, {} outputs). Last block timestamp: {}",
                rows_affected_tx, rows_affected_tx as f64 * dv, rows_affected_block_tx, rows_affected_tx_inputs, rows_affected_tx_outputs,
                chrono::DateTime::from_timestamp_millis(last_block_timestamp as i64 * 1000).unwrap());

            transactions = HashSet::with_capacity(INSERT_SIZE);
            block_tx = HashSet::with_capacity(INSERT_SIZE);
            tx_inputs = HashSet::with_capacity(INSERT_SIZE * 2);
            tx_outputs = HashSet::with_capacity(INSERT_SIZE * 2);
            last_commit_time = SystemTime::now();
        }
    }
}

async fn insert_transaction_outputs(values: Vec<TransactionOutput>, db_pool: Pool<ConnectionManager<PgConnection>>) -> usize {
    let key = "transactions_outputs";
    let start_time = SystemTime::now();
    debug!("Processing {} {}", values.len(), key);
    let mut rows_affected = 0;
    let con = &mut db_pool.get().expect("Database connection FAILED");
    for batch_values in values.chunks(INSERT_SIZE) {
        con.transaction(|con| {
            rows_affected += insert_into(transactions_outputs::dsl::transactions_outputs)
                .values(batch_values)
                .on_conflict_do_nothing() //Ignore conflicts as any conflicting rows will be identical
                .execute(con)
                .expect(format!("Commit {} FAILED", key).as_str());
            Ok::<_, Error>(())
        }).expect(format!("Commit {} FAILED", key).as_str());
    }
    debug!("Committed {} {} in {}ms", rows_affected, key, SystemTime::now().duration_since(start_time).unwrap().as_millis());
    return rows_affected;
}

async fn insert_transaction_inputs(values: Vec<TransactionInput>, db_pool: Pool<ConnectionManager<PgConnection>>) -> usize {
    let key = "transaction_inputs";
    let start_time = SystemTime::now();
    debug!("Processing {} {}", values.len(), key);
    let mut rows_affected = 0;
    let con = &mut db_pool.get().expect("Database connection FAILED");
    for batch_values in values.chunks(INSERT_SIZE) {
        con.transaction(|con| {
            rows_affected += insert_into(transactions_inputs::dsl::transactions_inputs)
                .values(batch_values)
                .on_conflict_do_nothing() //Ignore conflicts as any conflicting rows will be identical
                .execute(con)
                .expect(format!("Commit {} FAILED", key).as_str());
            Ok::<_, Error>(())
        }).expect(format!("Commit {} FAILED", key).as_str());
    }
    debug!("Committed {} {} in {}ms", rows_affected, key, SystemTime::now().duration_since(start_time).unwrap().as_millis());
    return rows_affected;
}

async fn insert_transactions(values: Vec<Transaction>, db_pool: Pool<ConnectionManager<PgConnection>>) -> usize {
    let key = "transactions";
    let start_time = SystemTime::now();
    debug!("Processing {} {}", values.len(), key);
    let mut rows_affected = 0;
    let con = &mut db_pool.get().expect("Database connection FAILED");
    for batch_values in values.chunks(INSERT_SIZE) {
        con.transaction(|con| {
            rows_affected += insert_into(transactions::dsl::transactions)
                .values(batch_values)
                .on_conflict_do_nothing() //Ignore conflicts as any conflicting rows will be identical
                .execute(con)
                .expect(format!("Commit {} FAILED", key).as_str());
            Ok::<_, Error>(())
        }).expect(format!("Commit {} FAILED", key).as_str());
    }
    debug!("Committed {} {} in {}ms", rows_affected, key, SystemTime::now().duration_since(start_time).unwrap().as_millis());
    return rows_affected;
}

async fn insert_block_transaction(values: Vec<BlockTransaction>, db_pool: Pool<ConnectionManager<PgConnection>>) -> usize {
    let key = "block/transaction mappings";
    let start_time = SystemTime::now();
    debug!("Processing {} {}", values.len(), key);
    let mut rows_affected = 0;
    let con = &mut db_pool.get().expect("Database connection FAILED");
    for batch_values in values.chunks(INSERT_SIZE) {
        con.transaction(|con| {
            rows_affected = insert_into(blocks_transactions::dsl::blocks_transactions)
                .values(batch_values)
                .on_conflict_do_nothing() //Ignore conflicts as any conflicting rows will be identical
                .execute(con)
                .expect(format!("Commit {} FAILED", key).as_str());
            Ok::<_, Error>(())
        }).expect(format!("Commit {} FAILED", key).as_str());
    }
    debug!("Committed {} {} in {}ms", rows_affected, key, SystemTime::now().duration_since(start_time).unwrap().as_millis());
    return rows_affected;
}
