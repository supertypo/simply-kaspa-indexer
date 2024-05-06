extern crate diesel;

use std::cmp::min;
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime};

use crossbeam_queue::ArrayQueue;
use diesel::{Connection, delete, ExpressionMethods, insert_into, QueryDsl, RunQueryDsl};
use diesel::pg::PgConnection;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::result::Error;
use log::{error, info};
use tokio::time::sleep;

use crate::database::models::Block;
use crate::database::schema::{blocks, chain_blocks, transactions_acceptances};
use crate::database::schema::blocks_transactions;
use crate::vars::vars::save_block_checkpoint;

pub async fn insert_blocks(running: Arc<AtomicBool>,
                           buffer_size: f64,
                           start_vcp: Arc<AtomicBool>,
                           db_blocks_queue: Arc<ArrayQueue<(bool, Block, Vec<Vec<u8>>)>>,
                           db_pool: Pool<ConnectionManager<PgConnection>>) {
    const CHECKPOINT_SAVE_INTERVAL: u64 = 60;
    let max_queue_size = min((1000f64 * buffer_size) as usize, 3500); // ~3500 is the max batch size db supports
    let mut vcp_notified = false;
    let mut delete_chain_blocks_queue: Vec<Vec<u8>> = vec![];
    let mut insert_queue: HashSet<Block> = HashSet::with_capacity(max_queue_size);
    let mut last_block_hash = vec![];
    let mut last_block_tx_count = 0;
    let mut last_block_timestamp = 0;
    let mut checkpoint_hash = vec![];
    let mut checkpoint_hash_tx_expected_count = 0;
    let mut checkpoint_last_saved = SystemTime::now();
    let mut last_commit_time = SystemTime::now();

    while running.load(Ordering::Relaxed) {
        if insert_queue.len() >= max_queue_size || (insert_queue.len() >= 1 && SystemTime::now().duration_since(last_commit_time).unwrap().as_secs() > 2) {
            let mut cbs_cleared = 0;
            let mut tas_cleared = 0;
            let mut blocks_inserted = 0;
            let con = &mut db_pool.get().expect("Database connection FAILED");
            con.transaction(|con| {
                if !delete_chain_blocks_queue.is_empty() {
                    cbs_cleared = delete(chain_blocks::dsl::chain_blocks)
                        .filter(chain_blocks::block_hash.eq_any(&delete_chain_blocks_queue))
                        .execute(con)
                        .expect("Commit removed chain blocks FAILED");
                    tas_cleared = delete(transactions_acceptances::dsl::transactions_acceptances)
                        .filter(transactions_acceptances::block_hash.eq_any(&delete_chain_blocks_queue))
                        .execute(con)
                        .expect("Commit rejected transactions FAILED");
                } else if !vcp_notified {
                    info!("\x1b[32mChain blocks and transaction acceptances cleared, notifying VCP\x1b[0m");
                    start_vcp.store(true, Ordering::Relaxed); // Let VCP commence
                    vcp_notified = true;
                }
                blocks_inserted = insert_into(blocks::dsl::blocks)
                    .values(Vec::from_iter(insert_queue.iter()))
                    .on_conflict_do_nothing()
                    .execute(con)
                    .expect("Commit blocks FAILED");
                Ok::<_, Error>(())
            }).expect("Commit blocks FAILED");

            if cbs_cleared != 0 {
                info!("Committed {} new blocks, cleared {} cb and {} ta. Last block timestamp: {}", blocks_inserted, cbs_cleared, tas_cleared,
                chrono::DateTime::from_timestamp_millis(last_block_timestamp / 1000 * 1000).unwrap());
            } else {
                info!("Committed {} new blocks. Last block timestamp: {}", blocks_inserted,
                chrono::DateTime::from_timestamp_millis(last_block_timestamp / 1000 * 1000).unwrap());
            }
            last_commit_time = SystemTime::now();

            if insert_queue.len() >= 1 && SystemTime::now().duration_since(checkpoint_last_saved).unwrap().as_secs() > CHECKPOINT_SAVE_INTERVAL {
                if checkpoint_hash.is_empty() {
                    checkpoint_hash = last_block_hash.clone();
                    checkpoint_hash_tx_expected_count = last_block_tx_count;
                } else { // Save the previously picked block hash as checkpoint if all its transactions are present
                    let checkpoint_hash_tx_committed_count = blocks_transactions::dsl::blocks_transactions
                        .filter(blocks_transactions::block_hash.eq(&checkpoint_hash))
                        .count()
                        .get_result::<i64>(con).unwrap();
                    if checkpoint_hash_tx_committed_count == checkpoint_hash_tx_expected_count {
                        let checkpoint = hex::encode(checkpoint_hash);
                        info!("Saving block_checkpoint {}", checkpoint);
                        save_block_checkpoint(checkpoint, db_pool.clone());
                        checkpoint_hash = vec![];
                        checkpoint_last_saved = SystemTime::now();
                    } else if checkpoint_hash_tx_committed_count > checkpoint_hash_tx_expected_count {
                        panic!("Expected {}, but found {} transactions on block {}!", checkpoint_hash_tx_expected_count, checkpoint_hash_tx_committed_count, hex::encode(&checkpoint_hash))
                    } else if SystemTime::now().duration_since(checkpoint_last_saved).unwrap().as_secs() > 300 {
                        error!("Still unable to save block_checkpoint {}. Expected {} txs, committed {}", hex::encode(&checkpoint_hash), checkpoint_hash_tx_expected_count, checkpoint_hash_tx_committed_count)
                    }
                }
            }
            delete_chain_blocks_queue = vec![];
            insert_queue = HashSet::with_capacity(max_queue_size);
        }
        let block_tuple = db_blocks_queue.pop();
        if block_tuple.is_some() {
            let block_tuple = block_tuple.unwrap();
            let synced = block_tuple.0;
            let block = block_tuple.1;
            let transactions = block_tuple.2;
            last_block_hash = block.hash.clone();
            last_block_tx_count = transactions.len() as i64;
            last_block_timestamp = block.timestamp.unwrap();
            if !synced {
                delete_chain_blocks_queue.push(block.hash.clone());
            }
            insert_queue.insert(block);
        } else {
            sleep(Duration::from_millis(100)).await;
        }
    }
}
