extern crate diesel;

use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use crossbeam_queue::ArrayQueue;
use diesel::{Connection, ExpressionMethods, insert_into, QueryDsl, RunQueryDsl, sql_query};
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
    let mut insert_tx_queue: HashSet<Transaction> = HashSet::with_capacity(INSERT_QUEUE_SIZE);
    let mut insert_block_tx_queue: HashSet<BlockTransaction> = HashSet::with_capacity(INSERT_QUEUE_SIZE);
    let mut insert_tx_inputs_queue: HashSet<TransactionInput> = HashSet::with_capacity(INSERT_QUEUE_SIZE);
    let mut insert_tx_outputs_queue: HashSet<TransactionOutput> = HashSet::with_capacity(INSERT_QUEUE_SIZE);
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
        insert_tx_queue.insert(transaction);
        insert_block_tx_queue.insert(transaction_tuple.1);
        insert_tx_inputs_queue.extend(transaction_tuple.2.into_iter());
        insert_tx_outputs_queue.extend(transaction_tuple.3.into_iter());

        if insert_block_tx_queue.len() >= INSERT_QUEUE_SIZE || (insert_block_tx_queue.len() >= 1 && SystemTime::now().duration_since(last_commit_time).unwrap().as_secs() > 2) {
            // Ordered to make sure all items pertaining to a transaction is inserted before linking to blocks, to avoid an incomplete checkpoint
            let rows_affected_tx_outputs = insert_transaction_outputs(insert_tx_outputs_queue, db_pool.clone());
            let rows_affected_tx_inputs = insert_transaction_inputs(insert_tx_inputs_queue, db_pool.clone());
            let rows_affected_tx = insert_transactions(&mut insert_tx_queue, db_pool.clone());
            let rows_affected_block_tx = insert_block_transaction(&mut insert_block_tx_queue, db_pool.clone());

            info!("Committed {} transactions ({} block/tx, {} inputs, {} outputs) to database. Last block timestamp: {}",
                rows_affected_tx, rows_affected_block_tx, rows_affected_tx_inputs, rows_affected_tx_outputs,
                chrono::DateTime::from_timestamp_millis(last_block_timestamp as i64 * 1000).unwrap());

            insert_tx_queue = HashSet::with_capacity(INSERT_QUEUE_SIZE);
            insert_block_tx_queue = HashSet::with_capacity(INSERT_QUEUE_SIZE);
            insert_tx_inputs_queue = HashSet::with_capacity(INSERT_QUEUE_SIZE);
            insert_tx_outputs_queue = HashSet::with_capacity(INSERT_QUEUE_SIZE);
            last_commit_time = SystemTime::now();
        }
    }
}

fn insert_transaction_outputs(insert_tx_outputs_queue: HashSet<TransactionOutput>, db_pool: Pool<ConnectionManager<PgConnection>>) -> usize {
    let mut rows_affected = 0;
    let con = &mut db_pool.get().expect("Database connection FAILED");
    for chunked in Vec::from_iter(insert_tx_outputs_queue).chunks(INSERT_QUEUE_SIZE) {
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
            debug!("Chunk size after filtering existing outputs: {}, took {}ms", chunked_outputs.len(), SystemTime::now().duration_since(start_time).unwrap().as_millis());

            // Fail on conflicts as they shouldn't happen
            rows_affected += insert_into(transactions_outputs::dsl::transactions_outputs)
                .values(Vec::from_iter(chunked_outputs))
                .execute(con)
                .expect("Commit transaction outputs to database FAILED");

            Ok::<_, Error>(())
        }).expect("Commit transactions to database FAILED");
    }
    return rows_affected;
}

fn insert_transaction_inputs(insert_tx_inputs_queue: HashSet<TransactionInput>, db_pool: Pool<ConnectionManager<PgConnection>>) -> usize {
    let mut rows_affected = 0;
    let con = &mut db_pool.get().expect("Database connection FAILED");
    for chunked in Vec::from_iter(insert_tx_inputs_queue).chunks(INSERT_QUEUE_SIZE) {
        let mut chunked_inputs: HashSet<&TransactionInput> = HashSet::from_iter(chunked);
        con.transaction(|con| {
            let start_time = SystemTime::now();
            debug!("Chunk size before filtering existing inputs: {}", chunked_inputs.len());
            sql_query(format!("SELECT * FROM transactions_inputs WHERE (transaction_id, index) IN ({})",
                              chunked_inputs.iter().map(|ti| format!("('\\x{}', {})", hex::encode(ti.transaction_id.clone()), ti.index)).collect::<Vec<String>>().join(", ")))
                .load::<TransactionInput>(con)
                .unwrap().iter()
                .for_each(|ti| {
                    let new = chunked_inputs.get(ti).unwrap();
                    if !ti.is_similar(new) { panic!("Was about to remove a different transaction input:\n\nOriginal: {:#?}\n\nNew: {:#?}", ti, new) };
                    chunked_inputs.remove(ti);
                });
            debug!("Chunk size after filtering existing inputs: {}, took {}ms", chunked_inputs.len(), SystemTime::now().duration_since(start_time).unwrap().as_millis());

            // Fail on conflicts as they shouldn't happen
            rows_affected += insert_into(transactions_inputs::dsl::transactions_inputs)
                .values(Vec::from_iter(chunked_inputs))
                .execute(con)
                .expect("Commit transaction inputs to database FAILED");

            Ok::<_, Error>(())
        }).expect("Commit transactions to database FAILED");
    }
    return rows_affected;
}

fn insert_transactions(insert_tx_queue: &mut HashSet<Transaction>, db_pool: Pool<ConnectionManager<PgConnection>>) -> usize {
    let mut rows_affected = 0;
    let con = &mut db_pool.get().expect("Database connection FAILED");
    con.transaction(|con| {
        let start_time = SystemTime::now();
        debug!("Size before filtering existing transactions: {}", insert_tx_queue.len());
        transactions::dsl::transactions
            .filter(transactions::transaction_id.eq_any(insert_tx_queue.iter()
                .map(|t| t.transaction_id.clone()).collect::<Vec<Vec<u8>>>()))
            .load::<Transaction>(con)
            .unwrap().iter()
            .for_each(|t| { insert_tx_queue.remove(t); });
        debug!("Size after filtering existing transactions: {}, took {}ms", insert_tx_queue.len(), SystemTime::now().duration_since(start_time).unwrap().as_millis());

        //Upsert transactions in case a conflicting tx was persisted
        rows_affected = insert_into(transactions::dsl::transactions)
            .values(Vec::from_iter(insert_tx_queue.iter()))
            .on_conflict(transactions::transaction_id)
            .do_update()
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

        Ok::<_, Error>(())
    }).expect("Commit transactions to database FAILED");
    return rows_affected;
}

fn insert_block_transaction(insert_block_tx_queue: &mut HashSet<BlockTransaction>, db_pool: Pool<ConnectionManager<PgConnection>>) -> usize {
    let mut rows_affected = 0;
    let con = &mut db_pool.get().expect("Database connection FAILED");
    con.transaction(|con| {
        let start_time = SystemTime::now();
        debug!("Size before filtering existing block_transactions: {}", insert_block_tx_queue.len());
        sql_query(format!("SELECT * FROM blocks_transactions WHERE (block_hash, transaction_id) IN ({})",
                          insert_block_tx_queue.iter().map(|bt| format!("('\\x{}', '\\x{}')", hex::encode(bt.block_hash.clone()), hex::encode(bt.transaction_id.clone()))).collect::<Vec<String>>().join(", ")))
            .load::<BlockTransaction>(con)
            .unwrap().iter()
            .for_each(|bt| { insert_block_tx_queue.remove(bt); });
        debug!("Size after filtering existing block_transactions: {}, took {}ms", insert_block_tx_queue.len(), SystemTime::now().duration_since(start_time).unwrap().as_millis());

        //Ignore conflicts as any conflicting rows will be identical
        rows_affected = insert_into(blocks_transactions::dsl::blocks_transactions)
            .values(Vec::from_iter(insert_block_tx_queue.iter()))
            .on_conflict(on_constraint("pk_blocks_transactions"))
            .do_nothing()
            .execute(con)
            .expect("Commit block/transaction mappings to database FAILED");

        Ok::<_, Error>(())
    }).expect("Commit transactions to database FAILED");
    return rows_affected;
}
