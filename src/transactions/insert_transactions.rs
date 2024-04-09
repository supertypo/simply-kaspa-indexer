extern crate diesel;

use std::collections::HashSet;
use std::ops::DerefMut;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use crossbeam_queue::ArrayQueue;
use diesel::{Connection, ExpressionMethods, insert_into, RunQueryDsl};
use diesel::dsl::sql;
use diesel::pg::PgConnection;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::result::Error;
use log::{error, info};
use tokio::time::sleep;

use crate::database::models::Transaction;
use crate::database::schema::transactions;

pub async fn insert_transactions(db_transactions_queue: Arc<ArrayQueue<Transaction>>, db_pool: Pool<ConnectionManager<PgConnection>>) -> Result<(), ()> {
    const INSERT_QUEUE_SIZE: usize = 7500;
    loop {
        info!("Insert transactions started");
        let mut insert_queue: HashSet<Transaction> = HashSet::with_capacity(INSERT_QUEUE_SIZE);
        let mut last_commit_time = SystemTime::now();
        loop {
            if insert_queue.len() >= INSERT_QUEUE_SIZE || (insert_queue.len() >= 1 && SystemTime::now().duration_since(last_commit_time).unwrap().as_secs() > 2) {
                let con_option = match db_pool.get() {
                    Ok(r) => { Some(r) }
                    Err(e) => {
                        info!("Database connection failed with: {}. Sleeping for 10 seconds...", e);
                        sleep(Duration::from_secs(10)).await;
                        None
                    }
                };
                if con_option.is_none() {
                    continue;
                }
                let _ = con_option.unwrap().deref_mut().transaction(|con| {
                    match insert_into(transactions::dsl::transactions)
                        .values(Vec::from_iter(&insert_queue))
                        .on_conflict(transactions::transaction_id)
                        .do_update()
                        .set(transactions::block_hash.eq(sql("(select array_agg(distinct x) from unnest(transactions.block_hash||excluded.block_hash) as t(x))")))
                        .execute(con) {
                        Ok(inserted_rows) => {
                            info!("Committed {} new/updated transactions to database", inserted_rows);
                            insert_queue = HashSet::with_capacity(INSERT_QUEUE_SIZE);
                            last_commit_time = SystemTime::now();
                        }
                        Err(e) => {
                            error!("Commit transactions to database failed with: {e}. Sleeping for 5 seconds...");
                            std::thread::sleep(Duration::from_secs(5));
                        }
                    };
                    Ok::<_, Error>(())
                }).unwrap();
            }
            if insert_queue.len() >= INSERT_QUEUE_SIZE { // In case db insertion failed
                sleep(Duration::from_secs(1)).await;
            } else {
                let transaction_option = db_transactions_queue.pop();
                if transaction_option.is_some() {
                    let transaction = transaction_option.unwrap();
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
                    sleep(Duration::from_secs(1)).await;
                }
            }
        }
    }
}
