extern crate diesel;

use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use crossbeam_queue::ArrayQueue;
use diesel::{Connection, ExpressionMethods, insert_into, QueryDsl, RunQueryDsl};
use diesel::pg::PgConnection;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::result::Error;
use log::info;
use tokio::time::sleep;

use crate::database::models::Block;
use crate::database::schema::blocks;

pub async fn insert_blocks(db_blocks_queue: Arc<ArrayQueue<Block>>, db_pool: Pool<ConnectionManager<PgConnection>>) -> Result<(), ()> {
    const INSERT_QUEUE_SIZE: usize = 1800;
    info!("Insert blocks started");
    let mut insert_queue: HashSet<Block> = HashSet::with_capacity(INSERT_QUEUE_SIZE);
    let mut last_block_timestamp = 0;
    let mut last_commit_time = SystemTime::now();
    let mut rows_affected = 0;
    loop {
        if insert_queue.len() >= INSERT_QUEUE_SIZE || (insert_queue.len() >= 1 && SystemTime::now().duration_since(last_commit_time).unwrap().as_secs() > 2) {
            let con = &mut db_pool.get().expect("Database connection FAILED");
            con.transaction(|con| {
                // If multiple threads should write to the blocks table, a lock should be placed first
                let existing_blocks = blocks::dsl::blocks
                    .filter(blocks::hash.eq_any(insert_queue.iter().map(|b| b.hash.clone()).collect::<Vec<Vec<u8>>>()))
                    .load::<Block>(con).unwrap();
                for b in existing_blocks {
                    insert_queue.remove(&b);
                }
                rows_affected = insert_into(blocks::dsl::blocks)
                    .values(Vec::from_iter(&insert_queue))
                    .execute(con)
                    .expect("Commit blocks to database FAILED");
                Ok::<_, Error>(())
            }).expect("Commit blocks to database FAILED");
            info!("Committed {} new dblocks to database. Last timestamp: {}", rows_affected,
                chrono::DateTime::from_timestamp_millis(last_block_timestamp as i64 * 1000).unwrap());
            insert_queue = HashSet::with_capacity(INSERT_QUEUE_SIZE);
            last_commit_time = SystemTime::now();
        }
        let block_option = db_blocks_queue.pop();
        if block_option.is_some() {
            let block = block_option.unwrap();
            last_block_timestamp = block.timestamp;
            insert_queue.insert(block);
        } else {
            sleep(Duration::from_millis(100)).await;
        }
    }
}
