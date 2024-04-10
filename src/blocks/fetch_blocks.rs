extern crate diesel;

use std::sync::Arc;
use std::time::{Duration, SystemTime};

use crossbeam_queue::ArrayQueue;
use kaspa_rpc_core::{RpcBlock, RpcTransaction};
use kaspa_rpc_core::api::rpc::RpcApi;
use kaspa_wrpc_client::KaspaRpcClient;
use log::{debug, trace, warn};
use log::info;
use tokio::time::sleep;

pub async fn fetch_blocks(start_block_hash: String,
                          kaspad_client: KaspaRpcClient, rpc_blocks_queue: Arc<ArrayQueue<RpcBlock>>,
                          rpc_transactions_queue: Arc<ArrayQueue<Vec<RpcTransaction>>>) -> Result<(), ()> {
    info!("start_block_hash={}", start_block_hash);
    let start_hash = kaspa_hashes::Hash::from_slice(hex::decode(start_block_hash.as_bytes()).unwrap().as_slice());
    let mut low_hash = start_hash;

    loop {
        let start_time = SystemTime::now();
        info!("Getting blocks with low_hash={}", low_hash);
        let res = kaspad_client.get_blocks(Some(low_hash), true, true).await
            .expect("Error when invoking GetBlocks");
        info!("Received {} blocks", res.blocks.len());
        trace!("Block hashes: \n{:#?}", res.block_hashes);

        let blocks_len = res.blocks.len();
        if blocks_len > 1 {
            low_hash = res.blocks.last().unwrap().header.hash;
            for b in res.blocks {
                let block_hash = b.header.hash;
                if block_hash == low_hash && block_hash != start_hash {
                    trace!("Ignoring low_hash block {}", low_hash);
                    continue;
                }
                while rpc_blocks_queue.is_full() {
                    warn!("RPC blocks queue is full, sleeping 2 seconds...");
                    sleep(Duration::from_secs(2)).await;
                }
                while rpc_transactions_queue.is_full() {
                    warn!("RPC transactions queue is full, sleeping 2 seconds...");
                    sleep(Duration::from_secs(2)).await;
                }
                rpc_blocks_queue.push(RpcBlock { header: b.header, transactions: vec![], verbose_data: b.verbose_data }).unwrap();
                rpc_transactions_queue.push(b.transactions).unwrap();
            }
        }
        debug!("Fetch blocks BPS: {}", 1000 * blocks_len as u128
            / SystemTime::now().duration_since(start_time).unwrap().as_millis());
        if blocks_len < 50 && SystemTime::now().duration_since(start_time).unwrap().as_secs() < 3 {
            sleep(Duration::from_secs(2)).await;
        }
    }
}
