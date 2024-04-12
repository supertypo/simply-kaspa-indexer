extern crate diesel;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use bigdecimal::ToPrimitive;

use crossbeam_queue::ArrayQueue;
use diesel::{insert_into, PgConnection, RunQueryDsl};
use diesel::r2d2::{ConnectionManager, Pool};
use kaspa_rpc_core::RpcTransaction;
use log::{debug, info, trace, warn};
use tokio::time::sleep;

use crate::database::models::{BlockTransaction, Subnetwork, SubnetworkInsertable, Transaction, TransactionInput, TransactionOutput};
use crate::database::schema::subnetworks;

pub async fn process_transactions(rpc_transactions_queue: Arc<ArrayQueue<Vec<RpcTransaction>>>,
                                  db_transactions_queue: Arc<ArrayQueue<(Transaction, BlockTransaction)>>,
                                  db_transactions_inputs_queue: Arc<ArrayQueue<TransactionInput>>,
                                  db_transactions_outputs_queue: Arc<ArrayQueue<TransactionOutput>>,
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
                subnetwork: Some(subnetwork_map.get(&t.subnetwork_id.to_string()).unwrap().clone()),
                hash: Some(verbose_data.hash.as_bytes().to_vec()),
                mass: Some(verbose_data.mass as i32),
                block_time: Some((verbose_data.block_time / 1000) as i32),
                is_accepted: false,
                accepting_block_hash: None,
            };
            let db_block_transaction = BlockTransaction {
                block_hash: verbose_data.block_hash.as_bytes().to_vec(),
                transaction_id: verbose_data.transaction_id.as_bytes().to_vec()
            };
            for (i, input) in t.inputs.iter().enumerate() {
                let db_transaction_input = TransactionInput {
                    transaction_id: verbose_data.transaction_id.as_bytes().to_vec(),
                    index: i as i16,
                    previous_outpoint_hash: input.previous_outpoint.transaction_id.as_bytes().to_vec(),
                    previous_outpoint_index: input.previous_outpoint.index.to_i16().unwrap(),
                    signature_script: input.signature_script.clone(),
                    sig_op_count: input.sig_op_count.to_i16().unwrap(),
                };
                while db_transactions_inputs_queue.is_full() {
                    warn!("DB transactions_inputs queue is full");
                    sleep(Duration::from_secs(2)).await;
                }
                let _ = db_transactions_inputs_queue.push(db_transaction_input);
            }
            for (i, output) in t.outputs.iter().enumerate() {
                let db_transaction_output = TransactionOutput {
                    transaction_id: verbose_data.transaction_id.as_bytes().to_vec(),
                    index: i as i16,
                    amount: output.value as i64,
                    script_public_key: output.script_public_key.script().to_vec(),
                    script_public_key_address: output.verbose_data.clone().unwrap().script_public_key_address.to_string(),
                    script_public_key_type: output.verbose_data.clone().unwrap().script_public_key_type.to_string()
                };
                while db_transactions_outputs_queue.is_full() {
                    warn!("DB transactions_inputs queue is full");
                    sleep(Duration::from_secs(2)).await;
                }
                let _ = db_transactions_outputs_queue.push(db_transaction_output);
            }
            while db_transactions_queue.is_full() {
                warn!("DB transactions queue is full");
                sleep(Duration::from_secs(2)).await;
            }
            let _ = db_transactions_queue.push((db_transaction, db_block_transaction));
        }
    }
}
