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

    let con = &mut db_pool.get().expect("Database connection FAILED");

    let removed_blocks = removed_hashes.into_iter().map(|h| h.as_bytes().to_vec()).collect::<Vec<Vec<u8>>>();
    for removed_blocks_chunk in removed_blocks.chunks(INSERT_QUEUE_SIZE) {
        let mut rows_affected = 0;
        con.transaction(|con| {
            debug!("Processing {} removed chain blocks", removed_blocks_chunk.len());
            rows_affected = delete(transactions_acceptances::dsl::transactions_acceptances)
                .filter(transactions_acceptances::block_hash.eq_any(removed_blocks_chunk))
                .execute(con)
                .expect("Commit rejected transactions to database FAILED");
            Ok::<_, Error>(())
        }).expect("Commit rejected transactions to database FAILED");
        info!("Committed {} rejected transactions to database", rows_affected);
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
    for accepted_transactions_chunk in accepted_transactions.chunks(INSERT_QUEUE_SIZE) {
        let mut rows_affected = 0;
        con.transaction(|con| {
            debug!("Processing {} accepted transactions", accepted_transactions_chunk.len());
            rows_affected = insert_into(transactions_acceptances::dsl::transactions_acceptances)
                .values(accepted_transactions_chunk)
                .on_conflict_do_nothing()
                .execute(con)
                .expect("Commit accepted transactions to database FAILED");
            Ok::<_, Error>(())
        }).expect("Commit accepted transactions to database FAILED");
        info!("Committed {} accepted transactions to database", rows_affected);
    }
    return accepted_transactions.last().map(|ta| ta.block_hash.clone());
}
