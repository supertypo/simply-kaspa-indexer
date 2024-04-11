extern crate diesel;

use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use crossbeam_queue::ArrayQueue;
use diesel::{Connection, ExpressionMethods, insert_into, PgConnection, QueryDsl, RunQueryDsl};
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::result::Error;
use diesel::upsert::excluded;
use kaspa_rpc_core::api::rpc::RpcApi;
use kaspa_wrpc_client::KaspaRpcClient;
use log::{info, trace, warn};
use tokio::time::sleep;

use crate::database::models::Transaction;
use crate::database::schema::{blocks, transactions};
use crate::vars::vars::save_virtual_start_hash;

pub async fn fetch_virtual_chains(start_hash: String,
                                  synced_queue: Arc<ArrayQueue<bool>>,
                                  kaspad_client: KaspaRpcClient,
                                  db_pool: Pool<ConnectionManager<PgConnection>>) -> Result<(), ()> {
    const INSERT_QUEUE_SIZE: usize = 7500;
    info!("start_block_hash={}", start_hash);
    let mut start_hash = hex::decode(start_hash.as_bytes()).unwrap();
    let mut rows_affected = 0;
    let mut start_hash_last_saved = SystemTime::now();

    while synced_queue.is_empty() || !synced_queue.pop().unwrap() {
        warn!("Not synced yet, sleeping for 5 seconds...");
        sleep(Duration::from_secs(5)).await;
    }
    loop {
        info!("Getting virtual chain from block={}", hex::encode(start_hash.clone()));
        let response = kaspad_client.get_virtual_chain_from_block(kaspa_hashes::Hash::from_slice(start_hash.as_slice()), true).await
            .expect("Error when invoking GetBlocks");
        info!("Received {} accepted transactions", response.accepted_transaction_ids.len());
        trace!("Accepted transactions: \n{:#?}", response.accepted_transaction_ids);

        let chain_blocks = response.accepted_transaction_ids.iter()
            .map(|cb| cb.accepting_block_hash.as_bytes().to_vec())
            .collect::<Vec<_>>();
        trace!("Chain blocks: \n{:#?}", &chain_blocks);

        let con = &mut db_pool.get().expect("Database connection FAILED");
        let chain_blocks_in_db: HashSet<Vec<u8>> = HashSet::from_iter(blocks::dsl::blocks
            .select(blocks::hash)
            .filter(blocks::hash.eq_any(chain_blocks))
            .load::<Vec<u8>>(con).unwrap());
        info!("Found {} chain blocks in database", chain_blocks_in_db.len());
        trace!("Chain blocks in database: \n{:#?}", &chain_blocks_in_db);

        let mut accepted_queue = vec![];
        for accepted_transaction in response.accepted_transaction_ids {
            let accepting_block_hash = accepted_transaction.accepting_block_hash.as_bytes().to_vec();
            if !chain_blocks_in_db.contains(&accepting_block_hash) {
                break;
            }
            for accepted_transaction_id in accepted_transaction.accepted_transaction_ids {
                accepted_queue.push(Transaction {
                    transaction_id: accepted_transaction_id.as_bytes().to_vec(),
                    subnetwork: None,
                    hash: None,
                    mass: None,
                    block_time: None,
                    block_hash: vec![],
                    is_accepted: true,
                    accepting_block_hash: Some(accepting_block_hash.clone()),
                })
            }
        }
        if !accepted_queue.is_empty() {
            for accepted_chunk in accepted_queue.chunks(INSERT_QUEUE_SIZE) {
                con.transaction(|con| {
                    rows_affected = insert_into(transactions::dsl::transactions)
                        .values(Vec::from_iter(accepted_chunk))
                        .on_conflict(transactions::transaction_id)
                        .do_update()
                        .set((
                            transactions::is_accepted.eq(excluded(transactions::is_accepted)),
                            transactions::accepting_block_hash.eq(excluded(transactions::accepting_block_hash)),
                        ))
                        .execute(con)
                        .expect("Commit transactions to database FAILED");
                    Ok::<_, Error>(())
                }).expect("Commit transactions to database FAILED");
                info!("Committed {} accepted transactions to database", rows_affected);
            }
            start_hash = accepted_queue.last().unwrap().accepting_block_hash.clone().unwrap();
            if SystemTime::now().duration_since(start_hash_last_saved).unwrap().as_secs() > 60 {
                save_virtual_start_hash(hex::encode(start_hash.clone()), db_pool.clone());
                start_hash_last_saved = SystemTime::now();
            }
        }
        sleep(Duration::from_secs(5)).await;
    }
}
