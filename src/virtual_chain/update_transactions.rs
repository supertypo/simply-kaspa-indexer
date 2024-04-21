extern crate diesel;

use diesel::{Connection, delete, ExpressionMethods, insert_into, PgConnection, RunQueryDsl};
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::result::Error;
use kaspa_rpc_core::{RpcAcceptedTransactionIds, RpcHash};
use log::{debug, info};

use crate::database::models::TransactionAcceptance;
use crate::database::schema::transactions_acceptances;

pub fn update_transactions(removed_hashes: Vec<RpcHash>, accepted_transaction_ids: Vec<RpcAcceptedTransactionIds>, db_pool: Pool<ConnectionManager<PgConnection>>) -> Option<Vec<u8>> {
    const INSERT_QUEUE_SIZE: usize = 7500;

    let mut rows_removed = 0;
    let mut rows_added = 0;
    let mut last_accepted_block_hash = None;

    let con = &mut db_pool.get().expect("Database connection FAILED");

    con.transaction(|con| {
        let removed_blocks = removed_hashes.into_iter().map(|h| h.as_bytes().to_vec()).collect::<Vec<Vec<u8>>>();
        for removed_blocks_chunk in removed_blocks.chunks(INSERT_QUEUE_SIZE) {
            debug!("Processing {} removed chain blocks", removed_blocks_chunk.len());
            rows_removed += delete(transactions_acceptances::dsl::transactions_acceptances)
                .filter(transactions_acceptances::block_hash.eq_any(removed_blocks_chunk))
                .execute(con)
                .expect("Commit rejected transactions FAILED");
        }
        let mut accepted_transactions = vec![];
        for accepted_id in accepted_transaction_ids {
            for transaction_id in accepted_id.accepted_transaction_ids {
                accepted_transactions.push(TransactionAcceptance {
                    transaction_id: transaction_id.as_bytes().to_vec(),
                    block_hash: accepted_id.accepting_block_hash.as_bytes().to_vec(),
                });
            }
        }
        last_accepted_block_hash = accepted_transactions.last().map(|ta| ta.block_hash.clone());
        for accepted_transactions_chunk in accepted_transactions.chunks(INSERT_QUEUE_SIZE) {
            debug!("Processing {} accepted transactions", accepted_transactions_chunk.len());
            rows_added += insert_into(transactions_acceptances::dsl::transactions_acceptances)
                .values(accepted_transactions_chunk)
                .on_conflict_do_nothing()
                .execute(con)
                .expect("Commit accepted transactions FAILED");
        }
        Ok::<_, Error>(())
    }).expect("Commit rejected/accepted transactions FAILED");
    info!("Committed {} accepted and {} rejected chain blocks", rows_added, rows_removed);
    return last_accepted_block_hash;
}
