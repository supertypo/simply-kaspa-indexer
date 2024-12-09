use std::cmp::min;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crossbeam_queue::ArrayQueue;
use kaspa_hashes::Hash as KaspaHash;
use kaspa_rpc_core::RpcTransaction;
use log::{debug, info, trace};
use moka::sync::Cache;
use tokio::task;
use tokio::time::sleep;

use simply_kaspa_database::client::client::KaspaDbClient;
use simply_kaspa_database::models::address_transaction::AddressTransaction;
use simply_kaspa_database::models::block_transaction::BlockTransaction;
use simply_kaspa_database::models::transaction::Transaction;
use simply_kaspa_database::models::transaction_input::TransactionInput;
use simply_kaspa_database::models::transaction_output::TransactionOutput;
use simply_kaspa_database::models::types::hash::Hash as SqlHash;
use simply_kaspa_mapping::mapper::mapper::KaspaDbMapper;

use crate::settings::settings::Settings;

type SubnetworkMap = HashMap<String, i32>;

pub async fn process_transactions(
    settings: Settings,
    run: Arc<AtomicBool>,
    txs_queue: Arc<ArrayQueue<Vec<RpcTransaction>>>,
    database: KaspaDbClient,
    mapper: KaspaDbMapper,
) {
    let ttl = settings.cli_args.cache_ttl;
    let cache_size = settings.net_tps_max as u64 * ttl * 2;
    let tx_id_cache: Cache<KaspaHash, ()> = Cache::builder().time_to_live(Duration::from_secs(ttl)).max_capacity(cache_size).build();

    let batch_scale = settings.cli_args.batch_scale;
    let batch_size = (5000f64 * batch_scale) as usize;

    let mut transactions = vec![];
    let mut block_tx = vec![];
    let mut tx_inputs = vec![];
    let mut tx_outputs = vec![];
    let mut tx_addresses = vec![];
    let mut last_block_time = 0;
    let mut last_commit_time = Instant::now();
    let mut valid_address = false;

    let mut subnetwork_map = SubnetworkMap::new();
    let results = database.select_subnetworks().await.expect("Select subnetworks FAILED");
    for s in results {
        subnetwork_map.insert(s.subnetwork_id, s.id);
    }
    info!("Loaded {} known subnetworks", subnetwork_map.len());

    while run.load(Ordering::Relaxed) {
        if let Some(rpc_transactions) = txs_queue.pop() {
            for rpc_transaction in rpc_transactions {
                let subnetwork_id = rpc_transaction.subnetwork_id.to_string();
                let subnetwork_key = match subnetwork_map.get(&subnetwork_id) {
                    Some(&subnetwork_key) => subnetwork_key,
                    None => {
                        let subnetwork_key = database.insert_subnetwork(&subnetwork_id).await.expect("Insert subnetwork FAILED");
                        subnetwork_map.insert(subnetwork_id.clone(), subnetwork_key);
                        info!("Committed new subnetwork, id: {} subnetwork_id: {}", subnetwork_key, subnetwork_id);
                        subnetwork_key
                    }
                };
                if !valid_address {
                    validate_address(&rpc_transaction);
                    valid_address = true;
                }
                let transaction_id =
                    rpc_transaction.verbose_data.as_ref().expect("Transaction verbose_data is missing").transaction_id;
                if tx_id_cache.contains_key(&transaction_id) {
                    trace!("Known transaction_id {}, keeping block relation only", transaction_id.to_string());
                } else {
                    let transaction = mapper.map_transaction(&rpc_transaction, subnetwork_key);
                    last_block_time = transaction.block_time;
                    transactions.push(transaction);
                    tx_inputs.extend(mapper.map_transaction_inputs(&rpc_transaction));
                    tx_outputs.extend(mapper.map_transaction_outputs(&rpc_transaction));
                    if !settings.cli_args.skip_resolving_addresses {
                        tx_addresses.extend(mapper.map_transaction_outputs_address(&rpc_transaction));
                    }
                    tx_id_cache.insert(transaction_id, ());
                }
                block_tx.push(mapper.map_block_transaction(&rpc_transaction));
            }

            if block_tx.len() >= batch_size || (block_tx.len() >= 1 && Instant::now().duration_since(last_commit_time).as_secs() > 2) {
                let start_commit_time = Instant::now();
                let transactions_len = transactions.len();
                let transaction_ids = transactions.iter().map(|t| t.transaction_id.clone()).collect();

                let tx_handle = task::spawn(insert_txs(batch_scale, transactions, database.clone()));
                let tx_inputs_handle = task::spawn(insert_tx_inputs(batch_scale, tx_inputs, database.clone()));
                let tx_outputs_handle = task::spawn(insert_tx_outputs(batch_scale, tx_outputs, database.clone()));
                let mut rows_affected_tx_addresses = 0;
                if !settings.cli_args.skip_resolving_addresses {
                    let tx_output_addr_handle = task::spawn(insert_output_tx_addr(batch_scale, tx_addresses, database.clone()));
                    rows_affected_tx_addresses += tx_output_addr_handle.await.unwrap()
                }
                let rows_affected_tx = tx_handle.await.unwrap();
                let rows_affected_tx_inputs = tx_inputs_handle.await.unwrap();
                let rows_affected_tx_outputs = tx_outputs_handle.await.unwrap();

                if !settings.cli_args.skip_resolving_addresses {
                    // ^Input address resolving can only happen after the transaction + inputs + outputs are committed
                    rows_affected_tx_addresses += insert_input_tx_addr(batch_scale, transaction_ids, database.clone()).await;
                }

                // ^All other transaction details needs to be committed before linking to blocks, to avoid incomplete checkpoints
                let rows_affected_block_tx = insert_block_txs(batch_scale, block_tx, database.clone()).await;

                let commit_time = Instant::now().duration_since(start_commit_time).as_millis();
                let tps = transactions_len as f64 / commit_time as f64 * 1000f64;
                info!(
                    "Committed {} new txs in {}ms ({:.1} tps, {} blk_tx, {} tx_in, {} tx_out, {} adr_tx). Last tx: {}",
                    rows_affected_tx,
                    commit_time,
                    tps,
                    rows_affected_block_tx,
                    rows_affected_tx_inputs,
                    rows_affected_tx_outputs,
                    rows_affected_tx_addresses,
                    chrono::DateTime::from_timestamp_millis(last_block_time / 1000 * 1000).unwrap()
                );

                transactions = vec![];
                block_tx = vec![];
                tx_inputs = vec![];
                tx_outputs = vec![];
                tx_addresses = vec![];
                last_commit_time = Instant::now();
            }
        } else {
            sleep(Duration::from_millis(100)).await;
        }
    }
}

async fn insert_txs(batch_scale: f64, values: Vec<Transaction>, database: KaspaDbClient) -> u64 {
    let batch_size = min((400f64 * batch_scale) as u16, 8000) as usize; // 2^16 / fields
    let key = "transactions";
    let start_time = Instant::now();
    debug!("Processing {} {}", values.len(), key);
    let mut rows_affected = 0;
    for batch_values in values.chunks(batch_size) {
        rows_affected += database.insert_transactions(batch_values).await.expect(format!("Insert {} FAILED", key).as_str());
    }
    debug!("Committed {} {} in {}ms", rows_affected, key, Instant::now().duration_since(start_time).as_millis());
    return rows_affected;
}

async fn insert_tx_inputs(batch_scale: f64, values: Vec<TransactionInput>, database: KaspaDbClient) -> u64 {
    let batch_size = min((400f64 * batch_scale) as u16, 8000) as usize; // 2^16 / fields
    let key = "transaction_inputs";
    let start_time = Instant::now();
    debug!("Processing {} {}", values.len(), key);
    let mut rows_affected = 0;
    for batch_values in values.chunks(batch_size) {
        rows_affected += database.insert_transaction_inputs(batch_values).await.expect(format!("Insert {} FAILED", key).as_str());
    }
    debug!("Committed {} {} in {}ms", rows_affected, key, Instant::now().duration_since(start_time).as_millis());
    return rows_affected;
}

async fn insert_tx_outputs(batch_scale: f64, values: Vec<TransactionOutput>, database: KaspaDbClient) -> u64 {
    let batch_size = min((500f64 * batch_scale) as u16, 10000) as usize; // 2^16 / fields
    let key = "transactions_outputs";
    let start_time = Instant::now();
    debug!("Processing {} {}", values.len(), key);
    let mut rows_affected = 0;
    for batch_values in values.chunks(batch_size) {
        rows_affected += database.insert_transaction_outputs(batch_values).await.expect(format!("Insert {} FAILED", key).as_str());
    }
    debug!("Committed {} {} in {}ms", rows_affected, key, Instant::now().duration_since(start_time).as_millis());
    return rows_affected;
}

async fn insert_input_tx_addr(batch_scale: f64, values: Vec<SqlHash>, database: KaspaDbClient) -> u64 {
    let batch_size = min((400f64 * batch_scale) as u16, 8000) as usize;
    let key = "input addresses_transactions";
    let start_time = Instant::now();
    debug!("Processing {} transactions for {}", values.len(), key);
    let mut rows_affected = 0;
    for batch_values in values.chunks(batch_size) {
        rows_affected +=
            database.insert_address_transactions_from_inputs(batch_values).await.expect(format!("Insert {} FAILED", key).as_str());
    }
    debug!("Committed {} {} in {}ms", rows_affected, key, Instant::now().duration_since(start_time).as_millis());
    return rows_affected;
}

async fn insert_output_tx_addr(batch_scale: f64, values: Vec<AddressTransaction>, database: KaspaDbClient) -> u64 {
    let batch_size = min((500f64 * batch_scale) as u16, 20000) as usize; // 2^16 / fields
    let key = "output addresses_transactions";
    let start_time = Instant::now();
    debug!("Processing {} {}", values.len(), key);
    let mut rows_affected = 0;
    for batch_values in values.chunks(batch_size) {
        rows_affected += database.insert_address_transactions(batch_values).await.expect(format!("Insert {} FAILED", key).as_str());
    }
    debug!("Committed {} {} in {}ms", rows_affected, key, Instant::now().duration_since(start_time).as_millis());
    return rows_affected;
}

async fn insert_block_txs(batch_scale: f64, values: Vec<BlockTransaction>, database: KaspaDbClient) -> u64 {
    let batch_size = min((800f64 * batch_scale) as u16, 30000) as usize; // 2^16 / fields
    let key = "block/transaction mappings";
    let start_time = Instant::now();
    debug!("Processing {} {}", values.len(), key);
    let mut rows_affected = 0;
    for batch_values in values.chunks(batch_size) {
        rows_affected += database.insert_block_transactions(batch_values).await.expect(format!("Insert {} FAILED", key).as_str());
    }
    debug!("Committed {} {} in {}ms", rows_affected, key, Instant::now().duration_since(start_time).as_millis());
    return rows_affected;
}

fn validate_address(rpc_transaction: &RpcTransaction) {
    if let Some(first_output) = rpc_transaction.outputs.first() {
        let verbose_data = first_output.verbose_data.as_ref().expect("Tx output verbose_data is missing");
        if !verbose_data.script_public_key_address.prefix.to_string().starts_with("kaspa") {
            panic!("Unexpected address");
        }
    }
}
