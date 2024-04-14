extern crate diesel;

use std::collections::HashSet;

use diesel::{Connection, ExpressionMethods, insert_into, PgConnection, QueryDsl, RunQueryDsl};
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::result::Error;
use diesel::upsert::excluded;
use kaspa_rpc_core::RpcAcceptedTransactionIds;
use log::{debug, info, trace};

use crate::database::models::Transaction;
use crate::database::schema::transactions;

pub fn accept_transactions(accepted_transaction_ids: Vec<RpcAcceptedTransactionIds>, db_pool: Pool<ConnectionManager<PgConnection>>) -> Option<Vec<u8>> {
    const INSERT_QUEUE_SIZE: usize = 7500;
    info!("Received {} accepted transactions", accepted_transaction_ids.len());
    trace!("Accepted transactions: \n{:#?}", accepted_transaction_ids);

    let mut accepted_queue = vec![];
    for accepted_id in accepted_transaction_ids {
        for transaction_id in accepted_id.accepted_transaction_ids {
            accepted_queue.push(Transaction {
                transaction_id: transaction_id.as_bytes().to_vec(),
                subnetwork: None,
                hash: None,
                mass: None,
                block_time: None,
                is_accepted: true,
                accepting_block_hash: Some(accepted_id.accepting_block_hash.as_bytes().to_vec()),
            })
        }
    }
    if !accepted_queue.is_empty() {
        let con = &mut db_pool.get().expect("Database connection FAILED");
        for accepted_chunk in accepted_queue.chunks(INSERT_QUEUE_SIZE) {
            let mut rows_affected = 0;
            con.transaction(|con| {
                let mut accepted_set: HashSet<&Transaction> = HashSet::from_iter(accepted_chunk.iter());
                debug!("Processing {} accepted transactions", accepted_set.len());
                // Find existing identical transactions and remove them from the insert queue
                transactions::dsl::transactions
                    .filter(transactions::transaction_id.eq_any(accepted_set.iter()
                        .map(|t| t.transaction_id.clone()).collect::<Vec<Vec<u8>>>()))
                    .load::<Transaction>(con)
                    .expect("Select accepted transactions from database FAILED").iter()
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
                    .expect("Commit accepted transactions to database FAILED");
                Ok::<_, Error>(())
            }).expect("Commit accepted transactions to database FAILED");
            info!("Committed {} accepted transactions to database", rows_affected);
        }
        return accepted_queue.last().map(|t| t.accepting_block_hash.clone().unwrap());
    }
    None
}
