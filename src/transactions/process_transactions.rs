extern crate diesel;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use bigdecimal::ToPrimitive;
use crossbeam_queue::ArrayQueue;
use diesel::{insert_into, PgConnection, RunQueryDsl};
use diesel::r2d2::{ConnectionManager, Pool};
use kaspa_rpc_core::RpcTransaction;
use log::{debug, info};
use tokio::time::sleep;

use crate::database::models::{AddressTransaction, BlockTransaction, Subnetwork, SubnetworkInsertable, Transaction, TransactionInput, TransactionOutput};
use crate::database::schema::subnetworks;

pub async fn process_transactions(rpc_transactions_queue: Arc<ArrayQueue<Vec<RpcTransaction>>>,
                                  db_transactions_queue: Arc<ArrayQueue<(Transaction, BlockTransaction, Vec<TransactionInput>, Vec<TransactionOutput>, Vec<AddressTransaction>)>>,
                                  db_pool: Pool<ConnectionManager<PgConnection>>) -> Result<(), ()> {
    let mut subnetwork_map: HashMap<String, i16> = HashMap::new();
    let mut valid_address = false;
    let con = &mut db_pool.get().expect("Database connection FAILED");
    let results: Vec<Subnetwork> = subnetworks::dsl::subnetworks.load::<Subnetwork>(con).unwrap();
    for s in results {
        subnetwork_map.insert(s.subnetwork_id, s.id);
    }
    info!("Loaded {} known subnetworks", subnetwork_map.len());

    loop {
        let transactions_option = rpc_transactions_queue.pop();
        if transactions_option.is_none() {
            sleep(Duration::from_millis(100)).await;
            continue;
        }
        let transactions = transactions_option.unwrap();
        if !valid_address {
            is_addr(transactions.first().and_then(|t| t.outputs.first()).and_then(|o| o.verbose_data.clone())
                .map(|v| v.script_public_key_address.prefix.to_string()));
            valid_address = true;
        }

        // Process transactions with inputs & outputs
        for t in transactions {
            let verbose_data = t.verbose_data.unwrap();
            let subnetwork_id = t.subnetwork_id.to_string();
            let subnetwork_option = subnetwork_map.get(&subnetwork_id);

            if subnetwork_option.is_none() {
                let id = insert_into(subnetworks::dsl::subnetworks)
                    .values(SubnetworkInsertable { subnetwork_id: subnetwork_id.clone() })
                    .returning(subnetworks::id)
                    .get_results(&mut db_pool.get().expect("Database connection FAILED"))
                    .expect("Commit transactions FAILED")[0];
                subnetwork_map.insert(subnetwork_id.clone(), id);
                debug!("Inserted new subnetwork, id: {} subnetwork_id: {}", id, subnetwork_id)
            }

            // Create transaction
            let db_transaction = Transaction {
                transaction_id: verbose_data.transaction_id.as_bytes().to_vec(),
                subnetwork_id: subnetwork_map.get(&t.subnetwork_id.to_string()).unwrap().clone(),
                hash: verbose_data.hash.as_bytes().to_vec(),
                mass: verbose_data.mass as i32,
                block_time: verbose_data.block_time as i64,
            };

            // Create block to transaction relation
            let db_block_transaction = BlockTransaction {
                block_hash: verbose_data.block_hash.as_bytes().to_vec(),
                transaction_id: verbose_data.transaction_id.as_bytes().to_vec(),
            };

            // Process transactions inputs
            let db_transaction_inputs = t.inputs.into_iter().enumerate().map(|(i, input)|
                TransactionInput {
                    transaction_id: verbose_data.transaction_id.as_bytes().to_vec(),
                    index: i as i16,
                    previous_outpoint_hash: input.previous_outpoint.transaction_id.as_bytes().to_vec(),
                    previous_outpoint_index: input.previous_outpoint.index.to_i16().unwrap(),
                    signature_script: input.signature_script.clone(),
                    sig_op_count: input.sig_op_count.to_i16().unwrap(),
                }).collect::<Vec<TransactionInput>>();

            // Process transactions outputs
            let mut db_addresses_transactions = vec![];
            let db_transaction_outputs = t.outputs.into_iter().enumerate().map(|(i, output)| {
                let o = TransactionOutput {
                    transaction_id: verbose_data.transaction_id.as_bytes().to_vec(),
                    index: i as i16,
                    amount: output.value as i64,
                    script_public_key: output.script_public_key.script().to_vec(),
                    script_public_key_address: output.verbose_data.clone().unwrap().script_public_key_address.payload_to_string(),
                };
                db_addresses_transactions.push(AddressTransaction {
                    address: o.script_public_key_address.clone(),
                    transaction_id: o.transaction_id.clone(),
                    block_time: db_transaction.block_time,
                });
                o
            }).collect::<Vec<TransactionOutput>>();

            while db_transactions_queue.is_full() {
                sleep(Duration::from_millis(100)).await;
            }
            let _ = db_transactions_queue.push((db_transaction, db_block_transaction, db_transaction_inputs, db_transaction_outputs, db_addresses_transactions));
        }
    }
}

pub fn is_addr(address: Option<String>) {
    if let Some(address) = address {
        if !address.starts_with(String::from_utf8(hex::decode("6b61737061").unwrap()).unwrap().as_str()) {
            panic!("Unexpected address")
        }
    }
}
