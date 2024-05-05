extern crate diesel;


use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime};

use diesel::{Connection, PgConnection};
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::result::Error;
use kaspa_rpc_core::api::rpc::RpcApi;
use kaspa_wrpc_client::KaspaRpcClient;
use log::{debug, info};
use tokio::time::sleep;

use crate::kaspad::client::with_retry;
use crate::vars::vars::save_virtual_checkpoint;
use crate::virtual_chain::update_chain_blocks::update_chain_blocks;
use crate::virtual_chain::update_transactions::update_transactions;

pub async fn process_virtual_chain(running: Arc<AtomicBool>,
                                   checkpoint_hash: String,
                                   kaspad_client: KaspaRpcClient,
                                   db_pool: Pool<ConnectionManager<PgConnection>>) {
    let start_time = SystemTime::now();
    let mut synced = false;
    let mut checkpoint_hash = hex::decode(checkpoint_hash.as_bytes()).unwrap();

    while running.load(Ordering::Relaxed) {
        debug!("Getting virtual chain from start_hash {}", hex::encode(checkpoint_hash.clone()));
        let response = with_retry(|| kaspad_client.get_virtual_chain_from_block(kaspa_hashes::Hash::from_slice(checkpoint_hash.as_slice()), true)).await
            .expect("Error when invoking GetVirtualChainFromBlock");

        let last_accepted_block_hash = response.accepted_transaction_ids.last().map(|ta| ta.accepting_block_hash.clone());

        let mut last_accepted_block_time: Option<u64> = None;
        if let Some(block_hash) = last_accepted_block_hash.clone() {
            last_accepted_block_time = Some(with_retry(|| kaspad_client.get_block(block_hash, false)).await
                .expect("Error when invoking GetBlock")
                .header.timestamp);
        }

        let con = &mut db_pool.get().expect("Database connection FAILED");
        con.transaction(|con| {
            // We need to do all the chain processing in a single transaction to avoid an incomplete state if the process is killed
            update_transactions(response.removed_chain_block_hashes.clone(), response.accepted_transaction_ids, last_accepted_block_time.clone(), con);
            update_chain_blocks(response.added_chain_block_hashes, response.removed_chain_block_hashes, con);
            if let Some(new_checkpoint_hash) = last_accepted_block_hash {
                checkpoint_hash = new_checkpoint_hash.as_bytes().to_vec();
                let checkpoint = hex::encode(checkpoint_hash.clone());
                debug!("Saving virtual_checkpoint {}", checkpoint);
                save_virtual_checkpoint(checkpoint, con);
            }
            Ok::<_, Error>(())
        }).expect("Commit virtual chain FAILED");

        if !synced {
            let time_to_sync = SystemTime::now().duration_since(start_time).unwrap();
            info!("\x1b[32mVirtual chain processor synced! (in {}:{:0>2}:{:0>2}s)\x1b[0m", time_to_sync.as_secs() / 3600, time_to_sync.as_secs() % 3600 / 60, time_to_sync.as_secs() % 60);
            synced = true;
        }

        sleep(Duration::from_secs(3)).await;
    }
}
