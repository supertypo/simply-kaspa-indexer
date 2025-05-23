use std::cmp::min;

use kaspa_rpc_core::RpcHash;
use log::{debug, trace};
use simply_kaspa_database::client::KaspaDbClient;
use simply_kaspa_database::models::transaction_acceptance::TransactionAcceptance;

pub async fn add_chain_blocks(batch_scale: f64, added_hashes: &[RpcHash], database: &KaspaDbClient) -> u64 {
    let batch_size = min((500f64 * batch_scale) as usize, 7500);
    if log::log_enabled!(log::Level::Debug) {
        let accepting_blocks = added_hashes.len();
        debug!("Received {} added chain blocks", accepting_blocks);
        trace!("Added chain blocks: \n{:#?}", added_hashes);
    }
    let mut rows_added = 0;
    for added_hashes_chunk in added_hashes.chunks(batch_size) {
        let accepted_transactions: Vec<_> =
            added_hashes_chunk.iter().map(|b| TransactionAcceptance { transaction_id: None, block_hash: Some((*b).into()) }).collect();
        rows_added += database.insert_transaction_acceptances(&accepted_transactions).await.unwrap();
    }
    rows_added
}
