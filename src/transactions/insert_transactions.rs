extern crate diesel;

use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use crossbeam_queue::ArrayQueue;
use diesel::{BoolExpressionMethods, Connection, ExpressionMethods, insert_into, QueryDsl, RunQueryDsl};
use diesel::pg::PgConnection;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::result::Error;
use diesel::upsert::{excluded, on_constraint};
use log::info;
use tokio::time::sleep;

use crate::database::models::{BlockTransaction, Transaction};
use crate::database::schema::blocks_transactions;
use crate::database::schema::transactions;

pub async fn insert_transactions(db_transactions_queue: Arc<ArrayQueue<(Transaction, BlockTransaction)>>,
                                 db_pool: Pool<ConnectionManager<PgConnection>>) -> Result<(), ()> {
    const INSERT_QUEUE_SIZE: usize = 7500;
    info!("Insert transactions started");
    let mut insert_tx_queue: HashSet<Transaction> = HashSet::with_capacity(INSERT_QUEUE_SIZE);
    let mut insert_block_tx_queue: HashSet<BlockTransaction> = HashSet::with_capacity(INSERT_QUEUE_SIZE);
    let mut last_block_timestamp = 0;
    let mut last_commit_time = SystemTime::now();
    let mut rows_affected_tx = 0;
    let mut rows_affected_block_tx = 0;
    loop {
        if insert_block_tx_queue.len() >= INSERT_QUEUE_SIZE || (insert_block_tx_queue.len() >= 1 && SystemTime::now().duration_since(last_commit_time).unwrap().as_secs() > 2) {
            let con = &mut db_pool.get().expect("Database connection FAILED");
            con.transaction(|con| {
                // Find existing identical transactions and remove them from the insert queue
                transactions::dsl::transactions
                    .filter(transactions::transaction_id.eq_any(insert_tx_queue.iter()
                        .map(|t| t.transaction_id.clone()).collect::<Vec<Vec<u8>>>()))
                    .load::<Transaction>(con)
                    .unwrap().iter()
                    .for_each(|t| {
                        let new_tx = insert_tx_queue.get(t).unwrap().clone();
                        if new_tx.subnetwork == t.subnetwork &&
                            new_tx.hash == t.hash &&
                            new_tx.mass == t.mass &&
                            new_tx.block_time == t.block_time {
                            insert_tx_queue.remove(t);
                        }
                    });
                // Find existing identical block to transaction mappings and remove them from the insert queue
                blocks_transactions::dsl::blocks_transactions
                    .filter(blocks_transactions::block_hash.eq_any(insert_block_tx_queue.iter().map(|bt| bt.block_hash.clone()).collect::<Vec<Vec<u8>>>())
                        .and(blocks_transactions::transaction_id.eq_any(insert_block_tx_queue.iter().map(|bt| bt.transaction_id.clone()).collect::<Vec<Vec<u8>>>())))
                    .load::<BlockTransaction>(con)
                    .unwrap().iter()
                    .for_each(|bt| { insert_block_tx_queue.remove(bt); });

                //Upsert transactions in case a conflicting tx was persisted
                rows_affected_tx = insert_into(transactions::dsl::transactions)
                    .values(Vec::from_iter(&insert_tx_queue))
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

                //Ignore conflicts as any conflicting rows will be identical
                rows_affected_block_tx = insert_into(blocks_transactions::dsl::blocks_transactions)
                    .values(Vec::from_iter(&insert_block_tx_queue))
                    .on_conflict(on_constraint("pk_blocks_transactions"))
                    .do_nothing()
                    .execute(con)
                    .expect("Commit transactions to database FAILED");

                Ok::<_, Error>(())
            }).expect("Commit transactions to database FAILED");
            info!("Committed {} transactions to database. Last block timestamp: {}", rows_affected_tx,
                chrono::DateTime::from_timestamp_millis(last_block_timestamp as i64 * 1000).unwrap());
            info!("Committed {} block/transaction mappings to database.", rows_affected_tx);
            insert_tx_queue = HashSet::with_capacity(INSERT_QUEUE_SIZE);
            insert_block_tx_queue = HashSet::with_capacity(INSERT_QUEUE_SIZE);
            last_commit_time = SystemTime::now();
        }
        let transaction_option = db_transactions_queue.pop();
        if transaction_option.is_some() {
            let transaction_tuple = transaction_option.unwrap();
            let transaction = transaction_tuple.0;
            last_block_timestamp = transaction.block_time.unwrap();
            insert_tx_queue.insert(transaction);
            insert_block_tx_queue.insert(transaction_tuple.1);
        } else {
            sleep(Duration::from_millis(100)).await;
        }
    }
}
