extern crate diesel;

use std::cmp::min;

use diesel::r2d2::{ConnectionManager, Pool};
use diesel::result::Error;
use diesel::{delete, insert_into, Connection, ExpressionMethods, PgConnection, RunQueryDsl};
use kaspa_rpc_core::{RpcAcceptedTransactionIds, RpcHash};
use log::{debug, info, trace};

use crate::database::models::TransactionAcceptance;
use crate::database::schema::transactions_acceptances;

pub fn update_transactions(
    batch_scale: f64,
    removed_hashes: Vec<RpcHash>,
    accepted_transaction_ids: Vec<RpcAcceptedTransactionIds>,
    last_accepting_time: u64,
    db_pool: Pool<ConnectionManager<PgConnection>>,
) {
    // ~7500 is the max batch size db supports:
    let batch_insert_size = min((2000f64 * batch_scale) as usize, 7500);
    if log::log_enabled!(log::Level::Debug) {
        let accepted_count = accepted_transaction_ids.iter().map(|t| t.accepted_transaction_ids.len()).sum::<usize>();
        debug!("Received {} accepted transactions and {} removed chain blocks", accepted_count, removed_hashes.len());
        trace!("Accepted transaction ids: \n{:#?}", accepted_transaction_ids);
        trace!("Removed chain blocks: \n{:#?}", removed_hashes);
    }

    let mut rows_removed = 0;
    let mut rows_added = 0;

    let con = &mut db_pool.get().expect("Database connection FAILED");
    con.transaction(|con| {
        let removed_blocks = removed_hashes.into_iter().map(|h| h.as_bytes().to_vec()).collect::<Vec<Vec<u8>>>();
        for removed_blocks_chunk in removed_blocks.chunks(batch_insert_size) {
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
        for accepted_transactions_chunk in accepted_transactions.chunks(batch_insert_size) {
            debug!("Processing {} accepted transactions", accepted_transactions_chunk.len());
            rows_added += insert_into(transactions_acceptances::dsl::transactions_acceptances)
                .values(accepted_transactions_chunk)
                .on_conflict_do_nothing()
                .execute(con)
                .expect("Commit accepted transactions FAILED");
        }
        Ok::<_, Error>(())
    })
    .expect("Commit rejected/accepted transactions FAILED");

    info!(
        "Committed {} accepted and {} rejected transactions. Last accepted: {}",
        rows_added,
        rows_removed,
        chrono::DateTime::from_timestamp_millis(last_accepting_time as i64 / 1000 * 1000).unwrap()
    );
}
