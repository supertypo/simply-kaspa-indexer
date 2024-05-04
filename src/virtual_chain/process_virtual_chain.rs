extern crate diesel;


use std::time::{Duration, SystemTime};

use diesel::PgConnection;
use diesel::r2d2::{ConnectionManager, Pool};
use kaspa_rpc_core::api::rpc::RpcApi;
use kaspa_wrpc_client::KaspaRpcClient;
use log::info;
use tokio::task;
use tokio::time::sleep;

use crate::kaspad::client::with_retry;
use crate::vars::vars::save_virtual_checkpoint;
use crate::virtual_chain::update_chain_blocks::update_chain_blocks;
use crate::virtual_chain::update_transactions::update_transactions;

pub async fn process_virtual_chain(checkpoint_hash: String,
                                   kaspad_client: KaspaRpcClient,
                                   db_pool: Pool<ConnectionManager<PgConnection>>) -> Result<(), ()> {
    const CHECKPOINT_SAVE_INTERVAL: u64 = 60;
    let start_time = SystemTime::now();
    let mut synced = false;
    let mut checkpoint_hash = hex::decode(checkpoint_hash.as_bytes()).unwrap();
    let mut checkpoint_hash_last_saved = SystemTime::now();

    loop {
        info!("Getting virtual chain from start_hash {}", hex::encode(checkpoint_hash.clone()));
        let response = with_retry(|| kaspad_client.get_virtual_chain_from_block(kaspa_hashes::Hash::from_slice(checkpoint_hash.as_slice()), true)).await
            .expect("Error when invoking GetVirtualChainFromBlock");

        let last_accepted_block_hash = response.accepted_transaction_ids.last().map(|ta| ta.accepting_block_hash.clone());

        let mut last_accepted_block_time: Option<u64> = None;
        if let Some(block_hash) = last_accepted_block_hash.clone() {
            last_accepted_block_time = Some(with_retry(|| kaspad_client.get_block(block_hash, false)).await
                .expect("Erro when invoking GetBlock")
                .header.timestamp);
        }

        let db_pool_clone = db_pool.clone();
        let removed_chain_block_hashes_clone = response.removed_chain_block_hashes.clone();
        let update_transactions_handle = task::spawn_blocking(move || {
            update_transactions(removed_chain_block_hashes_clone, response.accepted_transaction_ids, last_accepted_block_time.clone(), db_pool_clone)
        });
        let db_pool_clone = db_pool.clone();
        let update_chain_blocks_handle = task::spawn_blocking(|| {
            update_chain_blocks(response.added_chain_block_hashes, response.removed_chain_block_hashes, db_pool_clone)
        });
        let (update_transactions_result, _) = tokio::join!(update_transactions_handle, update_chain_blocks_handle);

        if let Ok(Some(new_checkpoint_hash)) = update_transactions_result {
            checkpoint_hash = new_checkpoint_hash;
        }

        if SystemTime::now().duration_since(checkpoint_hash_last_saved).unwrap().as_secs() > CHECKPOINT_SAVE_INTERVAL {
            let checkpoint = hex::encode(checkpoint_hash.clone());
            info!("Saving virtual_checkpoint {}", checkpoint);
            save_virtual_checkpoint(checkpoint, db_pool.clone());
            checkpoint_hash_last_saved = SystemTime::now();
        }
        if !synced {
            let time_to_sync = SystemTime::now().duration_since(start_time).unwrap();
            info!("\x1b[32mVirtual chain processor synced! (in {}:{:0>2}:{:0>2}s)\x1b[0m", time_to_sync.as_secs() / 3600, time_to_sync.as_secs() % 3600 / 60, time_to_sync.as_secs() % 60);
            synced = true;
        }

        sleep(Duration::from_secs(3)).await;
    }
}
