extern crate diesel;

use std::sync::Arc;
use std::time::Duration;

use crossbeam_queue::ArrayQueue;
use kaspa_rpc_core::RpcTransaction;
use log::{debug, warn};
use tokio::time::sleep;

use crate::database::models::Transaction;

pub async fn process_transactions(rpc_transactions_queue: Arc<ArrayQueue<Vec<RpcTransaction>>>,
                              db_transactions_queue: Arc<ArrayQueue<Transaction>>) -> Result<(), ()> {
    loop {
        let transactions_option = rpc_transactions_queue.pop();
        if transactions_option.is_none() {
            debug!("RPC transactions queue is empty, sleeping 2 seconds...");
            sleep(Duration::from_secs(2)).await;
            continue;
        }
        let transactions = transactions_option.unwrap();
        for t in transactions {
            let verbose_data = t.verbose_data.unwrap();
            let db_transaction = Transaction {
                transaction_id: verbose_data.transaction_id.as_bytes().to_vec(),
                subnetwork_id: t.subnetwork_id.to_string().as_bytes().to_vec(),
                hash: verbose_data.hash.as_bytes().to_vec(),
                mass: verbose_data.mass as i32,
                block_hash: vec![Some(verbose_data.block_hash.as_bytes().to_vec())],
                block_time: (verbose_data.block_time / 1000) as i32,
                is_accepted: false,
                accepting_block_hash: None,
            };
            while db_transactions_queue.is_full() {
                warn!("DB transactions queue is full, sleeping 2 seconds...");
                sleep(Duration::from_secs(2)).await;
            }
            let _ = db_transactions_queue.push(db_transaction);
        }
    }
}
