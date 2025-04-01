use std::cmp::min;

use kaspa_rpc_core::RpcAcceptedTransactionIds;
use log::{debug, trace};
use simply_kaspa_database::client::KaspaDbClient;
use simply_kaspa_database::models::transaction_acceptance::TransactionAcceptance;

pub async fn accept_transactions(
    batch_scale: f64,
    accepted_transaction_ids: &[RpcAcceptedTransactionIds],
    database: &KaspaDbClient,
) -> u64 {
    let batch_size = min((500f64 * batch_scale) as usize, 7500);
    if log::log_enabled!(log::Level::Debug) {
        let accepted_count = accepted_transaction_ids.iter().map(|t| t.accepted_transaction_ids.len()).sum::<usize>();
        debug!("Received {} accepted transactions", accepted_count);
        trace!("Accepted transaction ids: \n{:#?}", accepted_transaction_ids);
    }
    let mut rows_added = 0;
    let mut accepted_transactions = vec![];
    for accepted_id in accepted_transaction_ids {
        // Commit all transactions for an accepting block in the same go to avoid incomplete checkpoints:
        accepted_transactions.extend(
            accepted_id
                .accepted_transaction_ids
                .iter()
                .map(|t| TransactionAcceptance {
                    transaction_id: Some(t.to_owned().into()),
                    block_hash: accepted_id.accepting_block_hash.into(),
                })
                .collect::<Vec<_>>(),
        );
        if accepted_transactions.len() >= batch_size {
            rows_added += database.insert_transaction_acceptances(&accepted_transactions).await.unwrap();
            accepted_transactions = vec![];
        }
    }
    if !accepted_transactions.is_empty() {
        rows_added += database.insert_transaction_acceptances(&accepted_transactions).await.unwrap();
    }
    rows_added
}
