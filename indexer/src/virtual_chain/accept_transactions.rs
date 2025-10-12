use futures_util::{StreamExt, stream};
use kaspa_rpc_core::RpcAcceptedTransactionIds;
use log::{debug, trace};
use simply_kaspa_database::client::KaspaDbClient;
use simply_kaspa_database::models::transaction_acceptance::TransactionAcceptance;
use std::cmp::min;

pub async fn accept_transactions(
    batch_scale: f64,
    batch_concurrency: i8,
    accepted_transaction_ids: &[RpcAcceptedTransactionIds],
    database: &KaspaDbClient,
) -> u64 {
    let batch_size = min((1000f64 * batch_scale) as usize, 7500);
    let concurrency = 1 + batch_concurrency as usize;
    if log::log_enabled!(log::Level::Debug) {
        let accepted_count = accepted_transaction_ids.iter().map(|t| t.accepted_transaction_ids.len()).sum::<usize>();
        debug!("Received {} accepted transactions", accepted_count);
        trace!("Accepted transaction ids: \n{:#?}", accepted_transaction_ids);
    }
    let mut accepted_transactions = vec![];
    for accepted_id in accepted_transaction_ids {
        accepted_transactions.extend(accepted_id.accepted_transaction_ids.iter().map(|t| TransactionAcceptance {
            transaction_id: Some(t.to_owned().into()),
            block_hash: Some(accepted_id.accepting_block_hash.into()),
        }));
    }
    accepted_transactions.sort_by(|a, b| a.transaction_id.cmp(&b.transaction_id));
    let batches: Vec<_> = accepted_transactions.chunks(batch_size).map(|c| c.to_vec()).collect();
    let rows_added = stream::iter(batches.into_iter().map(|batch| {
        let db = database.clone();
        async move { db.insert_transaction_acceptances(&batch).await.unwrap_or_else(|e| panic!("Insert acceptances FAILED: {e}")) }
    }))
    .buffer_unordered(concurrency)
    .fold(0, |acc, rows| async move { acc + rows })
    .await;

    rows_added
}
