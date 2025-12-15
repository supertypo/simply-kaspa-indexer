use std::cmp::min;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use crate::blocks::fetch_blocks::BlockData;
use crate::checkpoint::{CheckpointBlock, CheckpointOrigin};
use crate::settings::Settings;
use crate::web::model::metrics::Metrics;
use chrono::DateTime;
use crossbeam_queue::ArrayQueue;
use log::{debug, info, warn};
use simply_kaspa_cli::cli_args::{CliDisable, CliEnable};
use simply_kaspa_database::client::KaspaDbClient;
use simply_kaspa_database::models::block::Block;
use simply_kaspa_database::models::block_parent::BlockParent;
use simply_kaspa_database::models::types::hash::Hash as SqlHash;
use crate::mapping::mapper::KaspaDbMapper;
use simply_kaspa_signal::signal_handler::SignalHandler;
use tokio::sync::RwLock;
use tokio::time::sleep;
use std::collections::HashMap;
use kaspa_hashes::Hash;
use crate::seqcom::merkle_hash;
use simply_kaspa_database::models::sequencing_commitment::SequencingCommitment;

pub async fn process_blocks(
    settings: Settings,
    signal_handler: SignalHandler,
    metrics: Arc<RwLock<Metrics>>,
    start_vcp: Arc<AtomicBool>,
    rpc_blocks_queue: Arc<ArrayQueue<BlockData>>,
    checkpoint_queue: Arc<ArrayQueue<CheckpointBlock>>,
    database: KaspaDbClient,
    mapper: KaspaDbMapper,
) {
    const NOOP_DELETES_BEFORE_VCP: i32 = 10;
    let batch_scale = settings.cli_args.batch_scale;
    let batch_size = (800f64 * batch_scale) as usize;
    let seqcom_enabled = settings.cli_args.is_enabled(CliEnable::SeqCom);
    let disable_virtual_chain_processing = settings.cli_args.is_disabled(CliDisable::VirtualChainProcessing);
    let disable_vcp_wait_for_sync = settings.disable_vcp_wait_for_sync;
    let disable_blocks = settings.cli_args.is_disabled(CliDisable::BlocksTable);
    let disable_block_relations = settings.cli_args.is_disabled(CliDisable::BlockParentTable);
    let mut first_block = true;
    let mut vcp_started = false;
    let mut blocks = vec![];
    let mut blocks_parents = vec![];
    let mut sequencing_commitments = if seqcom_enabled { vec![] } else { vec![] };
    let mut batch_seqcom_cache: HashMap<Hash, SequencingCommitment> = if seqcom_enabled { HashMap::new() } else { HashMap::new() };
    let mut checkpoint_blocks = vec![];
    let mut last_commit_time = Instant::now();
    let mut noop_delete_count = 0;

    while !signal_handler.is_shutdown() {
        if let Some(block_data) = rpc_blocks_queue.pop() {
            let synced = block_data.synced;

            if !disable_blocks {
                blocks.push(mapper.map_block(&block_data.block));
            }
            if !disable_block_relations {
                blocks_parents.extend(mapper.map_block_parents(&block_data.block));
            }

            if seqcom_enabled {
                let parent_seqcom = if let Some(parent_hash) = block_data.block.header.parents_by_level.get(0).and_then(|x| x.get(0)) {
                    let parent_hash_obj_db: simply_kaspa_database::models::types::hash::Hash = (*parent_hash).into();
                    let parent_hash_obj_cache: kaspa_hashes::Hash = parent_hash_obj_db.clone().into();
                    if let Some(cached_seqcom) = batch_seqcom_cache.get(&parent_hash_obj_cache) {
                        Some(cached_seqcom.seqcom_hash.clone())
                    } else {
                        database.get_sequencing_commitment(&parent_hash_obj_db).await.unwrap().map(|x| x.seqcom_hash)
                    }
                } else {
                    None
                };

                // KIP-15: SeqCom(block) = H(parent_seqcom || AIDMR)
                // AIDMR = Accepted ID Merkle Root from block header
                let aidmr = block_data.block.header.accepted_id_merkle_root;
                let new_seqcom = merkle_hash(parent_seqcom.clone().unwrap_or_default().into(), aidmr);

                let current_seqcom = SequencingCommitment {
                    block_hash: block_data.block.header.hash.into(),
                    seqcom_hash: new_seqcom.into(),
                    parent_seqcom_hash: parent_seqcom,
                };
                batch_seqcom_cache.insert(current_seqcom.block_hash.clone().into(), current_seqcom.clone());
                sequencing_commitments.push(current_seqcom);
            }

            checkpoint_blocks.push(CheckpointBlock {
                origin: CheckpointOrigin::Blocks,
                hash: block_data.block.header.hash.into(),
                timestamp: block_data.block.header.timestamp,
                daa_score: block_data.block.header.daa_score,
                blue_score: block_data.block.header.blue_score,
            });

            if checkpoint_blocks.len() >= batch_size
                || (!checkpoint_blocks.is_empty() && Instant::now().duration_since(last_commit_time).as_secs() > 2)
            {
                let start_commit_time = Instant::now();
                debug!("Committing {} blocks ({} parents)", blocks.len(), blocks_parents.len());
                let last_checkpoint_block = checkpoint_blocks.last().unwrap().clone();
                let blocks_inserted = if !disable_blocks { insert_blocks(batch_scale, blocks, database.clone()).await } else { 0 };
                let block_parents_inserted = if !disable_block_relations {
                    insert_block_parents(batch_scale, &blocks_parents, database.clone()).await
                } else {
                    0
                };
                let sequencing_commitments_inserted = if seqcom_enabled {
                    insert_sequencing_commitments(batch_scale, &sequencing_commitments, database.clone()).await
                } else {
                    0
                };
                let last_block_datetime = DateTime::from_timestamp_millis(last_checkpoint_block.timestamp as i64).unwrap();

                if !vcp_started && !disable_virtual_chain_processing {
                    let tas_deleted = delete_transaction_acceptances(
                        batch_scale,
                        // Skip deleting acceptance for first block hash, as it's not re-added by vcp:
                        checkpoint_blocks.iter().skip(first_block as usize).map(|c| c.hash.clone()).collect(),
                        database.clone(),
                    )
                    .await;
                    first_block = false;
                    if (disable_vcp_wait_for_sync || synced) && tas_deleted == 0 {
                        noop_delete_count += 1;
                    } else {
                        noop_delete_count = 0;
                    }
                    let commit_time = Instant::now().duration_since(start_commit_time).as_millis();
                    let bps = checkpoint_blocks.len() as f64 / commit_time as f64 * 1000f64;
                    if seqcom_enabled {
                        info!(
                            "Committed {} new blocks in {}ms ({:.1} bps, {} bp, {} sc) [clr {} ta]. Last block: {}",
                            blocks_inserted, commit_time, bps, block_parents_inserted, sequencing_commitments_inserted, tas_deleted, last_block_datetime
                        );
                    } else {
                        info!(
                            "Committed {} new blocks in {}ms ({:.1} bps, {} bp) [clr {} ta]. Last block: {}",
                            blocks_inserted, commit_time, bps, block_parents_inserted, tas_deleted, last_block_datetime
                        );
                    }
                    if noop_delete_count >= NOOP_DELETES_BEFORE_VCP {
                        info!("Notifying virtual chain processor");
                        start_vcp.store(true, Ordering::Relaxed);
                        vcp_started = true;
                    }
                } else if !disable_blocks || !disable_block_relations {
                    let commit_time = Instant::now().duration_since(start_commit_time).as_millis();
                    let bps = checkpoint_blocks.len() as f64 / commit_time as f64 * 1000f64;
                    if seqcom_enabled {
                        info!(
                            "Committed {} new blocks in {}ms ({:.1} bps, {} bp, {} sc). Last block: {}",
                            blocks_inserted, commit_time, bps, block_parents_inserted, sequencing_commitments_inserted, last_block_datetime
                        );
                    } else {
                        info!(
                            "Committed {} new blocks in {}ms ({:.1} bps, {} bp). Last block: {}",
                            blocks_inserted, commit_time, bps, block_parents_inserted, last_block_datetime
                        );
                    }
                }

                let mut metrics = metrics.write().await;
                metrics.components.block_processor.update_last_block(last_checkpoint_block.into());
                drop(metrics);

                for checkpoint_block in checkpoint_blocks {
                    while checkpoint_queue.push(checkpoint_block.clone()).is_err() {
                        warn!("Checkpoint queue is full");
                        sleep(Duration::from_secs(1)).await;
                    }
                }
                blocks = vec![];
                checkpoint_blocks = vec![];
                blocks_parents = vec![];
                sequencing_commitments = vec![];
                batch_seqcom_cache.clear();
                last_commit_time = Instant::now();
            }
        } else {
            sleep(Duration::from_millis(100)).await;
        }
    }
}

async fn insert_blocks(batch_scale: f64, values: Vec<Block>, database: KaspaDbClient) -> u64 {
    let batch_size = min((200f64 * batch_scale) as usize, 3500); // 2^16 / fields
    let key = "blocks";
    let start_time = Instant::now();
    debug!("Processing {} {}", values.len(), key);
    let mut rows_affected = 0;
    for batch_values in values.chunks(batch_size) {
        rows_affected += database.insert_blocks(batch_values).await.unwrap_or_else(|e| panic!("Insert {key} FAILED: {e}"));
    }
    debug!("Committed {} {} in {}ms", rows_affected, key, Instant::now().duration_since(start_time).as_millis());
    rows_affected
}

async fn insert_block_parents(batch_scale: f64, values: &[BlockParent], database: KaspaDbClient) -> u64 {
    let batch_size = min((400f64 * batch_scale) as usize, 10000); // 2^16 / fields
    let key = "block_parents";
    let start_time = Instant::now();
    debug!("Processing {} {}", values.len(), key);
    let mut rows_affected = 0;
    for batch_values in values.chunks(batch_size) {
        rows_affected += database.insert_block_parents(batch_values).await.unwrap_or_else(|e| panic!("Insert {key} FAILED: {e}"));
    }
    debug!("Committed {} {} in {}ms", rows_affected, key, Instant::now().duration_since(start_time).as_millis());
    rows_affected
}

async fn insert_sequencing_commitments(batch_scale: f64, values: &[SequencingCommitment], database: KaspaDbClient) -> u64 {
    let batch_size = min((400f64 * batch_scale) as usize, 10000); // 2^16 / fields
    let key = "sequencing_commitments";
    let start_time = Instant::now();
    debug!("Processing {} {}", values.len(), key);
    let mut rows_affected = 0;
    for batch_values in values.chunks(batch_size) {
        rows_affected += database.insert_sequencing_commitments(batch_values).await.unwrap_or_else(|e| panic!("Insert {key} FAILED: {e}"));
    }
    debug!("Committed {} {} in {}ms", rows_affected, key, Instant::now().duration_since(start_time).as_millis());
    rows_affected
}

async fn delete_transaction_acceptances(batch_scale: f64, block_hashes: Vec<SqlHash>, db: KaspaDbClient) -> u64 {
    let batch_size = min((100f64 * batch_scale) as usize, 50000); // 2^16 / fields
    let key = "transaction_acceptances";
    let start_time = Instant::now();
    debug!("Clearing {} {}", block_hashes.len(), key);
    let mut rows_affected = 0;
    for batch_values in block_hashes.chunks(batch_size) {
        rows_affected +=
            db.delete_transaction_acceptances(batch_values).await.unwrap_or_else(|e| panic!("Deleting {key} FAILED: {e}"));
    }
    debug!("Cleared {} {} in {}ms", rows_affected, key, Instant::now().duration_since(start_time).as_millis());
    rows_affected
}