extern crate diesel;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use crossbeam_queue::ArrayQueue;
use diesel::{BoolExpressionMethods, Connection, ExpressionMethods, insert_into, QueryDsl, RunQueryDsl};
use diesel::pg::PgConnection;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::result::Error;
use log::{debug, info};
use tokio::time::sleep;

use crate::database::models::{Block, BlockTransaction};
use crate::database::schema::blocks;
use crate::database::schema::blocks_transactions;
use crate::vars::vars::save_block_start_hash;

pub async fn insert_blocks(db_blocks_queue: Arc<ArrayQueue<(Block, Vec<BlockTransaction>)>>,
                           db_pool: Pool<ConnectionManager<PgConnection>>) -> Result<(), ()> {
    const INSERT_QUEUE_SIZE: usize = 1800;
    info!("Insert blocks started");
    let mut insert_queue: HashMap<Block, Vec<BlockTransaction>> = HashMap::with_capacity(INSERT_QUEUE_SIZE);
    let mut last_block_timestamp = 0;
    let mut last_commit_time = SystemTime::now();
    let mut rows_affected = 0;
    let mut start_hash_last_saved = SystemTime::now();
    loop {
        if insert_queue.len() >= INSERT_QUEUE_SIZE || (insert_queue.len() >= 1 && SystemTime::now().duration_since(last_commit_time).unwrap().as_secs() > 2) {
            let con = &mut db_pool.get().expect("Database connection FAILED");
            con.transaction(|con| {
                // Find existing blocks and remove them from the insert queue
                blocks::dsl::blocks
                    .filter(blocks::hash.eq_any(insert_queue.keys().map(|b| b.hash.clone()).collect::<Vec<Vec<u8>>>()))
                    .load::<Block>(con)
                    .unwrap().iter()
                    .for_each(|b| { insert_queue.remove(b).unwrap(); });

                rows_affected = insert_into(blocks::dsl::blocks)
                    .values(Vec::from_iter(insert_queue.keys()))
                    .execute(con)
                    .expect("Commit blocks to database FAILED");

                Ok::<_, Error>(())
            }).expect("Commit blocks to database FAILED");

            info!("Committed {} new blocks to database. Last timestamp: {}", rows_affected,
                chrono::DateTime::from_timestamp_millis(last_block_timestamp as i64 * 1000).unwrap());
            last_commit_time = SystemTime::now();
            
            if insert_queue.len() >= 1 && SystemTime::now().duration_since(start_hash_last_saved).unwrap().as_secs() > 60 {
                // Introduce a small delay to increase the chance for success
                sleep(Duration::from_millis(300)).await;
                // Pick a random block and save it as start_hash if all its transactions are present
                let block_tuple = insert_queue.iter().next().unwrap();
                let block = block_tuple.0;
                let block_transactions = block_tuple.1;
                let commited_transaction_count = blocks_transactions::dsl::blocks_transactions
                    .filter(blocks_transactions::block_hash.eq(&block.hash)
                        .and(blocks_transactions::transaction_id.eq_any(&block_transactions.iter()
                            .map(|bt| bt.transaction_id.clone()).collect::<Vec<Vec<u8>>>())))
                    .count()
                    .get_result::<i64>(con).unwrap();
                if block_transactions.len() as i64 == commited_transaction_count {
                    save_block_start_hash(hex::encode(block.hash.clone()), db_pool.clone());
                    start_hash_last_saved = SystemTime::now();
                } else {
                    debug!("Unable to save start point as some transactions was missing")
                }
            }
            insert_queue = HashMap::with_capacity(INSERT_QUEUE_SIZE);
        }
        let block_option = db_blocks_queue.pop();
        if block_option.is_some() {
            let block_tuple = block_option.unwrap();
            let block = block_tuple.0;
            last_block_timestamp = block.timestamp;
            insert_queue.insert(block, block_tuple.1);
        } else {
            sleep(Duration::from_millis(100)).await;
        }
    }
}
