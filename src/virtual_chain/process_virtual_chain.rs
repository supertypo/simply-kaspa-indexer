extern crate diesel;

use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use crossbeam_queue::ArrayQueue;
use diesel::{BoolExpressionMethods, Connection, ExpressionMethods, insert_into, PgConnection, QueryDsl, RunQueryDsl};
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::result::Error;
use diesel::upsert::excluded;
use kaspa_rpc_core::api::rpc::RpcApi;
use kaspa_wrpc_client::KaspaRpcClient;
use log::{info, trace, warn};
use tokio::time::sleep;

use crate::database::models::{Block, Transaction};
use crate::database::schema::{transactions, blocks};
use crate::vars::vars::save_virtual_checkpoint;

pub async fn process_virtual_chain(checkpoint_hash: String,
                                   synced_queue: Arc<ArrayQueue<bool>>,
                                   kaspad_client: KaspaRpcClient,
                                   db_pool: Pool<ConnectionManager<PgConnection>>) -> Result<(), ()> {
    const INSERT_QUEUE_SIZE: usize = 7500;
    info!("virtual checkpoint_hash={}", checkpoint_hash);
    let mut checkpoint_hash = hex::decode(checkpoint_hash.as_bytes()).unwrap();
    let mut checkpoint_hash_last_saved = SystemTime::now();
    let mut rows_affected = 0;

    while synced_queue.is_empty() || !synced_queue.pop().unwrap() {
        warn!("Not synced yet, sleeping for 5 seconds...");
        sleep(Duration::from_secs(5)).await;
    }
    loop {
        info!("Getting virtual chain from start_hash={}", hex::encode(checkpoint_hash.clone()));
        let response = kaspad_client.get_virtual_chain_from_block(kaspa_hashes::Hash::from_slice(checkpoint_hash.as_slice()), true).await
            .expect("Error when invoking GetBlocks");
        info!("Received {} accepted transactions", response.accepted_transaction_ids.len());
        trace!("Accepted transactions: \n{:#?}", response.accepted_transaction_ids);

        let mut accepted_queue = vec![];
        for accepted_transaction in response.accepted_transaction_ids {
            for accepted_transaction_id in accepted_transaction.accepted_transaction_ids {
                accepted_queue.push(Transaction {
                    transaction_id: accepted_transaction_id.as_bytes().to_vec(),
                    subnetwork: None,
                    hash: None,
                    mass: None,
                    block_time: None,
                    is_accepted: true,
                    accepting_block_hash: Some(accepted_transaction.accepting_block_hash.as_bytes().to_vec()),
                })
            }
        }
        let con = &mut db_pool.get().expect("Database connection FAILED");
        if !accepted_queue.is_empty() {
            for accepted_chunk in accepted_queue.chunks(INSERT_QUEUE_SIZE) {
                con.transaction(|con| {
                    let mut accepted_set: HashSet<&Transaction> = HashSet::from_iter(accepted_chunk.iter());
                    // Find existing identical transactions and remove them from the insert queue
                    transactions::dsl::transactions
                        .filter(transactions::transaction_id.eq_any(accepted_set.iter()
                            .map(|t| t.transaction_id.clone()).collect::<Vec<Vec<u8>>>()))
                        .load::<Transaction>(con)
                        .unwrap().iter()
                        .for_each(|t| {
                            let new_tx = accepted_set.get(t).unwrap();
                            if new_tx.is_accepted == t.is_accepted &&
                                new_tx.accepting_block_hash == t.accepting_block_hash {
                                accepted_set.remove(t);
                            }
                        });
                    //Upsert transactions in case a conflicting tx was persisted
                    rows_affected = insert_into(transactions::dsl::transactions)
                        .values(Vec::from_iter(accepted_set))
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
        }

        if !response.added_chain_block_hashes.is_empty() {
            info!("Received {} added chain blocks", response.added_chain_block_hashes.len());
            trace!("Added chain blocks: \n{:#?}", response.added_chain_block_hashes);
            let mut chain_hashes = response.added_chain_block_hashes.iter().map(|b| b.as_bytes().to_vec()).collect::<HashSet<Vec<u8>>>();
            con.transaction(|con| {
                // Find existing chain blocks and remove them from the insert queue
                blocks::dsl::blocks
                    .select(blocks::hash)
                    .filter(blocks::hash.eq_any(&chain_hashes)
                        .and(blocks::is_chain_block.eq(true)))
                    .load::<Vec<u8>>(con)
                    .expect("Commit transactions to database FAILED").iter()
                    .for_each(|b| {
                        chain_hashes.remove(b);
                    });
                //Upsert blocks in case a conflicting block was persisted
                rows_affected = insert_into(blocks::dsl::blocks)
                    .values(chain_hashes.iter().map(|h| Block::new(h.clone(), true)).collect::<Vec<Block>>())
                    .on_conflict(blocks::hash)
                    .do_update()
                    .set(blocks::is_chain_block.eq(true))
                    .execute(con)
                    .expect("Commit added chain blocks to database FAILED");
                Ok::<_, Error>(())
            }).expect("Commit added chain blocks to database FAILED");
            info!("Committed {} added chain blocks to database", rows_affected);
        }

        // Save checkpoint as the last accepting_block_hash
        if !accepted_queue.is_empty() && SystemTime::now().duration_since(checkpoint_hash_last_saved).unwrap().as_secs() > 60 {
            checkpoint_hash = accepted_queue.last().unwrap().accepting_block_hash.clone().unwrap();
            save_virtual_checkpoint(hex::encode(checkpoint_hash.clone()), db_pool.clone());
            checkpoint_hash_last_saved = SystemTime::now();
        }
        sleep(Duration::from_secs(1)).await;
    }
}
