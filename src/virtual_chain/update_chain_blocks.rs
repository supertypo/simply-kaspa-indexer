extern crate diesel;

use diesel::{Connection, delete, ExpressionMethods, insert_into, PgConnection, RunQueryDsl};
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::result::Error;
use kaspa_rpc_core::RpcHash;
use log::{debug, info, trace};

use crate::database::models::BlockChain;
use crate::database::schema::blocks_chains;

pub fn update_chain_blocks(added_hashes: Vec<RpcHash>, removed_hashes: Vec<RpcHash>, db_pool: Pool<ConnectionManager<PgConnection>>) {
    const INSERT_QUEUE_SIZE: usize = 7500;
    info!("Received {} added and {} removed chain blocks", added_hashes.len(), removed_hashes.len());
    trace!("Added chain blocks: \n{:#?}", added_hashes);
    trace!("Removed chain blocks: \n{:#?}", removed_hashes);

    if !added_hashes.is_empty() || !removed_hashes.is_empty() {
        let con = &mut db_pool.get().expect("Database connection FAILED");

        let removed_blocks = removed_hashes.into_iter().map(|h| h.as_bytes().to_vec()).collect::<Vec<Vec<u8>>>();
        for removed_blocks_chunk in removed_blocks.chunks(INSERT_QUEUE_SIZE) {
            let mut rows_affected = 0;
            con.transaction(|con| {
                debug!("Processing {} removed chain blocks", removed_blocks_chunk.len());
                rows_affected = delete(blocks_chains::dsl::blocks_chains)
                    .filter(blocks_chains::block_hash.eq_any(removed_blocks_chunk))
                    .execute(con)
                    .expect("Commit removed chain blocks to database FAILED");
                Ok::<_, Error>(())
            }).expect("Commit removed chain blocks to database FAILED");
            info!("Committed {} removed chain blocks to database", rows_affected);
        }

        let added_blocks = added_hashes.into_iter().map(|h| BlockChain { block_hash: h.as_bytes().to_vec() }).collect::<Vec<BlockChain>>();
        for added_blocks_chunk in added_blocks.chunks(INSERT_QUEUE_SIZE) {
            let mut rows_affected = 0;
            con.transaction(|con| {
                debug!("Processing {} added chain blocks", added_blocks_chunk.len());
                rows_affected = insert_into(blocks_chains::dsl::blocks_chains)
                    .values(added_blocks_chunk)
                    .on_conflict_do_nothing()
                    .execute(con)
                    .expect("Commit added chain blocks to database FAILED");
                Ok::<_, Error>(())
            }).expect("Commit added chain blocks to database FAILED");
            info!("Committed {} added chain blocks to database", rows_affected);
        }
    }
}
