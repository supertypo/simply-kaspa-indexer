extern crate diesel;

use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use crossbeam_queue::ArrayQueue;
use diesel::{Connection, ExpressionMethods, insert_into, RunQueryDsl};
use diesel::pg::PgConnection;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::result::Error;
use diesel::upsert::excluded;
use log::info;
use tokio::time::sleep;

use crate::database::models::Block;
use crate::database::schema::blocks;

pub async fn insert_blocks(db_blocks_queue: Arc<ArrayQueue<Block>>, db_pool: Pool<ConnectionManager<PgConnection>>) -> Result<(), ()> {
    const INSERT_QUEUE_SIZE: usize = 1800;
    loop {
        info!("Insert blocks started");
        let mut insert_queue: HashSet<Block> = HashSet::with_capacity(INSERT_QUEUE_SIZE);
        let mut last_block_timestamp = 0;
        let mut last_commit_time = SystemTime::now();
        let mut rows_affected = 0;
        loop {
            if insert_queue.len() >= INSERT_QUEUE_SIZE || (insert_queue.len() >= 1 && SystemTime::now().duration_since(last_commit_time).unwrap().as_secs() > 2) {
                let con = &mut db_pool.get().expect("Database connection FAILED");
                con.transaction(|con| {
                    rows_affected = insert_into(blocks::dsl::blocks)
                        .values(Vec::from_iter(&insert_queue))
                        .on_conflict(blocks::hash)
                        .do_update()
                        .set((
                            blocks::hash.eq(excluded(blocks::hash)),
                            blocks::accepted_id_merkle_root.eq(excluded(blocks::accepted_id_merkle_root)),
                            blocks::difficulty.eq(excluded(blocks::difficulty)),
                            blocks::is_chain_block.eq(excluded(blocks::is_chain_block)),
                            blocks::merge_set_blues_hashes.eq(excluded(blocks::merge_set_blues_hashes)),
                            blocks::merge_set_reds_hashes.eq(excluded(blocks::merge_set_reds_hashes)),
                            blocks::selected_parent_hash.eq(excluded(blocks::selected_parent_hash)),
                            blocks::bits.eq(excluded(blocks::bits)),
                            blocks::blue_score.eq(excluded(blocks::blue_score)),
                            blocks::blue_work.eq(excluded(blocks::blue_work)),
                            blocks::daa_score.eq(excluded(blocks::daa_score)),
                            blocks::hash_merkle_root.eq(excluded(blocks::hash_merkle_root)),
                            blocks::nonce.eq(excluded(blocks::nonce)),
                            blocks::parents.eq(excluded(blocks::parents)),
                            blocks::pruning_point.eq(excluded(blocks::pruning_point)),
                            blocks::timestamp.eq(excluded(blocks::timestamp)),
                            blocks::utxo_commitment.eq(excluded(blocks::utxo_commitment)),
                            blocks::version.eq(excluded(blocks::version))))
                        .execute(con)
                        .expect("Commit blocks to database FAILED");
                    Ok::<_, Error>(())
                }).expect("Commit blocks to database FAILED");
                info!("Committed {} blocks to database. Last timestamp: {}", rows_affected, 
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
}
