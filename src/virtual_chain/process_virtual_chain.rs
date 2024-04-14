extern crate diesel;

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use crossbeam_queue::ArrayQueue;
use diesel::{Connection, ExpressionMethods, insert_into, PgConnection, QueryDsl, RunQueryDsl};
use diesel::r2d2::{ConnectionManager, Pool, PooledConnection};
use diesel::result::Error;
use diesel::upsert::excluded;
use kaspa_rpc_core::api::rpc::RpcApi;
use kaspa_rpc_core::RpcHash;
use kaspa_wrpc_client::KaspaRpcClient;
use log::{info, trace, warn};
use tokio::time::sleep;

use crate::database::models::{Block, Transaction};
use crate::database::schema::{blocks, transactions};
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

        update_chain_blocks(response.added_chain_block_hashes, response.removed_chain_block_hashes, con);

        // Save checkpoint as the last accepting_block_hash
        if !accepted_queue.is_empty() && SystemTime::now().duration_since(checkpoint_hash_last_saved).unwrap().as_secs() > 60 {
            checkpoint_hash = accepted_queue.last().unwrap().accepting_block_hash.clone().unwrap();
            save_virtual_checkpoint(hex::encode(checkpoint_hash.clone()), db_pool.clone());
            checkpoint_hash_last_saved = SystemTime::now();
        }
        sleep(Duration::from_secs(1)).await;
    }
}

fn update_chain_blocks(added_hashes: Vec<RpcHash>, removed_hashes: Vec<RpcHash>, con: &mut PooledConnection<ConnectionManager<PgConnection>>) {
    info!("Received {} added chain blocks", added_hashes.len());
    trace!("Added chain blocks: \n{:#?}", added_hashes);
    info!("Received {} removed chain blocks", removed_hashes.len());
    trace!("Removed chain blocks: \n{:#?}", removed_hashes);
    let mut rows_affected = 0;
    let mut updated_chain_blocks = HashMap::with_capacity(added_hashes.len() + removed_hashes.len());

    // Join added and removed blocks to only require one exists and one insert query
    for added_hash in added_hashes {
        updated_chain_blocks.insert(added_hash.as_bytes().to_vec(), true);
    }
    for removed_hash in removed_hashes {
        updated_chain_blocks.insert(removed_hash.as_bytes().to_vec(), false);
    }
    if !updated_chain_blocks.is_empty() {
        con.transaction(|con| {
            // Find existing and remove them from the insert queue
            blocks::dsl::blocks
                .select((blocks::hash, blocks::is_chain_block))
                .filter(blocks::hash.eq_any(updated_chain_blocks.keys()))
                .load::<(Vec<u8>, bool)>(con)
                .expect("Select chain blocks from database FAILED").iter()
                .for_each(|(b, is_cb)| {
                    if updated_chain_blocks.get(b).unwrap() == is_cb {
                        updated_chain_blocks.remove(b); // is_chain_block is already set to the same value in db
                    }
                });
            // Save changed rows
            rows_affected = insert_into(blocks::dsl::blocks)
                .values(updated_chain_blocks.iter().map(|(h, is_cb)| Block::new(h.clone(), *is_cb)).collect::<Vec<Block>>())
                .on_conflict(blocks::hash)
                .do_update()
                .set(blocks::is_chain_block.eq(excluded(blocks::is_chain_block))) // Upsert guarantees consistency when rows where inserted since the select
                .execute(con)
                .expect("Commit updated chain blocks to database FAILED");
            Ok::<_, Error>(())
        }).expect("Commit updated chain blocks to database FAILED");
    }
    info!("Committed {} updated chain blocks to database", rows_affected);
}
