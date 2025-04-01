use std::cmp::min;

use kaspa_rpc_core::RpcHash;
use log::{debug, trace};
use simply_kaspa_database::client::KaspaDbClient;

pub async fn remove_chain_blocks(batch_scale: f64, removed_hashes: &[RpcHash], database: &KaspaDbClient) -> u64 {
    let batch_size = min((500f64 * batch_scale) as usize, 7500);
    if log::log_enabled!(log::Level::Debug) {
        debug!("Received {} removed chain blocks", removed_hashes.len());
        trace!("Removed chain blocks: \n{:#?}", removed_hashes);
    }
    let mut rows_removed = 0;
    let removed_blocks = removed_hashes.iter().map(|h| h.to_owned().into()).collect::<Vec<_>>();
    for removed_blocks_chunk in removed_blocks.chunks(batch_size) {
        rows_removed += database.delete_transaction_acceptances(removed_blocks_chunk).await.unwrap();
    }
    rows_removed
}
