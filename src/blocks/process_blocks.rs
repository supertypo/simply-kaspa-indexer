use std::sync::Arc;
use std::time::Duration;

use crossbeam_queue::ArrayQueue;
use kaspa_rpc_core::RpcBlock;
use tokio::time::sleep;

use crate::database::models::Block;

pub async fn process_blocks(rpc_blocks_queue: Arc<ArrayQueue<RpcBlock>>,
                            db_blocks_queue: Arc<ArrayQueue<(Block, Vec<Vec<u8>>)>>) -> Result<(), ()> {
    loop {
        let block_option = rpc_blocks_queue.pop();
        if block_option.is_none() {
            sleep(Duration::from_millis(100)).await;
            continue;
        }
        let block = block_option.unwrap();
        let db_block = Block {
            hash: block.header.hash.as_bytes().to_vec(),
            accepted_id_merkle_root: Some(block.header.accepted_id_merkle_root.as_bytes().to_vec()),
            difficulty: block.verbose_data.as_ref().map(|v| v.difficulty),
            merge_set_blues_hashes: block.verbose_data.as_ref().map(|v| v.merge_set_blues_hashes.iter()
                .map(|w| Some(w.as_bytes().to_vec())).collect()),
            merge_set_reds_hashes: block.verbose_data.as_ref().map(|v| v.merge_set_reds_hashes.iter()
                .map(|w| Some(w.as_bytes().to_vec())).collect()),
            selected_parent_hash: block.verbose_data.as_ref().map(|v| v.selected_parent_hash.as_bytes().to_vec()),
            bits: Some(block.header.bits as i64),
            blue_score: Some(block.header.blue_score as i64),
            blue_work: Some(block.header.blue_work.to_be_bytes_var()),
            daa_score: Some(block.header.daa_score as i64),
            hash_merkle_root: Some(block.header.hash_merkle_root.as_bytes().to_vec()),
            nonce: Some(block.header.nonce.to_be_bytes().to_vec()),
            parents: Some(block.header.parents_by_level[0].iter().map(|v| Some(v.as_bytes().to_vec())).collect()),
            pruning_point: Some(block.header.pruning_point.as_bytes().to_vec()),
            timestamp: Some((block.header.timestamp / 1000) as i32),
            utxo_commitment: Some(block.header.utxo_commitment.as_bytes().to_vec()),
            version: Some(block.header.version as i16),
        };
        while db_blocks_queue.is_full() {
            sleep(Duration::from_millis(100)).await;
        }
        let _ = db_blocks_queue.push((db_block, block.verbose_data.as_ref()
            .map(|vd| vd.transaction_ids.iter()
                .map(|t| t.as_bytes().to_vec())
                .collect()).unwrap()));
    }
}
