extern crate diesel;

use chrono::DateTime;
use std::cmp::min;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crossbeam_queue::ArrayQueue;
use itertools::Itertools;
use log::{error, info};
use sqlx::{Acquire, Executor, Pool, Postgres, Row};
use tokio::time::sleep;

use crate::database::models::{Block, VAR_KEY_BLOCK_CHECKPOINT};

struct Checkpoint {
    block_hash: Vec<u8>,
    tx_count: i64,
}

pub async fn insert_blocks(
    run: Arc<AtomicBool>,
    batch_scale: f64,
    start_vcp: Arc<AtomicBool>,
    db_blocks_queue: Arc<ArrayQueue<(Block, Vec<Vec<u8>>)>>,
    db_pool: Pool<Postgres>,
) {
    const NOOP_DELETES_BEFORE_VCP: i32 = 10;
    const CHECKPOINT_SAVE_INTERVAL: u64 = 60;
    const CHECKPOINT_WARN_AFTER: u64 = 5 * CHECKPOINT_SAVE_INTERVAL;
    let batch_size = min((500f64 * batch_scale) as usize, 3500); // 2^16 / fields in Blocks
    let mut vcp_started = false;
    let mut insert_queue = vec![];
    let mut checkpoint = None;
    let mut last_block_datetime;
    let mut checkpoint_last_saved = Instant::now();
    let mut last_commit_time = Instant::now();
    let mut noop_delete_count = 0;

    while run.load(Ordering::Relaxed) {
        if let Some((block, transactions)) = db_blocks_queue.pop() {
            if Instant::now().duration_since(checkpoint_last_saved).as_secs() > CHECKPOINT_SAVE_INTERVAL {
                if let None = checkpoint {
                    // Select the current block as checkpoint if none is set
                    checkpoint = Some(Checkpoint { block_hash: block.hash.clone(), tx_count: transactions.len() as i64 })
                }
            }
            last_block_datetime = DateTime::from_timestamp_millis(block.timestamp.unwrap() / 1000 * 1000).unwrap();
            insert_queue.push(block);
        } else {
            sleep(Duration::from_millis(100)).await;
            continue;
        }
        if insert_queue.len() >= batch_size
            || (insert_queue.len() >= 1 && Instant::now().duration_since(last_commit_time).as_secs() > 2)
        {
            let mut con = db_pool.acquire().await.expect("Database connection FAILED");
            let mut tx = con.begin().await.expect("Transaction begin FAILED");

            let sql = format!(
                "INSERT INTO blocks (
                    hash,
                    accepted_id_merkle_root,
                    difficulty,
                    merge_set_blues_hashes,
                    merge_set_reds_hashes,
                    selected_parent_hash,
                    bits,
                    blue_score,
                    blue_work,
                    daa_score,
                    hash_merkle_root,
                    nonce,
                    parents,
                    pruning_point,
                    timestamp,
                    utxo_commitment, 
                    version
                ) VALUES {} ON CONFLICT DO NOTHING",
                (0..insert_queue.len()).map(|i| format!("({})", (1..=17).map(|c| format!("${}", c + i * 17)).join(", "))).join(", ")
            );

            let mut query = sqlx::query(&sql);
            let mut block_hashes = vec![];
            for block in insert_queue {
                if !vcp_started {
                    block_hashes.push(block.hash.clone());
                }
                query = query.bind(block.hash);
                query = query.bind(block.accepted_id_merkle_root);
                query = query.bind(block.difficulty);
                query = query.bind(block.merge_set_blues_hashes);
                query = query.bind(block.merge_set_reds_hashes);
                query = query.bind(block.selected_parent_hash);
                query = query.bind(block.bits);
                query = query.bind(block.blue_score);
                query = query.bind(block.blue_work);
                query = query.bind(block.daa_score);
                query = query.bind(block.hash_merkle_root);
                query = query.bind(block.nonce);
                query = query.bind(block.parents);
                query = query.bind(block.pruning_point);
                query = query.bind(block.timestamp);
                query = query.bind(block.utxo_commitment);
                query = query.bind(block.version);
            }
            let blocks_inserted = tx.execute(query).await.expect("Insert blocks FAILED").rows_affected();

            if !vcp_started {
                checkpoint = None; // Clear the checkpoint block until vcp has been started
                let query = sqlx::query("DELETE FROM chain_blocks WHERE block_hash = ANY($1)").bind(&block_hashes);
                let cbs_deleted = tx.execute(query).await.expect("Delete chain_blocks FAILED").rows_affected();
                let query = sqlx::query("DELETE FROM transactions_acceptances WHERE block_hash = ANY($1)").bind(&block_hashes);
                let tas_deleted = tx.execute(query).await.expect("Delete transactions_acceptances FAILED").rows_affected();
                if blocks_inserted > 0 && tas_deleted == 0 && cbs_deleted == 0 {
                    noop_delete_count += 1;
                } else {
                    noop_delete_count = 0;
                }
                if noop_delete_count >= NOOP_DELETES_BEFORE_VCP {
                    info!("Notifying virtual chain processor");
                    start_vcp.store(true, Ordering::Relaxed);
                    vcp_started = true;
                    checkpoint_last_saved = Instant::now(); // Give VCP time to catch up
                }
                info!(
                    "Committed {} new blocks, cleared {} cb and {} ta. Last block: {}",
                    blocks_inserted, cbs_deleted, tas_deleted, last_block_datetime
                );
            } else {
                info!("Committed {} new blocks. Last block: {}", blocks_inserted, last_block_datetime);
                if let Some(c) = checkpoint {
                    // Check if the checkpoint candidate's transactions are present
                    let query = sqlx::query("SELECT COUNT(*) FROM blocks_transactions WHERE block_hash = $1").bind(&c.block_hash);
                    let count: i64 = tx.fetch_one(query).await.expect("Get transactions for block FAILED").get(0);
                    if count == c.tx_count {
                        // Next, let's check if the VCP has proccessed it
                        let query = sqlx::query("SELECT COUNT(*) FROM chain_blocks WHERE block_hash = $1").bind(&c.block_hash);
                        let count: i64 = tx.fetch_one(query).await.expect("Get chain_blocks for block FAILED").get(0);
                        if count == 1 {
                            // All set, the checkpoint block has all transactions present and are marked as a chain block by the VCP
                            let checkpoint_string = hex::encode(c.block_hash);
                            info!("Saving block_checkpoint {}", checkpoint_string);
                            let query = sqlx::query(
                                "INSERT INTO vars (key, value) VALUES ($1, $2)
                                ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value",
                            )
                            .bind(VAR_KEY_BLOCK_CHECKPOINT)
                            .bind(checkpoint_string);
                            tx.execute(query).await.expect(format!("Commit {} FAILED", VAR_KEY_BLOCK_CHECKPOINT).as_str());
                            checkpoint_last_saved = Instant::now();
                        }
                        // Clear the checkpoint candidate either way
                        checkpoint = None;
                    } else if count > c.tx_count {
                        panic!("Expected {}, but found {} transactions on block {}!", &c.tx_count, count, hex::encode(&c.block_hash))
                    } else if Instant::now().duration_since(checkpoint_last_saved).as_secs() > CHECKPOINT_WARN_AFTER {
                        error!(
                            "Still unable to save block_checkpoint {}. Expected {} txs, committed {}",
                            hex::encode(&c.block_hash),
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
            tx.commit().await.expect("Commit blocks FAILED");
            last_commit_time = Instant::now();
            insert_queue = vec![];
        }
    }
}
