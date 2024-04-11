extern crate diesel;

use std::collections::HashSet;
use std::time::{Duration, SystemTime};
use diesel::{Connection, ExpressionMethods, insert_into, PgConnection, QueryDsl, RunQueryDsl, select};
use diesel::dsl::sql;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::result::Error;
use diesel::upsert::excluded;
use kaspa_rpc_core::api::rpc::RpcApi;
use kaspa_rpc_core::RpcHash;
use kaspa_wrpc_client::KaspaRpcClient;
use log::{debug, info, trace};
use tokio::time::sleep;
use crate::database::models::{Subnetwork, Transaction};
use crate::database::schema::{blocks, subnetworks, transactions};
use crate::vars::vars::save_start_block_hash;

pub async fn fetch_virtual_chains(start_block_hash: String,
                                  kaspad_client: KaspaRpcClient,
                                  db_pool: Pool<ConnectionManager<PgConnection>>) -> Result<(), ()> {
    const INSERT_QUEUE_SIZE: usize = 7500;
    info!("start_block_hash={}", start_block_hash);
    let start_hash = kaspa_hashes::Hash::from_slice(hex::decode(start_block_hash.as_bytes()).unwrap().as_slice());
    let mut last_block_timestamp = 0;
    let mut last_commit_time = SystemTime::now();
    let mut rows_affected = 0;
    loop {
        let response = kaspad_client.get_virtual_chain_from_block(start_hash, true).await
            .expect("Error when invoking GetBlocks");
        info!("Received {} accepted transactions", response.accepted_transaction_ids.len());
        trace!("Accepted transactions: \n{:#?}", response.accepted_transaction_ids);

        let chain_blocks = response.accepted_transaction_ids.iter()
            .map(|cb| cb.accepting_block_hash.as_bytes().to_vec())
            .collect::<Vec<_>>();
        info!("Received {} chain blocks", chain_blocks.len());
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
            if !chain_blocks_in_db.contains(&accepting_block_hash) || accepted_queue.len() >= INSERT_QUEUE_SIZE {
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
        if accepted_queue.first().is_none() { // Should not happen, but just in case it does...
            sleep(Duration::from_secs(2)).await;
            continue;
        }
        let last_known_chain_block = accepted_queue.last().unwrap().accepting_block_hash.clone().unwrap();

        if accepted_queue.len() >= 1 && SystemTime::now().duration_since(last_commit_time).unwrap().as_secs() > 2 {
            con.transaction(|con| {
                rows_affected = insert_into(transactions::dsl::transactions)
                    .values(Vec::from_iter(&accepted_queue))
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
            info!("Committed {} new/updated transactions to database. Last block timestamp: {}", rows_affected,
                chrono::DateTime::from_timestamp_millis(last_block_timestamp as i64 * 1000).unwrap());

            save_start_block_hash(hex::encode(last_known_chain_block), db_pool.clone());

            if accepted_queue.len() >= INSERT_QUEUE_SIZE { // FIXME: Use last block time to increase wait
                sleep(Duration::from_secs(5)).await;
            } else {
                sleep(Duration::from_secs(2)).await;
            }
            debug!("Fetch virtual chain BPS: {}", 1000 * accepted_queue.len() as u128
                / SystemTime::now().duration_since(last_commit_time).unwrap().as_millis());
            last_commit_time = SystemTime::now();
        }
    }
}
