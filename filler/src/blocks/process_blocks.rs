use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crossbeam_queue::ArrayQueue;
use kaspa_database::models::block::Block;
use kaspa_database::models::types::hash::Hash as SqlHash;
use kaspa_rpc_core::RpcBlock;
use tokio::time::sleep;

pub async fn process_blocks(
    run: Arc<AtomicBool>,
    rpc_blocks_queue: Arc<ArrayQueue<(RpcBlock, bool)>>,
    db_blocks_queue: Arc<ArrayQueue<(Block, Vec<SqlHash>, bool)>>,
) {
    while run.load(Ordering::Relaxed) {
        if let Some((block, synced)) = rpc_blocks_queue.pop() {
            let db_block = map_block(&block);
            while db_blocks_queue.is_full() && run.load(Ordering::Relaxed) {
                sleep(Duration::from_millis(100)).await;
            }
            let _ = db_blocks_queue.push((
                db_block,
                block.verbose_data.map(|vd| vd.transaction_ids.into_iter().map(|t| t.into()).collect()).unwrap(),
                synced,
            ));
        } else {
            sleep(Duration::from_millis(100)).await;
        }
    }
}

fn map_block(block: &RpcBlock) -> Block {
    let verbose_data = block.verbose_data.as_ref().expect("Block verbose_data is missing");
    Block {
        hash: block.header.hash.into(),
        accepted_id_merkle_root: block.header.accepted_id_merkle_root.into(),
        difficulty: verbose_data.difficulty,
        merge_set_blues_hashes: verbose_data.merge_set_blues_hashes.iter().map(|v| v.to_owned().into()).collect(),
        merge_set_reds_hashes: verbose_data.merge_set_reds_hashes.iter().map(|v| v.to_owned().into()).collect(),
        selected_parent_hash: verbose_data.selected_parent_hash.into(),
        bits: block.header.bits as i64,
        blue_score: block.header.blue_score as i64,
        blue_work: block.header.blue_work.to_be_bytes_var(),
        daa_score: block.header.daa_score as i64,
        hash_merkle_root: block.header.hash_merkle_root.into(),
        nonce: block.header.nonce.to_be_bytes().to_vec(),
        parents: block.header.parents_by_level[0].iter().map(|v| v.to_owned().into()).collect(),
        pruning_point: block.header.pruning_point.into(),
        timestamp: block.header.timestamp as i64,
        utxo_commitment: block.header.utxo_commitment.into(),
        version: block.header.version as i16,
    }
}
