extern crate diesel;

use std::cmp::min;

use diesel::r2d2::{ConnectionManager, Pool};
use diesel::result::Error;
use diesel::{delete, insert_into, Connection, ExpressionMethods, PgConnection, RunQueryDsl};
use kaspa_rpc_core::RpcHash;
use log::{debug, info, trace};

use crate::database::models::ChainBlock;
use crate::database::schema::chain_blocks;

pub fn update_chain_blocks(
    buffer_size: f64,
    added_hashes: Vec<RpcHash>,
    removed_hashes: Vec<RpcHash>,
    db_pool: Pool<ConnectionManager<PgConnection>>,
) {
    let batch_insert_size = min((2000f64 * buffer_size) as usize, 7500); // ~7500 is the max batch size db supports
    debug!("Received {} added and {} removed chain blocks", added_hashes.len(), removed_hashes.len());
    trace!("Added chain blocks: \n{:#?}", added_hashes);
    trace!("Removed chain blocks: \n{:#?}", removed_hashes);

    let mut rows_removed = 0;
    let mut rows_added = 0;

    let con = &mut db_pool.get().expect("Database connection FAILED");
    con.transaction(|con| {
        let removed_blocks = removed_hashes.into_iter().map(|h| h.as_bytes().to_vec()).collect::<Vec<Vec<u8>>>();
        for removed_blocks_chunk in removed_blocks.chunks(batch_insert_size) {
            debug!("Processing {} removed chain blocks", removed_blocks_chunk.len());
            rows_removed += delete(chain_blocks::dsl::chain_blocks)
                .filter(chain_blocks::block_hash.eq_any(removed_blocks_chunk))
                .execute(con)
                .expect("Commit removed chain blocks FAILED");
        }
        let added_blocks =
            added_hashes.into_iter().map(|h| ChainBlock { block_hash: h.as_bytes().to_vec() }).collect::<Vec<ChainBlock>>();
        for added_blocks_chunk in added_blocks.chunks(batch_insert_size) {
            debug!("Processing {} added chain blocks", added_blocks_chunk.len());
            rows_added += insert_into(chain_blocks::dsl::chain_blocks)
                .values(added_blocks_chunk)
                .on_conflict_do_nothing()
                .execute(con)
                .expect("Commit added chain blocks FAILED");
        }
        Ok::<_, Error>(())
    })
    .expect("Commit removed/added chain blocks FAILED");
    info!("Committed {} added and {} removed chain blocks", rows_added, rows_removed);
}
