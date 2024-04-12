extern crate diesel;

use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use crossbeam_queue::ArrayQueue;
use diesel::{Connection, ExpressionMethods, insert_into, QueryDsl, RunQueryDsl};
use diesel::dsl::sql;
use diesel::pg::PgConnection;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::result::Error;
use diesel::upsert::excluded;
use log::info;
use tokio::time::sleep;

use crate::database::models::Transaction;
use crate::database::schema::transactions;

pub async fn insert_transactions(db_transactions_queue: Arc<ArrayQueue<Transaction>>, 
                                 db_pool: Pool<ConnectionManager<PgConnection>>) -> Result<(), ()> {
    const INSERT_QUEUE_SIZE: usize = 7500;
    info!("Insert transactions started");
    let mut insert_queue: HashSet<Transaction> = HashSet::with_capacity(INSERT_QUEUE_SIZE);
    let mut last_block_timestamp = 0;
    let mut last_commit_time = SystemTime::now();
    let mut rows_affected = 0;
    loop {
        if insert_queue.len() >= INSERT_QUEUE_SIZE || (insert_queue.len() >= 1 && SystemTime::now().duration_since(last_commit_time).unwrap().as_secs() > 2) {
            let con = &mut db_pool.get().expect("Database connection FAILED");
            con.transaction(|con| {
                let existing_transactions = transactions::dsl::transactions
                    .filter(transactions::transaction_id.eq_any(insert_queue.iter()
                        .map(|t| t.transaction_id.clone()).collect::<Vec<Vec<u8>>>()))
                    .load::<Transaction>(con)
                    .unwrap();
                for existing_transaction in existing_transactions {
                    let enqueued_transaction = insert_queue.get(&existing_transaction).unwrap().clone();
                    if existing_transaction.is_equal_to(&enqueued_transaction) {
                        insert_queue.remove(&enqueued_transaction);
                    }
                }
                rows_affected = insert_into(transactions::dsl::transactions)
                    .values(Vec::from_iter(&insert_queue))
                    .on_conflict(transactions::transaction_id)
                    .do_update()
                    .set((
                        transactions::subnetwork.eq(excluded(transactions::subnetwork)),
                        transactions::hash.eq(excluded(transactions::hash)),
                        transactions::mass.eq(excluded(transactions::mass)),
                        // Merge block_hash with array from existing row:
                        transactions::block_hash.eq(sql("(select array_agg(distinct x) from unnest(transactions.block_hash||excluded.block_hash) as t(x))")),
                        transactions::block_time.eq(excluded(transactions::block_time)),
                        // Keep is_accepted, as it might already be touched by the virtual chain processor
                        // Keep accepting_block_hash, as it might already be touched by the virtual chain processor
                    ))
                    .execute(con)
                    .expect("Commit transactions to database FAILED");
                Ok::<_, Error>(())
            }).expect("Commit transactions to database FAILED");
            info!("Committed {} new/updated transactions to database. Last block timestamp: {}", rows_affected,
                chrono::DateTime::from_timestamp_millis(last_block_timestamp as i64 * 1000).unwrap());
            insert_queue = HashSet::with_capacity(INSERT_QUEUE_SIZE);
            last_commit_time = SystemTime::now();
        }
        let transaction_option = db_transactions_queue.pop();
        if transaction_option.is_some() {
            let transaction = transaction_option.unwrap();
            last_block_timestamp = transaction.block_time.unwrap();
            let existing_transaction_option = insert_queue.get(&transaction);
            if existing_transaction_option.is_some() {
                let existing_transaction = &mut existing_transaction_option.unwrap().clone();
                existing_transaction.block_hash.append(&mut transaction.block_hash.clone());
                existing_transaction.block_hash.sort_unstable();
                existing_transaction.block_hash.dedup();
                insert_queue.insert(existing_transaction.clone());
            } else {
                insert_queue.insert(transaction);
            }
        } else {
            sleep(Duration::from_millis(100)).await;
        }
    }
}
