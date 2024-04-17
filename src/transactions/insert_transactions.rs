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
use tokio::task;
use tokio::time::sleep;

use crate::database::models::{BlockTransaction, Transaction, TransactionInput, TransactionOutput};
use crate::database::schema::{blocks_transactions, transactions_inputs, transactions_outputs};
use crate::database::schema::transactions;

const INSERT_QUEUE_SIZE: usize = 7500;

pub async fn insert_txs_ins_outs(db_transactions_queue: Arc<ArrayQueue<(Transaction, BlockTransaction, Vec<TransactionInput>, Vec<TransactionOutput>)>>,
                                 db_pool: Pool<ConnectionManager<PgConnection>>) -> Result<(), ()> {
    let mut transactions: Vec<Transaction> = vec![];
    let mut block_transactions: Vec<BlockTransaction> = vec![];
    let mut inputs: Vec<TransactionInput> = vec![];
    let mut outputs: Vec<TransactionOutput> = vec![];
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
        transactions.push(transaction);
        block_transactions.push(transaction_tuple.1);
        inputs.extend(transaction_tuple.2.into_iter());
        outputs.extend(transaction_tuple.3.into_iter());

        if block_transactions.len() >= INSERT_QUEUE_SIZE || (block_transactions.len() >= 1 && SystemTime::now().duration_since(last_commit_time).unwrap().as_secs() > 2) {
            info!("Processing {} transactions ({} block/tx, {} inputs, {} outputs)", 
                transactions.len(), block_transactions.len(), inputs.len(), outputs.len());
            let db_pool_clone = db_pool.clone();
            let tx_outputs_handle = task::spawn_blocking(|| { insert_transaction_outputs(outputs, db_pool_clone) });
            let db_pool_clone = db_pool.clone();
            let tx_inputs_handle = task::spawn_blocking(|| { insert_transaction_inputs(inputs, db_pool_clone) });
            let db_pool_clone = db_pool.clone();
            let tx_handle = task::spawn_blocking(|| { insert_transactions(transactions, db_pool_clone) });

            let rows_affected_tx_outputs = tx_outputs_handle.await.unwrap();
            let rows_affected_tx_inputs = tx_inputs_handle.await.unwrap();
            let rows_affected_tx = tx_handle.await.unwrap();
            // ^Needs to complete first to avoid incomplete block checkpoints
            let rows_affected_block_tx = insert_block_transaction(block_transactions, db_pool.clone());

            info!("Committed {} transactions ({} block/tx, {} inputs, {} outputs) to database. Last block timestamp: {}",
                rows_affected_tx, rows_affected_block_tx, rows_affected_tx_inputs, rows_affected_tx_outputs,
                chrono::DateTime::from_timestamp_millis(last_block_timestamp as i64 * 1000).unwrap());

            transactions = vec![];
            block_transactions = vec![];
            inputs = vec![];
            outputs = vec![];
            last_commit_time = SystemTime::now();
        }
    }
}

fn insert_transaction_outputs(outputs: Vec<TransactionOutput>, db_pool: Pool<ConnectionManager<PgConnection>>) -> usize {
    let mut rows_affected = 0;
    let con = &mut db_pool.get().expect("Database connection FAILED");
    for chunked_outputs in outputs.chunks(INSERT_QUEUE_SIZE) {
        let chunked_outputs: HashSet<&TransactionOutput> = HashSet::from_iter(chunked_outputs.into_iter());
        con.transaction(|con| {
            let start_time = SystemTime::now();
            debug!("Processing {} transaction outputs", chunked_outputs.len());
            rows_affected += insert_into(transactions_outputs::dsl::transactions_outputs)
                .values(Vec::from_iter(chunked_outputs.into_iter()))
                .on_conflict_do_nothing()
                .execute(con)
                .expect("Commit transaction outputs to database FAILED");
            debug!("Committed {} transaction outputs in {}ms", rows_affected, SystemTime::now().duration_since(start_time).unwrap().as_millis());
            Ok::<_, Error>(())
        }).expect("Commit transactions to database FAILED");
    }
    return rows_affected;
}

fn insert_transaction_inputs(inputs: Vec<TransactionInput>, db_pool: Pool<ConnectionManager<PgConnection>>) -> usize {
    let mut rows_affected = 0;
    let con = &mut db_pool.get().expect("Database connection FAILED");
    for chunked_inputs in inputs.chunks(INSERT_QUEUE_SIZE) {
        let chunked_inputs: HashSet<&TransactionInput> = HashSet::from_iter(chunked_inputs.into_iter());
        con.transaction(|con| {
            let start_time = SystemTime::now();
            debug!("Processing {} transaction inputs", chunked_inputs.len());
            rows_affected += insert_into(transactions_inputs::dsl::transactions_inputs)
                .values(Vec::from_iter(chunked_inputs.into_iter()))
                .on_conflict_do_nothing()
                .execute(con)
                .expect("Commit transaction inputs to database FAILED");
            debug!("Committed {} transaction inputs in {}ms", rows_affected, SystemTime::now().duration_since(start_time).unwrap().as_millis());
            Ok::<_, Error>(())
        }).expect("Commit transactions to database FAILED");
    }
    return rows_affected;
}

fn insert_transactions(transactions: Vec<Transaction>, db_pool: Pool<ConnectionManager<PgConnection>>) -> usize {
    let transactions: HashSet<Transaction> = HashSet::from_iter(transactions.into_iter());
    let mut rows_affected = 0;
    let con = &mut db_pool.get().expect("Database connection FAILED");
    con.transaction(|con| {
        debug!("Processing {} transactions", transactions.len());
        let start_time = SystemTime::now();
        rows_affected = insert_into(transactions::dsl::transactions)
            .values(Vec::from_iter(transactions.iter()))
            .on_conflict_do_nothing()
            .execute(con)
            .expect("Commit transactions to database FAILED");
        debug!("Committed {} transactions in {}ms", rows_affected, SystemTime::now().duration_since(start_time).unwrap().as_millis());
        Ok::<_, Error>(())
    }).expect("Commit transactions to database FAILED");
    return rows_affected;
}

fn insert_block_transaction(block_transactions: Vec<BlockTransaction>, db_pool: Pool<ConnectionManager<PgConnection>>) -> usize {
    let block_transactions: HashSet<BlockTransaction> = HashSet::from_iter(block_transactions.into_iter());
    let mut rows_affected = 0;
    let con = &mut db_pool.get().expect("Database connection FAILED");
    con.transaction(|con| {
        let start_time = SystemTime::now();
        rows_affected = insert_into(blocks_transactions::dsl::blocks_transactions)
            .values(Vec::from_iter(block_transactions.iter()))
            .on_conflict_do_nothing() //Ignore conflicts as any conflicting rows will be identical
            .execute(con)
            .expect("Commit block/transaction mappings to database FAILED");
        debug!("Committed {} block/transaction mappings in {}ms", rows_affected, SystemTime::now().duration_since(start_time).unwrap().as_millis());
        Ok::<_, Error>(())
    }).expect("Commit block/transaction mappings to database FAILED");
    return rows_affected;
}
