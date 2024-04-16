extern crate diesel;

use std::collections::HashSet;

use diesel::{Connection, ExpressionMethods, insert_into, PgConnection, QueryDsl, RunQueryDsl, sql_query};
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::result::Error;
use diesel::upsert::excluded;
use kaspa_rpc_core::RpcHash;
use log::{debug, info, trace};

use crate::database::models::Block;
use crate::database::schema::blocks;

pub fn update_chain_blocks(added_hashes: Vec<RpcHash>, removed_hashes: Vec<RpcHash>, db_pool: Pool<ConnectionManager<PgConnection>>) {
    const INSERT_QUEUE_SIZE: usize = 7500;
    info!("Received {} added and {} removed chain blocks", added_hashes.len(), removed_hashes.len());
    trace!("Added chain blocks: \n{:#?}", added_hashes);
    trace!("Removed chain blocks: \n{:#?}", removed_hashes);

    let mut updated_chain_blocks = added_hashes.into_iter().map(|h| Block::new(h.as_bytes().to_vec(), true)).collect::<Vec<Block>>();
    updated_chain_blocks.append(&mut removed_hashes.into_iter().map(|h| Block::new(h.as_bytes().to_vec(), false)).collect::<Vec<Block>>());

    if !updated_chain_blocks.is_empty() {
        let con = &mut db_pool.get().expect("Database connection FAILED");
        for updated_chain_blocks_chunk in updated_chain_blocks.chunks(INSERT_QUEUE_SIZE) {
            let mut rows_affected = 0;
            con.transaction(|con| {
                sql_query("LOCK TABLE blocks IN EXCLUSIVE MODE").execute(con).expect("Locking table before commit transactions to database FAILED");

                let mut update_chain_blocks_set: HashSet<&Block> = HashSet::from_iter(updated_chain_blocks_chunk.iter());
                debug!("Processing {} updated chain blocks", update_chain_blocks_set.len());
                // Find existing identical blocks and remove them from the insert queue
                blocks::dsl::blocks
                    .filter(blocks::hash.eq_any(update_chain_blocks_set.iter().map(|&b| b.hash.clone()).collect::<Vec<Vec<u8>>>()))
                    .load::<Block>(con)
                    .expect("Select updated chain blocks from database FAILED").iter()
                    .for_each(|b| {
                        if update_chain_blocks_set.get(b).unwrap().is_chain_block == b.is_chain_block {
                            update_chain_blocks_set.remove(b);
                        }
                    });
                // Save changed rows
                rows_affected = insert_into(blocks::dsl::blocks)
                    .values(Vec::from_iter(update_chain_blocks_set))
                    .on_conflict(blocks::hash)
                    .do_update()
                    .set(blocks::is_chain_block.eq(excluded(blocks::is_chain_block))) // Upsert guarantees consistency when rows where inserted since the select
                    .execute(con)
                    .expect("Commit updated chain blocks to database FAILED");
                Ok::<_, Error>(())
            }).expect("Commit updated chain blocks to database FAILED");
            info!("Committed {} updated chain blocks to database", rows_affected);
        }
    }
}
