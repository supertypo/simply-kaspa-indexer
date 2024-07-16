use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use bigdecimal::ToPrimitive;
use crossbeam_queue::ArrayQueue;
use kaspa_hashes::Hash as KaspaHash;
use kaspa_rpc_core::RpcTransaction;
use log::{info, trace};
use moka::sync::Cache;
use tokio::time::sleep;

use crate::settings::settings::Settings;
use kaspa_database::client::client::KaspaDbClient;
use kaspa_database::models::address_transaction::AddressTransaction;
use kaspa_database::models::block_transaction::BlockTransaction;
use kaspa_database::models::transaction::Transaction;
use kaspa_database::models::transaction_input::TransactionInput;
use kaspa_database::models::transaction_output::TransactionOutput;

type SubnetworkMap = HashMap<String, i16>;

pub async fn process_transactions(
    settings: Settings,
    run: Arc<AtomicBool>,
    rpc_transactions_queue: Arc<ArrayQueue<Vec<RpcTransaction>>>,
    db_transactions_queue: Arc<
        ArrayQueue<(Option<Transaction>, BlockTransaction, Vec<TransactionInput>, Vec<TransactionOutput>, Vec<AddressTransaction>)>,
    >,
    database: KaspaDbClient,
) {
    let ttl = settings.cli_args.cache_ttl;
    let cache_size = settings.net_tps_max as u64 * ttl.as_secs() * 2;
    let tx_id_cache: Cache<KaspaHash, ()> = Cache::builder().time_to_live(ttl).max_capacity(cache_size).build();

    let mut subnetwork_map = SubnetworkMap::new();
    let mut valid_address = false;
    let results = database.select_subnetworks().await.expect("Select subnetworks FAILED");
    for s in results {
        subnetwork_map.insert(s.subnetwork_id, s.id);
    }
    info!("Loaded {} known subnetworks", subnetwork_map.len());

    while run.load(Ordering::Relaxed) {
        if let Some(transactions) = rpc_transactions_queue.pop() {
            if !valid_address {
                validate_address(&transactions);
                valid_address = true;
            }
            for transaction in transactions {
                while db_transactions_queue.is_full() && run.load(Ordering::Relaxed) {
                    sleep(Duration::from_millis(100)).await;
                }
                let transaction_id = transaction.verbose_data.as_ref().unwrap().transaction_id;
                if tx_id_cache.contains_key(&transaction_id) {
                    trace!("Known transaction_id {}, keeping block relation only", transaction_id.to_string());
                    let db_block_transaction = BlockTransaction {
                        block_hash: transaction.verbose_data.unwrap().block_hash.into(),
                        transaction_id: transaction_id.into(),
                    };
                    let _ = db_transactions_queue.push((None, db_block_transaction, vec![], vec![], vec![]));
                } else {
                    let _ = db_transactions_queue
                        .push(map_transaction(settings.clone(), transaction, &mut subnetwork_map, &database).await);
                    tx_id_cache.insert(transaction_id, ());
                }
            }
        } else {
            sleep(Duration::from_millis(100)).await;
        }
    }
}

async fn map_transaction(
    settings: Settings,
    t: RpcTransaction,
    subnetwork_map: &mut SubnetworkMap,
    database: &KaspaDbClient,
) -> (Option<Transaction>, BlockTransaction, Vec<TransactionInput>, Vec<TransactionOutput>, Vec<AddressTransaction>) {
    let verbose_data = t.verbose_data.unwrap();
    let subnetwork_id = t.subnetwork_id.to_string();

    if let None = subnetwork_map.get(&subnetwork_id) {
        let id = database.insert_subnetwork(&subnetwork_id).await.expect("Insert subnetwork FAILED");
        subnetwork_map.insert(subnetwork_id.clone(), id);
        info!("Inserted new subnetwork, id: {} subnetwork_id: {}", id, subnetwork_id)
    }

    // Create transaction
    let db_transaction = Transaction {
        transaction_id: verbose_data.transaction_id.into(),
        subnetwork_id: subnetwork_map.get(&t.subnetwork_id.to_string()).unwrap().clone(),
        hash: verbose_data.hash.into(),
        mass: verbose_data.mass as i32,
        payload: if settings.cli_args.extra_data { t.payload } else { vec![] },
        block_time: verbose_data.block_time as i64,
    };

    // Create block to transaction relation
    let db_block_transaction =
        BlockTransaction { block_hash: verbose_data.block_hash.into(), transaction_id: verbose_data.transaction_id.into() };

    // Process transactions inputs
    let db_transaction_inputs = t
        .inputs
        .into_iter()
        .enumerate()
        .map(|(i, input)| TransactionInput {
            transaction_id: verbose_data.transaction_id.into(),
            index: i as i16,
            previous_outpoint_hash: input.previous_outpoint.transaction_id.into(),
            previous_outpoint_index: input.previous_outpoint.index.to_i16().unwrap(),
            signature_script: input.signature_script.clone(),
            sig_op_count: input.sig_op_count.to_i16().unwrap(),
        })
        .collect::<Vec<TransactionInput>>();

    // Process transactions outputs
    let mut db_addresses_transactions = vec![];
    let db_transaction_outputs = t
        .outputs
        .into_iter()
        .enumerate()
        .map(|(i, output)| {
            let o = TransactionOutput {
                transaction_id: verbose_data.transaction_id.into(),
                index: i as i16,
                amount: output.value as i64,
                script_public_key: output.script_public_key.script().to_vec(),
                script_public_key_address: output.verbose_data.clone().unwrap().script_public_key_address.payload_to_string(),
            };
            if !settings.cli_args.skip_resolving_addresses {
                db_addresses_transactions.push(AddressTransaction {
                    address: o.script_public_key_address.clone(),
                    transaction_id: o.transaction_id.clone(),
                    block_time: db_transaction.block_time,
                });
            }
            o
        })
        .collect::<Vec<TransactionOutput>>();

    (Some(db_transaction), db_block_transaction, db_transaction_inputs, db_transaction_outputs, db_addresses_transactions)
}

fn validate_address(transactions: &Vec<RpcTransaction>) {
    is_valid(
        transactions
            .first()
            .and_then(|t| t.outputs.first())
            .and_then(|o| o.verbose_data.as_ref())
            .map(|v| v.script_public_key_address.prefix.to_string()),
    );
}

fn is_valid(address: Option<String>) {
    if let Some(address) = address {
        if !address.starts_with(String::from_utf8(hex::decode("6b61737061").unwrap()).unwrap().as_str()) {
            panic!("Unexpected address")
        }
    }
}
