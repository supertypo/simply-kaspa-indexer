extern crate diesel;

use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use crossbeam_queue::ArrayQueue;
use diesel::{Connection, ExpressionMethods, insert_into, QueryDsl, RunQueryDsl};
use diesel::pg::PgConnection;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::result::Error;
use log::{error, info};
use tokio::time::sleep;

use crate::database::models::Block;
use crate::database::schema::blocks;
use crate::database::schema::blocks_transactions;
use crate::vars::vars::save_block_start_hash;

pub async fn insert_blocks(db_blocks_queue: Arc<ArrayQueue<(Block, Vec<Vec<u8>>)>>,
                           db_pool: Pool<ConnectionManager<PgConnection>>) -> Result<(), ()> {
    const INSERT_QUEUE_SIZE: usize = 1800;
    info!("Insert blocks started");
    let mut insert_queue: HashSet<Block> = HashSet::with_capacity(INSERT_QUEUE_SIZE);
    let mut rows_affected = 0;
    let mut last_block_hash = vec![];
    let mut last_block_tx_count = 0;
    let mut last_block_timestamp = 0;
    let mut start_hash = vec![];
    let mut start_hash_tx_count = 0;
    let mut start_hash_last_saved = SystemTime::now();
    let mut last_commit_time = SystemTime::now();
    loop {
        if insert_queue.len() >= INSERT_QUEUE_SIZE || (insert_queue.len() >= 1 && SystemTime::now().duration_since(last_commit_time).unwrap().as_secs() > 2) {
            let con = &mut db_pool.get().expect("Database connection FAILED");
            con.transaction(|con| {
                // Find existing blocks and remove them from the insert queue
                blocks::dsl::blocks
                    .filter(blocks::hash.eq_any(insert_queue.iter().map(|b| b.hash.clone()).collect::<Vec<Vec<u8>>>()))
                    .load::<Block>(con)
                    .unwrap().iter()
                    .for_each(|b| { insert_queue.remove(b); });

                rows_affected = insert_into(blocks::dsl::blocks)
                    .values(Vec::from_iter(insert_queue.iter()))
                    .execute(con)
                    .expect("Commit blocks to database FAILED");

                Ok::<_, Error>(())
            }).expect("Commit blocks to database FAILED");

            info!("Committed {} new blocks to database. Last timestamp: {}", rows_affected,
                chrono::DateTime::from_timestamp_millis(last_block_timestamp as i64 * 1000).unwrap());
            last_commit_time = SystemTime::now();

            if insert_queue.len() >= 1 && SystemTime::now().duration_since(start_hash_last_saved).unwrap().as_secs() > 60 {
                if start_hash.is_empty() {
                    start_hash = last_block_hash.clone(); // Choose the last block as start_hash
                    start_hash_tx_count = last_block_tx_count;
                } else { // Save the previously picked block as start_hash if all its transactions are present
                    let start_hash_tx_commited_count = blocks_transactions::dsl::blocks_transactions
                        .filter(blocks_transactions::block_hash.eq(&start_hash))
                        .count()
                        .get_result::<i64>(con).unwrap();
                    if start_hash_tx_count as i64 == start_hash_tx_commited_count {
                        save_block_start_hash(hex::encode(start_hash.clone()), db_pool.clone());
                        start_hash = vec![];
                        start_hash_tx_count = 0;
                        start_hash_last_saved = SystemTime::now();
                    } else if start_hash_tx_count as i64 > start_hash_tx_commited_count {
                        panic!("Expected {}, but found {} transactions on block {}!", start_hash_tx_count, start_hash_tx_commited_count, hex::encode(&start_hash))
                    } else if SystemTime::now().duration_since(start_hash_last_saved).unwrap().as_secs() > 600 {
                        error!("Still unable to save start point due to missing transactions")
                    }
                }
            }
            insert_queue = HashSet::with_capacity(INSERT_QUEUE_SIZE);
        }
        let block_tuple = db_blocks_queue.pop();
        if block_tuple.is_some() {
            let block_tuple = block_tuple.unwrap();
            let block = block_tuple.0;
            let transactions = block_tuple.1;
            last_block_hash = block.hash.clone();
            last_block_tx_count = transactions.len();
            last_block_timestamp = block.timestamp.clone();
            insert_queue.insert(block);
        } else {
            sleep(Duration::from_millis(100)).await;
        }
    }
}
