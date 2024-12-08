use std::cmp::min;

use kaspa_rpc_core::{RpcAcceptedTransactionIds, RpcHash};
use log::{debug, info, trace};
use simply_kaspa_database::client::client::KaspaDbClient;
use simply_kaspa_database::models::transaction_acceptance::TransactionAcceptance;

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

    debug!("Processing {} removed chain blocks", removed_hashes.len());
    let removed_blocks = removed_hashes.into_iter().map(|h| h.to_owned().into()).collect::<Vec<_>>();
    for removed_blocks_chunk in removed_blocks.chunks(batch_size) {
        rows_removed += database.delete_transaction_acceptances(removed_blocks_chunk).await.unwrap();
    }

    debug!(
        "Processing {} accepted transactions",
        accepted_transaction_ids.iter().map(|t| t.accepted_transaction_ids.len()).sum::<usize>()
    );
    let mut accepted_transactions = vec![];
    for accepted_id in accepted_transaction_ids {
        // Commit all transactions for an accepting block in the same go to avoid incomplete checkpoints:
        accepted_transactions.extend(
            accepted_id
                .accepted_transaction_ids
                .iter()
                .map(|t| TransactionAcceptance {
                    transaction_id: t.to_owned().into(),
                    block_hash: accepted_id.accepting_block_hash.into(),
                })
                .collect::<Vec<_>>(),
        );
        if accepted_transactions.len() >= batch_size {
            rows_added += database.insert_transaction_acceptances(&accepted_transactions).await.unwrap();
            accepted_transactions = vec![];
        }
    }
    if accepted_transactions.len() > 0 {
        rows_added += database.insert_transaction_acceptances(&accepted_transactions).await.unwrap();
    }

    info!(
        "Committed {} accepted and {} rejected transactions. Last accepted: {}",
        rows_added,
        rows_removed,
        chrono::DateTime::from_timestamp_millis(last_accepting_time as i64 / 1000 * 1000).unwrap()
    );
}
