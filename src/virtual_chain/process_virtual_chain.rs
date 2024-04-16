extern crate diesel;


use std::sync::Arc;
use std::time::{Duration, SystemTime};

use crossbeam_queue::ArrayQueue;
use diesel::PgConnection;
use diesel::r2d2::{ConnectionManager, Pool};
use kaspa_rpc_core::api::rpc::RpcApi;
use kaspa_wrpc_client::KaspaRpcClient;
use log::{info, warn};
use tokio::time::sleep;
use crate::kaspad::client::with_retry;

use crate::vars::vars::save_virtual_checkpoint;
use crate::virtual_chain::update_chain_blocks::update_chain_blocks;
use crate::virtual_chain::update_transactions::update_transactions;

pub async fn process_virtual_chain(checkpoint_hash: String,
                                   synced_queue: Arc<ArrayQueue<bool>>,
                                   kaspad_client: KaspaRpcClient,
                                   db_pool: Pool<ConnectionManager<PgConnection>>) -> Result<(), ()> {
    let start_time = SystemTime::now();
    let mut synced = false;
    let mut checkpoint_hash = hex::decode(checkpoint_hash.as_bytes()).unwrap();
    let mut checkpoint_hash_last_saved = SystemTime::now();

    while synced_queue.is_empty() || !synced_queue.pop().unwrap() {
        warn!("Not synced yet, sleeping for 5 seconds...");
        sleep(Duration::from_secs(5)).await;
    }
    loop {
        info!("Getting virtual chain from start_hash={}", hex::encode(checkpoint_hash.clone()));
        let response = with_retry(|| kaspad_client.get_virtual_chain_from_block(kaspa_hashes::Hash::from_slice(checkpoint_hash.as_slice()), true)).await
            .expect("Error when invoking GetBlocks");

        checkpoint_hash = update_transactions(response.removed_chain_block_hashes.clone(), response.accepted_transaction_ids, db_pool.clone()).unwrap_or(checkpoint_hash.clone());
        update_chain_blocks(response.added_chain_block_hashes, response.removed_chain_block_hashes, db_pool.clone());

        if SystemTime::now().duration_since(checkpoint_hash_last_saved).unwrap().as_secs() > 60 {
            save_virtual_checkpoint(hex::encode(checkpoint_hash.clone()), db_pool.clone());
            checkpoint_hash_last_saved = SystemTime::now();
        }
        if !synced {
            let time_to_sync = SystemTime::now().duration_since(start_time).unwrap();
            info!("Virtual chain processor synced! (in {}:{:0>2}:{:0>2}s)", time_to_sync.as_secs() / 3600, time_to_sync.as_secs() % 3600 / 60, time_to_sync.as_secs() % 60);
            synced = true;
        }

        sleep(Duration::from_secs(3)).await;
    }
}
