extern crate diesel;

use std::cmp::min;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::DateTime;
use crossbeam_queue::ArrayQueue;
use itertools::Itertools;
use log::{error, info};
use sqlx::{Executor, Pool, Postgres, Row};
use tokio::time::sleep;

use crate::database::copy_from::{format_binary, format_binary_array};
use crate::database::models::Block;
use crate::vars::vars::save_checkpoint;

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
    let mut blocks = vec![];
    let mut block_hashes = vec![];
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
            last_block_datetime = DateTime::from_timestamp_millis(block.timestamp / 1000 * 1000).unwrap();
            if !vcp_started {
                block_hashes.push(block.hash.clone());
            }
            blocks.push(block);
        } else {
            sleep(Duration::from_millis(100)).await;
            continue;
        }
        if blocks.len() >= batch_size || (blocks.len() >= 1 && Instant::now().duration_since(last_commit_time).as_secs() > 2) {
            let start_commit_time = Instant::now();
            let blocks_len = blocks.len();
            let blocks_inserted = commit_blocks(blocks, &db_pool).await.expect("Insert blocks FAILED");
            // let blocks_inserted = bulk_insert_blocks(blocks, &db_pool).await.expect("Insert blocks FAILED");
            let commit_time = Instant::now().duration_since(start_commit_time).as_millis();
            let tps = blocks_len as f64 / commit_time as f64 * 1000f64;
            blocks = vec![];

            if !vcp_started {
                checkpoint = None; // Clear the checkpoint block until vcp has been started
                let cbs_deleted = delete_cbs(&block_hashes, &db_pool).await.expect("Delete chain_blocks FAILED");
                let tas_deleted = delete_tas(&block_hashes, &db_pool).await.expect("Delete transactions_acceptances FAILED");
                block_hashes = vec![];
                if blocks_inserted > 0 && tas_deleted == 0 && cbs_deleted == 0 {
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
                    "Committed {} new blocks, cleared {} cb and {} ta in {}ms ({:.1} tps). Last block: {}",
                    blocks_inserted, cbs_deleted, tas_deleted, commit_time, tps, last_block_datetime
                );
            } else {
                info!(
                    "Committed {} new blocks in {}ms ({:.1} tps. Last block: {}",
                    blocks_inserted, commit_time, tps, last_block_datetime
                );
                if let Some(c) = checkpoint {
                    // Check if the checkpoint candidate's transactions are present
                    let count: i64 = get_tx_count(&c.block_hash, &db_pool).await.expect("Get tx count FAILED");
                    if count == c.tx_count {
                        // Next, let's check if the VCP has proccessed it
                        let is_chain_block = get_is_chain_block(&c.block_hash, &db_pool).await.expect("Get is cb FAILED");
                        if is_chain_block {
                            // All set, the checkpoint block has all transactions present and are marked as a chain block by the VCP
                            let checkpoint_string = hex::encode(c.block_hash);
                            info!("Saving block_checkpoint {}", checkpoint_string);
                            save_checkpoint(&checkpoint_string, &db_pool).await;
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
            last_commit_time = Instant::now();
        }
    }
}

async fn commit_blocks(blocks: Vec<Block>, db_pool: &Pool<Postgres>) -> Result<u64, sqlx::Error> {
    let mut tx = db_pool.begin().await?;

    let sql = format!(
        "INSERT INTO blocks (hash, accepted_id_merkle_root, difficulty, merge_set_blues_hashes, merge_set_reds_hashes,
            selected_parent_hash, bits, blue_score, blue_work, daa_score, hash_merkle_root, nonce, parents, pruning_point,
            timestamp, utxo_commitment, version
        ) VALUES {} ON CONFLICT DO NOTHING",
        (0..blocks.len()).map(|i| format!("({})", (1..=17).map(|c| format!("${}", c + i * 17)).join(", "))).join(", ")
    );

    let mut query = sqlx::query(&sql);
    for block in blocks {
        query = query.bind(block.hash);
        query = query.bind(block.accepted_id_merkle_root);
        query = query.bind(block.difficulty);
        query = query.bind(if !block.merge_set_blues_hashes.is_empty() { Some(block.merge_set_blues_hashes) } else { None });
        query = query.bind(if !block.merge_set_reds_hashes.is_empty() { Some(block.merge_set_reds_hashes) } else { None });
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
    let rows_affected = tx.execute(query).await?.rows_affected();
    tx.commit().await?;
    Ok(rows_affected)
}

async fn bulk_insert_blocks(blocks: &Vec<Block>, db_pool: &Pool<Postgres>) -> Result<u64, sqlx::Error> {
    const TEMP_TABLE_NAME: &str = "blocks_copy_in";

    let mut tx = db_pool.begin().await?;

    let query = format!(
        "CREATE TEMPORARY TABLE {} (
            hash                    BYTEA,
            accepted_id_merkle_root BYTEA,
            difficulty              DOUBLE PRECISION,
            merge_set_blues_hashes  BYTEA[],
            merge_set_reds_hashes   BYTEA[],
            selected_parent_hash    BYTEA,
            bits                    BIGINT,
            blue_score              BIGINT,
            blue_work               BYTEA,
            daa_score               BIGINT,
            hash_merkle_root        BYTEA,
            nonce                   BYTEA,
            parents                 BYTEA[],
            pruning_point           BYTEA,
            timestamp               BIGINT,
            utxo_commitment         BYTEA,
            version                 SMALLINT
        ) ON COMMIT DROP",
        TEMP_TABLE_NAME
    );
    tx.execute(sqlx::query(&query)).await?;

    let mut copy_in = tx.copy_in_raw(format!("COPY {} FROM STDIN", TEMP_TABLE_NAME).as_str()).await?;

    for block in blocks {
        let data = format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            format_binary(&block.hash),
            format_binary(&block.accepted_id_merkle_root),
            format!("{}", block.difficulty),
            format_binary_array(&block.merge_set_blues_hashes),
            format_binary_array(&block.merge_set_reds_hashes),
            format_binary(&block.selected_parent_hash),
            format!("{}", block.bits),
            format!("{}", block.blue_score),
            format_binary(&block.blue_work),
            format!("{}", block.daa_score),
            format_binary(&block.hash_merkle_root),
            format_binary(&block.nonce),
            format_binary_array(&block.parents),
            format_binary(&block.pruning_point),
            format!("{}", block.timestamp),
            format_binary(&block.utxo_commitment),
            format!("{}", block.version),
        );
        copy_in.send(data.as_bytes()).await?;
    }
    copy_in.finish().await?;

    let query = format!("INSERT INTO blocks SELECT * FROM {} ON CONFLICT DO NOTHING", TEMP_TABLE_NAME);
    let rows_affected = tx.execute(sqlx::query(&query)).await?.rows_affected();

    tx.commit().await?;
    Ok(rows_affected)
}

async fn delete_cbs(block_hashes: &Vec<Vec<u8>>, db_pool: &Pool<Postgres>) -> Result<u64, sqlx::Error> {
    Ok(sqlx::query("DELETE FROM chain_blocks WHERE block_hash = ANY($1)").bind(&block_hashes).execute(db_pool).await?.rows_affected())
}

async fn delete_tas(block_hashes: &Vec<Vec<u8>>, db_pool: &Pool<Postgres>) -> Result<u64, sqlx::Error> {
    Ok(sqlx::query("DELETE FROM transactions_acceptances WHERE block_hash = ANY($1)")
        .bind(&block_hashes)
        .execute(db_pool)
        .await?
        .rows_affected())
}

async fn get_tx_count(block_hash: &Vec<u8>, db_pool: &Pool<Postgres>) -> Result<i64, sqlx::Error> {
    sqlx::query("SELECT COUNT(*) FROM blocks_transactions WHERE block_hash = $1").bind(block_hash).fetch_one(db_pool).await?.try_get(0)
}

async fn get_is_chain_block(block_hash: &Vec<u8>, db_pool: &Pool<Postgres>) -> Result<bool, sqlx::Error> {
    sqlx::query("SELECT EXISTS(SELECT 1 FROM chain_blocks WHERE block_hash = $1)")
        .bind(block_hash)
        .fetch_one(db_pool)
        .await?
        .try_get(0)
}
