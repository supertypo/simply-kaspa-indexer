extern crate diesel;

use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use crossbeam_queue::ArrayQueue;
use diesel::{BoolExpressionMethods, Connection, ExpressionMethods, insert_into, QueryDsl, RunQueryDsl, sql_query};
use diesel::pg::PgConnection;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::result::Error;
use diesel::upsert::{excluded, on_constraint};
use log::{debug, info};
use tokio::time::sleep;

use crate::database::models::{BlockTransaction, Similar, Transaction, TransactionInput, TransactionOutput};
use crate::database::schema::{blocks_transactions, transactions_inputs, transactions_outputs};
use crate::database::schema::transactions;

const INSERT_QUEUE_SIZE: usize = 5000;

pub async fn insert_txs_ins_outs(db_transactions_queue: Arc<ArrayQueue<(Transaction, BlockTransaction, Vec<TransactionInput>, Vec<TransactionOutput>)>>,
                                 db_pool: Pool<ConnectionManager<PgConnection>>) -> Result<(), ()> {
    let mut transactions: HashSet<Transaction> = HashSet::with_capacity(INSERT_QUEUE_SIZE);
    let mut block_transactions: HashSet<BlockTransaction> = HashSet::with_capacity(INSERT_QUEUE_SIZE);
    let mut inputs: HashSet<TransactionInput> = HashSet::with_capacity(INSERT_QUEUE_SIZE);
    let mut outputs: HashSet<TransactionOutput> = HashSet::with_capacity(INSERT_QUEUE_SIZE);
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

        // insert_block_tx_queue will be larger than insert_tx_queue as multiple blocks can reference the same tx
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
            inputs = HashSet::with_capacity(INSERT_QUEUE_SIZE);
            outputs = HashSet::with_capacity(INSERT_QUEUE_SIZE);
            last_commit_time = SystemTime::now();
        }
    }
}

fn insert_transaction_outputs(outputs: HashSet<TransactionOutput>, db_pool: Pool<ConnectionManager<PgConnection>>) -> usize {
    let mut rows_affected = 0;
    let con = &mut db_pool.get().expect("Database connection FAILED");
    for chunked in Vec::from_iter(outputs).chunks(INSERT_QUEUE_SIZE) {
        let mut chunked_outputs: HashSet<&TransactionOutput> = HashSet::from_iter(chunked);
        con.transaction(|con| {
            let start_time = SystemTime::now();
            debug!("Chunk size before filtering existing outputs: {}", chunked_outputs.len());
            sql_query(format!("SELECT * FROM transactions_outputs WHERE (transaction_id, index) IN ({})",
                              chunked_outputs.iter().map(|ti| format!("('\\x{}', {})", hex::encode(ti.transaction_id.clone()), ti.index)).collect::<Vec<String>>().join(", ")))
                .load::<TransactionOutput>(con)
                .unwrap().iter()
                .for_each(|to| {
                    if !to.is_similar(chunked_outputs.get(to).unwrap()) { panic!("Was about to remove a different transaction output") };
                    chunked_outputs.remove(to);
                });
            debug!("Filtered {} existing outputs in {} ms", chunked_outputs.len(), SystemTime::now().duration_since(start_time).unwrap().as_millis());

            let len_inputs = chunked_outputs.len();
            let start_time = SystemTime::now();
            rows_affected += insert_into(transactions_outputs::dsl::transactions_outputs)
                .values(Vec::from_iter(chunked_outputs)) // Fail on conflicts as they shouldn't happen
                .execute(con)
                .expect("Commit transaction outputs to database FAILED");
            debug!("Committed {} transaction outputs in {}ms", len_inputs, SystemTime::now().duration_since(start_time).unwrap().as_millis());

            Ok::<_, Error>(())
        }).expect("Commit transactions to database FAILED");
    }
    return rows_affected;
}

fn insert_transaction_inputs(inputs: HashSet<TransactionInput>, db_pool: Pool<ConnectionManager<PgConnection>>) -> usize {
    let mut rows_affected = 0;
    let con = &mut db_pool.get().expect("Database connection FAILED");
    for chunked in Vec::from_iter(inputs).chunks(INSERT_QUEUE_SIZE) {
        let mut chunked_inputs: HashSet<&TransactionInput> = HashSet::from_iter(chunked);
        con.transaction(|con| {
            let start_time = SystemTime::now();
            debug!("Chunk size before filtering existing transaction inputs: {}", chunked_inputs.len());
            sql_query(format!("SELECT * FROM transactions_inputs WHERE (transaction_id, index) IN ({})",
                              chunked_inputs.iter().map(|ti| format!("('\\x{}', {})", hex::encode(ti.transaction_id.clone()), ti.index)).collect::<Vec<String>>().join(", ")))
                .load::<TransactionInput>(con)
                .unwrap().iter()
                .for_each(|ti| {
                    let new = chunked_inputs.get(ti).unwrap();
                    if !ti.is_similar(new) { panic!("Was about to remove a different transaction input:\n\nOriginal: {:#?}\n\nNew: {:#?}", ti, new) };
                    chunked_inputs.remove(ti);
                });
            debug!("Filtered {} existing transaction inputs in {} ms", chunked_inputs.len(), SystemTime::now().duration_since(start_time).unwrap().as_millis());

            let len_inputs = chunked_inputs.len();
            let start_time = SystemTime::now();
            rows_affected += insert_into(transactions_inputs::dsl::transactions_inputs)
                .values(Vec::from_iter(chunked_inputs))  // Fail on conflicts as they shouldn't happen
                .execute(con)
                .expect("Commit transaction inputs to database FAILED");
            debug!("Committed {} transaction inputs in {}ms", len_inputs, SystemTime::now().duration_since(start_time).unwrap().as_millis());

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
        debug!("Size before filtering existing transactions: {}", transactions.len());
        transactions::dsl::transactions
            .filter(transactions::transaction_id.eq_any(transactions.iter()
                .map(|t| t.transaction_id.clone()).collect::<Vec<Vec<u8>>>())
                .and(transactions::subnetwork.is_not_null())) // VCP only writes acceptance data
            .load::<Transaction>(con)
            .unwrap().iter()
            .for_each(|t| { transactions.remove(t); });
        debug!("Filtered {} existing transactions in {} ms", transactions.len(), SystemTime::now().duration_since(start_time).unwrap().as_millis());

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
        debug!("Committed {} transactions in {}ms", transactions.len(), SystemTime::now().duration_since(start_time).unwrap().as_millis());

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
        debug!("Committed {} block/transaction mappings in {}ms", block_transactions.len(), SystemTime::now().duration_since(start_time).unwrap().as_millis());
        Ok::<_, Error>(())
    }).expect("Commit block/transaction mappings to database FAILED");
    return rows_affected;
}
