use std::cmp::min;

use kaspa_rpc_core::{RpcAcceptedTransactionIds, RpcHash};
use log::{debug, info, trace};
use kaspa_database::client::client::KaspaDbClient;
use kaspa_database::models::types::hash::Hash as SqlHash;
use kaspa_database::models::transaction_acceptance::TransactionAcceptance;

pub async fn update_txs(
    batch_scale: f64,
    removed_hashes: &Vec<RpcHash>,
    accepted_transaction_ids: &Vec<RpcAcceptedTransactionIds>,
    last_accepting_time: u64,
    database: &KaspaDbClient,
) {
    let batch_size = min((1000f64 * batch_scale) as usize, 7500);
    if log::log_enabled!(log::Level::Debug) {
        let accepted_count = accepted_transaction_ids.iter().map(|t| t.accepted_transaction_ids.len()).sum::<usize>();
        debug!("Received {} accepted transactions and {} removed chain blocks", accepted_count, removed_hashes.len());
        trace!("Accepted transaction ids: \n{:#?}", accepted_transaction_ids);
        trace!("Removed chain blocks: \n{:#?}", removed_hashes);
    }

    let mut rows_removed = 0;
    let mut rows_added = 0;

    let removed_blocks = removed_hashes.into_iter().map(|h| SqlHash::from(*h)).collect::<Vec<_>>();
    for removed_blocks_chunk in removed_blocks.chunks(batch_size) {
        debug!("Processing {} removed chain blocks", removed_blocks_chunk.len());
        rows_removed +=
            database.delete_transaction_acceptances(removed_blocks_chunk).await.expect("Delete accepted transactions FAILED");
    }
    let mut accepted_transactions = vec![];
    for accepted_id in accepted_transaction_ids {
        for transaction_id in accepted_id.accepted_transaction_ids.iter() {
            accepted_transactions.push(TransactionAcceptance {
                transaction_id: SqlHash::from(*transaction_id),
                block_hash: SqlHash::from(accepted_id.accepting_block_hash),
            });
        }
    }
    for accepted_transactions_chunk in accepted_transactions.chunks(batch_size) {
        debug!("Processing {} accepted transactions", accepted_transactions_chunk.len());
        rows_added +=
            database.insert_transaction_acceptances(accepted_transactions_chunk).await.expect("Insert accepted transactions FAILED");
    }

    info!(
        "Committed {} accepted and {} rejected transactions. Last accepted: {}",
        rows_added,
        rows_removed,
        chrono::DateTime::from_timestamp_millis(last_accepting_time as i64 / 1000 * 1000).unwrap()
    );
}
