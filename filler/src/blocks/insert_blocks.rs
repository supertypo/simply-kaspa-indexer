use std::cmp::min;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::DateTime;
use crossbeam_queue::ArrayQueue;
use log::{debug, error, info};
use tokio::time::sleep;
use kaspa_database::client::client::KaspaDbClient;
use kaspa_database::models::block::Block;
use kaspa_database::models::sql_hash::SqlHash;

use crate::vars::vars::save_checkpoint;

struct Checkpoint {
    block_hash: SqlHash,
    tx_count: i64,
}

pub async fn insert_blocks(
    run: Arc<AtomicBool>,
    batch_scale: f64,
    start_vcp: Arc<AtomicBool>,
    db_blocks_queue: Arc<ArrayQueue<(Block, Vec<SqlHash>, bool)>>,
    database: KaspaDbClient,
) {
    const NOOP_DELETES_BEFORE_VCP: i32 = 10;
    const CHECKPOINT_SAVE_INTERVAL: u64 = 60;
    const CHECKPOINT_WARN_AFTER: u64 = 5 * CHECKPOINT_SAVE_INTERVAL;
    let batch_size = min((500f64 * batch_scale) as usize, 3500); // 2^16 / fields in Blocks
    let mut vcp_started = false;
    let mut blocks = vec![];
    let mut block_hashes = vec![];
    let mut checkpoint = None;
    let mut last_block_datetime;
    let mut checkpoint_last_saved = Instant::now();
    let mut last_commit_time = Instant::now();
    let mut noop_delete_count = 0;

    while run.load(Ordering::Relaxed) {
        if let Some((block, transactions, synced)) = db_blocks_queue.pop() {
            if Instant::now().duration_since(checkpoint_last_saved).as_secs() > CHECKPOINT_SAVE_INTERVAL {
                if let None = checkpoint {
                    // Select the current block as checkpoint if none is set
                    checkpoint = Some(Checkpoint { block_hash: block.hash.clone(), tx_count: transactions.len() as i64 })
                }
            }
            last_block_datetime = DateTime::from_timestamp_millis(block.timestamp / 1000 * 1000).unwrap();
            if !vcp_started {
                block_hashes.push(block.hash.clone());
            }
            blocks.push(block);

            if blocks.len() >= batch_size || (blocks.len() >= 1 && Instant::now().duration_since(last_commit_time).as_secs() > 2) {
                debug!("Processing {} blocks", blocks.len());
                let start_commit_time = Instant::now();
                let blocks_len = blocks.len();
                let blocks_inserted = database.insert_blocks(&blocks).await.expect("Insert blocks FAILED");
                let commit_time = Instant::now().duration_since(start_commit_time).as_millis();
                let bps = blocks_len as f64 / commit_time as f64 * 1000f64;
                blocks = vec![];

                if !vcp_started {
                    checkpoint = None; // Clear the checkpoint block until vcp has been started
                    let cbs_deleted = database.delete_chain_blocks(&block_hashes).await.expect("Delete chain_blocks FAILED");
                    let tas_deleted =
                        database.delete_transaction_acceptances(&block_hashes).await.expect("Delete transactions_acceptances FAILED");
                    block_hashes = vec![];
                    if synced && blocks_inserted > 0 && tas_deleted == 0 && cbs_deleted == 0 {
                        noop_delete_count += 1;
                    } else {
                        noop_delete_count = 0;
                    }
                    if noop_delete_count >= NOOP_DELETES_BEFORE_VCP {
                        info!("Notifying virtual chain processor");
                        start_vcp.store(true, Ordering::Relaxed);
                        vcp_started = true;
                        checkpoint_last_saved = Instant::now(); // Give VCP time to catch up before complaining
                    }
                    info!(
                        "Committed {} new blocks, cleared {} cb and {} ta in {}ms ({:.1} bps). Last block: {}",
                        blocks_inserted, cbs_deleted, tas_deleted, commit_time, bps, last_block_datetime
                    );
                } else {
                    info!(
                        "Committed {} new blocks in {}ms ({:.1} bps). Last block: {}",
                        blocks_inserted, commit_time, bps, last_block_datetime
                    );
                    if let Some(c) = checkpoint {
                        // Check if the checkpoint candidate's transactions are present
                        let count: i64 = database.select_tx_count(&c.block_hash).await.expect("Get tx count FAILED");
                        if count == c.tx_count {
                            // Next, let's check if the VCP has proccessed it
                            let is_chain_block = database.select_is_chain_block(&c.block_hash).await.expect("Get is cb FAILED");
                            if is_chain_block {
                                // All set, the checkpoint block has all transactions present and are marked as a chain block by the VCP
                                let checkpoint_string = hex::encode(c.block_hash.as_bytes());
                                info!("Saving block_checkpoint {}", checkpoint_string);
                                save_checkpoint(&checkpoint_string, &database).await.expect("Checkpoint saving FAILED");
                                checkpoint_last_saved = Instant::now();
                            }
                            // Clear the checkpoint candidate either way
                            checkpoint = None;
                        } else if count > c.tx_count {
                            panic!(
                                "Expected {}, but found {} transactions on block {}!",
                                &c.tx_count,
                                count,
                                hex::encode(c.block_hash.as_bytes())
                            )
                        } else if Instant::now().duration_since(checkpoint_last_saved).as_secs() > CHECKPOINT_WARN_AFTER {
                            error!(
                                "Still unable to save block_checkpoint {}. Expected {} txs, committed {}",
                                hex::encode(c.block_hash.as_bytes()),
                                &c.tx_count,
                                count
                            );
                            checkpoint = Some(c);
                        } else {
                            // Let's wait one round and check again
                            checkpoint = Some(c);
                        }
                    }
                }
                last_commit_time = Instant::now();
            }
        } else {
            sleep(Duration::from_millis(100)).await;
        }
    }
}
