use std::cmp::min;

use kaspa_rpc_core::RpcHash;
use log::{debug, info, trace};

use crate::database::client::client::KaspaDbClient;
use crate::database::models::chain_block::ChainBlock;

pub async fn update_chain_blocks(
    batch_scale: f64,
    added_hashes: &Vec<RpcHash>,
    removed_hashes: &Vec<RpcHash>,
    database: &KaspaDbClient,
) {
    let batch_size = min((1000f64 * batch_scale) as usize, 7500);
    if log::log_enabled!(log::Level::Debug) {
        debug!("Received {} added and {} removed chain blocks", added_hashes.len(), removed_hashes.len());
        trace!("Added chain blocks: \n{:#?}", added_hashes);
        trace!("Removed chain blocks: \n{:#?}", removed_hashes);
    }
    let mut rows_removed = 0;
    let mut rows_added = 0;

    let removed_blocks = removed_hashes.into_iter().map(|h| h.as_bytes()).collect::<Vec<[u8; 32]>>();
    for removed_blocks_chunk in removed_blocks.chunks(batch_size) {
        debug!("Processing {} removed chain blocks", removed_blocks_chunk.len());
        rows_removed += database.delete_chain_blocks(removed_blocks_chunk).await.expect("Delete chain blocks FAILED");
    }
    let added_blocks = added_hashes.into_iter().map(|h| ChainBlock { block_hash: h.as_bytes() }).collect::<Vec<ChainBlock>>();
    for added_blocks_chunk in added_blocks.chunks(batch_size) {
        debug!("Processing {} added chain blocks", added_blocks_chunk.len());
        rows_added += database.insert_chain_blocks(added_blocks_chunk).await.expect("Insert chain blocks FAILED");
    }
    info!("Committed {} added and {} removed chain blocks", rows_added, rows_removed);
}
