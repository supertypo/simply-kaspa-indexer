extern crate diesel;

use std::cmp::min;

use itertools::Itertools;
use kaspa_rpc_core::RpcHash;
use log::{debug, info, trace};
use sqlx::{Pool, Postgres};

use crate::database::models::chain_block::ChainBlock;

pub async fn update_chain_blocks(
    batch_scale: f64,
    added_hashes: &Vec<RpcHash>,
    removed_hashes: &Vec<RpcHash>,
    db_pool: &Pool<Postgres>,
) {
    let batch_size = min((1000f64 * batch_scale) as usize, 7500);
    if log::log_enabled!(log::Level::Debug) {
        debug!("Received {} added and {} removed chain blocks", added_hashes.len(), removed_hashes.len());
        trace!("Added chain blocks: \n{:#?}", added_hashes);
        trace!("Removed chain blocks: \n{:#?}", removed_hashes);
    }
    let mut rows_removed = 0;
    let mut rows_added = 0;

    let removed_blocks = removed_hashes.into_iter().map(|h| h.as_bytes().to_vec()).collect::<Vec<Vec<u8>>>();
    for removed_blocks_chunk in removed_blocks.chunks(batch_size) {
        debug!("Processing {} removed chain blocks", removed_blocks_chunk.len());
        rows_removed += delete_cbs(removed_blocks_chunk, db_pool).await.expect("Delete accepted transactions FAILED");
    }
    let added_blocks = added_hashes.into_iter().map(|h| ChainBlock { block_hash: h.as_bytes().to_vec() }).collect::<Vec<ChainBlock>>();
    for added_blocks_chunk in added_blocks.chunks(batch_size) {
        debug!("Processing {} added chain blocks", added_blocks_chunk.len());
        rows_added += insert_cbs(added_blocks_chunk, db_pool).await.expect("Delete accepted transactions FAILED");
    }
    info!("Committed {} added and {} removed chain blocks", rows_added, rows_removed);
}

async fn delete_cbs(block_hashes: &[Vec<u8>], db_pool: &Pool<Postgres>) -> Result<u64, sqlx::Error> {
    Ok(sqlx::query("DELETE FROM chain_blocks WHERE block_hash = ANY($1)").bind(&block_hashes).execute(db_pool).await?.rows_affected())
}

async fn insert_cbs(cbs: &[ChainBlock], db_pool: &Pool<Postgres>) -> Result<u64, sqlx::Error> {
    const COLS: usize = 1;
    let sql = format!(
        "INSERT INTO chain_blocks (block_hash) VALUES {} ON CONFLICT DO NOTHING",
        (0..cbs.len()).map(|i| format!("({})", (1..=COLS).map(|c| format!("${}", c + i * COLS)).join(", "))).join(", ")
    );
    let mut query = sqlx::query(&sql);
    for cb in cbs {
        query = query.bind(&cb.block_hash);
    }
    Ok(query.execute(db_pool).await?.rows_affected())
}
