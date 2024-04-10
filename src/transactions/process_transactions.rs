extern crate diesel;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use crossbeam_queue::ArrayQueue;
use diesel::{insert_into, PgConnection, RunQueryDsl};
use diesel::r2d2::{ConnectionManager, Pool};
use kaspa_rpc_core::RpcTransaction;
use log::{debug, info, trace, warn};
use tokio::time::sleep;

use crate::database::models::{Subnetwork, SubnetworkInsertable, Transaction};
use crate::database::schema::subnetworks;

pub async fn process_transactions(rpc_transactions_queue: Arc<ArrayQueue<Vec<RpcTransaction>>>,
                                  db_transactions_queue: Arc<ArrayQueue<Transaction>>,
                                  db_pool: Pool<ConnectionManager<PgConnection>>) -> Result<(), ()> {
    let mut subnetwork_map: HashMap<String, i32> = HashMap::new();
    let con = &mut db_pool.get().expect("Database connection FAILED");
    let results: Vec<Subnetwork> = subnetworks::dsl::subnetworks.load::<Subnetwork>(con).unwrap();
    for s in results {
        subnetwork_map.insert(s.subnetwork_id, s.id);
    }
    info!("Loaded {} known subnetworks", subnetwork_map.len());

    loop {
        let transactions_option = rpc_transactions_queue.pop();
        if transactions_option.is_none() {
            trace!("RPC transactions queue is empty");
            sleep(Duration::from_secs(1)).await;
            continue;
        }
        let transactions = transactions_option.unwrap();
        for t in transactions {
            let verbose_data = t.verbose_data.unwrap();
            let subnetwork_id = t.subnetwork_id.to_string();
            let subnetwork_option = subnetwork_map.get(&subnetwork_id);
            if subnetwork_option.is_none() {
                let id = insert_into(subnetworks::dsl::subnetworks)
                    .values(SubnetworkInsertable { subnetwork_id: subnetwork_id.clone() })
                    .returning(subnetworks::id)
                    .get_results(&mut db_pool.get().expect("Database connection FAILED"))
                    .expect("Commit transactions to database FAILED")[0];
                subnetwork_map.insert(subnetwork_id.clone(), id);
                debug!("Inserted new subnetwork, id: {} subnetwork_id: {}", id, subnetwork_id)
            }
            let db_transaction = Transaction {
                transaction_id: verbose_data.transaction_id.as_bytes().to_vec(),
                subnetwork: subnetwork_map.get(&t.subnetwork_id.to_string()).unwrap().clone(),
                hash: verbose_data.hash.as_bytes().to_vec(),
                mass: verbose_data.mass as i32,
                block_hash: vec![Some(verbose_data.block_hash.as_bytes().to_vec())],
                block_time: (verbose_data.block_time / 1000) as i32,
                is_accepted: false,
                accepting_block_hash: None,
            };
            while db_transactions_queue.is_full() {
                warn!("DB transactions queue is full, sleeping 2 seconds...");
                sleep(Duration::from_millis(100)).await;
            }
            let _ = db_transactions_queue.push(db_transaction);
        }
    }
}
