extern crate diesel;

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use crossbeam_queue::ArrayQueue;
use diesel::{Connection, ExpressionMethods, insert_into, QueryDsl, RunQueryDsl, sql_query};
use diesel::pg::PgConnection;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::result::Error;
use diesel::upsert::excluded;
use itertools::Itertools;
use log::{debug, info};
use tokio::time::sleep;

use crate::database::models::{BlockTransaction, Transaction, TransactionInput, TransactionInputKey, TransactionOutput, TransactionOutputKey};
use crate::database::schema::{blocks_transactions, transactions_inputs, transactions_outputs};
use crate::database::schema::transactions;

const INSERT_QUEUE_SIZE: usize = 7500;

pub async fn insert_txs_ins_outs(db_transactions_queue: Arc<ArrayQueue<(Transaction, BlockTransaction, Vec<TransactionInput>, Vec<TransactionOutput>)>>,
                                 db_pool: Pool<ConnectionManager<PgConnection>>) -> Result<(), ()> {
    let mut transactions: HashSet<Transaction> = HashSet::with_capacity(INSERT_QUEUE_SIZE);
    let mut block_transactions: HashSet<BlockTransaction> = HashSet::with_capacity(INSERT_QUEUE_SIZE);
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
        transactions.insert(transaction);
        block_transactions.insert(transaction_tuple.1);
        inputs.extend(transaction_tuple.2.into_iter());
        outputs.extend(transaction_tuple.3.into_iter());

        if block_transactions.len() >= INSERT_QUEUE_SIZE || (block_transactions.len() >= 1 && SystemTime::now().duration_since(last_commit_time).unwrap().as_secs() > 2) {
            // Ordered to make sure all items pertaining to a transaction is inserted before linking to blocks, to avoid an incomplete checkpoint
            let rows_affected_tx_outputs = insert_transaction_outputs(outputs, db_pool.clone());
            let rows_affected_tx_inputs = insert_transaction_inputs(inputs, db_pool.clone());
            let rows_affected_tx = insert_transactions(&mut transactions, db_pool.clone());
            let rows_affected_block_tx = insert_block_transaction(&mut block_transactions, db_pool.clone());

            info!("Committed {} transactions ({} block/tx, {} inputs, {} outputs) to database. Last block timestamp: {}",
                rows_affected_tx, rows_affected_block_tx, rows_affected_tx_inputs, rows_affected_tx_outputs,
                chrono::DateTime::from_timestamp_millis(last_block_timestamp as i64 * 1000).unwrap());

            transactions = HashSet::with_capacity(INSERT_QUEUE_SIZE);
            block_transactions = HashSet::with_capacity(INSERT_QUEUE_SIZE);
            inputs = vec![];
            outputs = vec![];
            last_commit_time = SystemTime::now();
        }
    }
}

fn insert_transaction_outputs(outputs: Vec<TransactionOutput>, db_pool: Pool<ConnectionManager<PgConnection>>) -> usize {
    let mut rows_affected = 0;
    let con = &mut db_pool.get().expect("Database connection FAILED");
    for chunked in &outputs.into_iter().chunks(INSERT_QUEUE_SIZE) {
        let mut chunked_outputs = chunked.into_iter().map(|i| (TransactionOutputKey {
            transaction_id: i.transaction_id.clone(),
            index: i.index,
        }, i)).collect::<HashMap<TransactionOutputKey, TransactionOutput>>();
        con.transaction(|con| {
            let start_time = SystemTime::now();
            let before_filter_len = chunked_outputs.len();
            debug!("Chunk size before filtering existing outputs: {}", chunked_outputs.len());
            sql_query(format!("SELECT transaction_id, index FROM transactions_outputs WHERE (transaction_id, index) IN ({})",
                              chunked_outputs.values().map(|to| format!("('\\x{}', {})", hex::encode(to.transaction_id.clone()), to.index)).collect::<Vec<String>>().join(", ")))
                .load::<TransactionOutputKey>(con)
                .unwrap().iter()
                .for_each(|to_key| { chunked_outputs.remove(to_key); });
            debug!("Filtered {} existing outputs in {}ms", before_filter_len - chunked_outputs.len(), SystemTime::now().duration_since(start_time).unwrap().as_millis());

            let start_time = SystemTime::now();
            rows_affected += insert_into(transactions_outputs::dsl::transactions_outputs)
                .values(Vec::from_iter(chunked_outputs.values())) // Fail on conflicts as they shouldn't happen
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
    for chunked in &inputs.into_iter().chunks(INSERT_QUEUE_SIZE) {
        let mut chunked_inputs = chunked.into_iter().map(|i| (TransactionInputKey {
            transaction_id: i.transaction_id.clone(),
            index: i.index,
        }, i)).collect::<HashMap<TransactionInputKey, TransactionInput>>();
        con.transaction(|con| {
            let start_time = SystemTime::now();
            let before_filter_len = chunked_inputs.len();
            debug!("Chunk size before filtering existing transaction inputs: {}", chunked_inputs.len());
            sql_query(format!("SELECT transaction_id, index FROM transactions_inputs WHERE (transaction_id, index) IN ({})",
                              chunked_inputs.values().map(|ti| format!("('\\x{}', {})", hex::encode(ti.transaction_id.clone()), ti.index)).collect::<Vec<String>>().join(", ")))
                .load::<TransactionInputKey>(con)
                .unwrap().iter()
                .for_each(|ti_key| { chunked_inputs.remove(ti_key); });
            debug!("Filtered {} existing transaction inputs in {}ms", before_filter_len - chunked_inputs.len(), SystemTime::now().duration_since(start_time).unwrap().as_millis());

            let start_time = SystemTime::now();
            rows_affected += insert_into(transactions_inputs::dsl::transactions_inputs)
                .values(Vec::from_iter(chunked_inputs.values()))  // Fail on conflicts as they shouldn't happen
                .execute(con)
                .expect("Commit transaction inputs to database FAILED");
            debug!("Committed {} transaction inputs in {}ms", rows_affected, SystemTime::now().duration_since(start_time).unwrap().as_millis());

            Ok::<_, Error>(())
        }).expect("Commit transactions to database FAILED");
    }
    return rows_affected;
}

fn insert_transactions(transactions: &mut HashSet<Transaction>, db_pool: Pool<ConnectionManager<PgConnection>>) -> usize {
    let mut rows_affected = 0;
    let con = &mut db_pool.get().expect("Database connection FAILED");
    con.transaction(|con| {
        sql_query("LOCK TABLE transactions IN EXCLUSIVE MODE").execute(con).expect("Locking table before commit transactions to database FAILED");

        let start_time = SystemTime::now();
        let before_filter_len = transactions.len();
        debug!("Size before filtering existing transactions: {}", transactions.len());
        transactions::dsl::transactions
            .filter(transactions::transaction_id.eq_any(transactions.iter().map(|t| t.transaction_id.clone()).collect::<Vec<Vec<u8>>>()))
            .load::<Transaction>(con)
            .unwrap().iter()
            .for_each(|t| if t.subnetwork.is_some() { transactions.remove(t); }); // VCP only writes acceptance data
        debug!("Filtered {} existing transactions in {}ms", before_filter_len - transactions.len(), SystemTime::now().duration_since(start_time).unwrap().as_millis());

        let start_time = SystemTime::now();
        rows_affected = insert_into(transactions::dsl::transactions)
            .values(Vec::from_iter(transactions.iter()))
            .on_conflict(transactions::transaction_id)
            .do_update() //Upsert transactions in case a conflicting tx was persisted
            .set((
                transactions::subnetwork.eq(excluded(transactions::subnetwork)),
                transactions::hash.eq(excluded(transactions::hash)),
                transactions::mass.eq(excluded(transactions::mass)),
                transactions::block_time.eq(excluded(transactions::block_time)),
                // Keep is_accepted, as it might already be touched by the virtual chain processor
                // Keep accepting_block_hash, as it might already be touched by the virtual chain processor
            ))
            .execute(con)
            .expect("Commit transactions to database FAILED");
        debug!("Committed {} transactions in {}ms", rows_affected, SystemTime::now().duration_since(start_time).unwrap().as_millis());

        Ok::<_, Error>(())
    }).expect("Commit transactions to database FAILED");
    return rows_affected;
}

fn insert_block_transaction(block_transactions: &mut HashSet<BlockTransaction>, db_pool: Pool<ConnectionManager<PgConnection>>) -> usize {
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
