extern crate diesel;

use std::cmp::min;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crossbeam_queue::ArrayQueue;
use diesel::dsl::exists;
use diesel::pg::PgConnection;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::result::Error;
use diesel::{delete, insert_into, select, Connection, ExpressionMethods, QueryDsl, RunQueryDsl};
use log::{error, info};
use tokio::time::sleep;

use crate::database::models::Block;
use crate::database::schema::blocks_transactions;
use crate::database::schema::{blocks, chain_blocks, transactions_acceptances};
use crate::vars::vars::save_block_checkpoint;

pub async fn insert_blocks(
    run: Arc<AtomicBool>,
    batch_scale: f64,
    start_vcp: Arc<AtomicBool>,
    db_blocks_queue: Arc<ArrayQueue<(Block, Vec<Vec<u8>>)>>,
    db_pool: Pool<ConnectionManager<PgConnection>>,
) {
    const NOOP_DELETES_BEFORE_VCP: i32 = 10;
    const CHECKPOINT_SAVE_INTERVAL: u64 = 60;
    const CHECKPOINT_WARN_AFTER: u64 = 5 * CHECKPOINT_SAVE_INTERVAL;
    let batch_size = min((500f64 * batch_scale) as usize, 3500); // 2^16 / fields in Blocks
    let mut vcp_started = false;
    let mut insert_queue = vec![];
    let mut last_block_hash = vec![];
    let mut last_block_tx_count = 0;
    let mut last_block_timestamp = 0;
    let mut checkpoint_hash = vec![];
    let mut checkpoint_hash_tx_expected_count = 0;
    let mut checkpoint_last_saved = Instant::now();
    let mut last_commit_time = Instant::now();
    let mut noop_delete_count = 0;

    while run.load(Ordering::Relaxed) {
        if let Some((block, transactions)) = db_blocks_queue.pop() {
            last_block_hash = block.hash.clone();
            last_block_tx_count = transactions.len() as i64;
            last_block_timestamp = block.timestamp.unwrap();
            insert_queue.push(block);
        } else {
            sleep(Duration::from_millis(100)).await;
        }
        if insert_queue.len() >= batch_size
            || (insert_queue.len() >= 1 && Instant::now().duration_since(last_commit_time).as_secs() > 2)
        {
            let mut cb_cleared = 0;
            let mut ta_cleared = 0;
            let mut blocks_inserted = 0;
            let con = &mut db_pool.get().expect("Database connection FAILED");
            con.transaction(|con| {
                blocks_inserted = insert_into(blocks::dsl::blocks)
                    .values(&insert_queue)
                    .on_conflict_do_nothing()
                    .execute(con)
                    .expect("Commit blocks FAILED");

                if !vcp_started {
                    let delete_block_hashes: Vec<Vec<u8>> = insert_queue.iter().map(|b| b.hash.clone()).collect();
                    ta_cleared = delete(transactions_acceptances::dsl::transactions_acceptances)
                        .filter(transactions_acceptances::block_hash.eq_any(&delete_block_hashes))
                        .execute(con)
                        .expect("Commit rejected transactions FAILED");
                    cb_cleared = delete(chain_blocks::dsl::chain_blocks)
                        .filter(chain_blocks::block_hash.eq_any(&delete_block_hashes))
                        .execute(con)
                        .expect("Commit removed chain blocks FAILED");
                    if blocks_inserted > 0 && cb_cleared == 0 && ta_cleared == 0 {
                        noop_delete_count += 1;
                    } else {
                        noop_delete_count = 0;
                    }
                }

                if !vcp_started && noop_delete_count >= NOOP_DELETES_BEFORE_VCP {
                    info!("End of previous run reached, notifying virtual chain processor");
                    start_vcp.store(true, Ordering::Relaxed);
                    vcp_started = true;
                    checkpoint_last_saved = Instant::now(); // Give VCP time to catch up
                }
                Ok::<_, Error>(())
            })
            .expect("Commit blocks FAILED");
            last_commit_time = Instant::now();

            if !vcp_started {
                info!(
                    "Committed {} new blocks, cleared {} cb and {} ta. Last block: {}",
                    blocks_inserted,
                    cb_cleared,
                    ta_cleared,
                    chrono::DateTime::from_timestamp_millis(last_block_timestamp / 1000 * 1000).unwrap()
                );
            } else {
                info!(
                    "Committed {} new blocks. Last block: {}",
                    blocks_inserted,
                    chrono::DateTime::from_timestamp_millis(last_block_timestamp / 1000 * 1000).unwrap()
                );
            }

            if vcp_started && Instant::now().duration_since(checkpoint_last_saved).as_secs() > CHECKPOINT_SAVE_INTERVAL {
                if checkpoint_hash.is_empty() {
                    // Pick the last block as a checkpoint candidate
                    checkpoint_hash = last_block_hash.clone();
                    checkpoint_hash_tx_expected_count = last_block_tx_count;
                } else {
                    // Check if the checkpoint candidate's transactions are present
                    let checkpoint_hash_tx_committed_count = blocks_transactions::dsl::blocks_transactions
                        .filter(blocks_transactions::block_hash.eq(&checkpoint_hash))
                        .count()
                        .get_result::<i64>(con)
                        .unwrap();
                    if checkpoint_hash_tx_committed_count == checkpoint_hash_tx_expected_count {
                        // Next, let's check if the VCP has proccessed it
                        if select(exists(chain_blocks::dsl::chain_blocks.filter(chain_blocks::block_hash.eq(&checkpoint_hash))))
                            .get_result::<bool>(con)
                            .unwrap()
                        {
                            // All set, the checkpoint block has all transactions present and are marked as a chain block by the VCP
                            let checkpoint = hex::encode(checkpoint_hash);
                            info!("Saving block_checkpoint {}", checkpoint);
                            save_block_checkpoint(checkpoint, db_pool.clone());
                            checkpoint_last_saved = Instant::now();
                        }
                        checkpoint_hash = vec![]; // Clear the candidate either way
                    } else if checkpoint_hash_tx_committed_count > checkpoint_hash_tx_expected_count {
                        panic!(
                            "Expected {}, but found {} transactions on block {}!",
                            checkpoint_hash_tx_expected_count,
                            checkpoint_hash_tx_committed_count,
                            hex::encode(&checkpoint_hash)
                        )
                    } else if Instant::now().duration_since(checkpoint_last_saved).as_secs() > CHECKPOINT_WARN_AFTER {
                        error!(
                            "Still unable to save block_checkpoint {}. Expected {} txs, committed {}",
                            hex::encode(&checkpoint_hash),
                            checkpoint_hash_tx_expected_count,
                            checkpoint_hash_tx_committed_count
                        )
                    }
                }
            }
            insert_queue = vec![];
        }
    }
}
