use crate::settings::Settings;
use crate::transactions::process_transactions::{insert_tx_addr, insert_tx_script, insert_txs};
use futures_util::{StreamExt, stream};
use indexmap::IndexSet;
use kaspa_hashes::Hash as KaspaHash;
use kaspa_rpc_core::RpcChainBlockAcceptedTransactions;
use log::{debug, trace};
use moka::sync::Cache;
use simply_kaspa_cli::cli_args::{CliDisable, CliField};
use simply_kaspa_database::client::KaspaDbClient;
use simply_kaspa_database::models::address_transaction::AddressTransaction;
use simply_kaspa_database::models::script_transaction::ScriptTransaction;
use simply_kaspa_database::models::transaction::Transaction;
use simply_kaspa_database::models::transaction_acceptance::TransactionAcceptance;
use simply_kaspa_database::models::types::hash::Hash as SqlHash;
use simply_kaspa_mapping::mapper::KaspaDbMapper;
use std::cmp::min;
use std::time::{Duration, Instant};
use tokio::task;

pub async fn accept_transactions(
    batch_scale: f64,
    batch_concurrency: i8,
    settings: &Settings,
    chain_block_accepted_transactions: &[RpcChainBlockAcceptedTransactions],
    database: &KaspaDbClient,
    mapper: &KaspaDbMapper,
) -> (u64, u64, u64) {
    let ttl = settings.cli_args.cache_ttl;
    let cache_size = settings.net_tps_max as u64 * ttl * 2;
    let tx_id_cache: Cache<KaspaHash, ()> = Cache::builder().time_to_live(Duration::from_secs(ttl)).max_capacity(cache_size).build();

    let disable_transactions = settings.cli_args.is_disabled(CliDisable::TransactionsTable);
    let disable_address_transactions = settings.cli_args.is_disabled(CliDisable::AddressesTransactionsTable);
    let exclude_tx_out_script_public_key_address = settings.cli_args.is_excluded(CliField::TxOutScriptPublicKeyAddress);
    let exclude_tx_out_script_public_key = settings.cli_args.is_excluded(CliField::TxOutScriptPublicKey);

    let mut accepted_transactions = vec![];
    let mut transactions: Vec<Transaction> = vec![];
    let mut address_transactions: IndexSet<AddressTransaction> = IndexSet::new();
    let mut script_transactions: IndexSet<ScriptTransaction> = IndexSet::new();

    for chain_block in chain_block_accepted_transactions {
        let block_hash: SqlHash = chain_block.chain_block_header.hash.unwrap().into();

        for transaction in &chain_block.accepted_transactions {
            if mapper.is_self_send_full(transaction) {
                continue;
            }
            let transaction_id = transaction.verbose_data.as_ref().unwrap().transaction_id.unwrap();
            accepted_transactions
                .push(TransactionAcceptance { transaction_id: Some(transaction_id.into()), block_hash: Some(block_hash.clone()) });
            if tx_id_cache.contains_key(&transaction_id) {
                trace!("Known transaction_id {}, skipping", transaction_id);
            } else {
                if !disable_transactions {
                    transactions.push(mapper.map_optional_transaction(transaction));
                }
                if !disable_address_transactions {
                    if !exclude_tx_out_script_public_key_address {
                        address_transactions.extend(mapper.map_optional_transaction_inputs_address(transaction));
                        address_transactions.extend(mapper.map_optional_transaction_outputs_address(transaction));
                    } else if !exclude_tx_out_script_public_key {
                        script_transactions.extend(mapper.map_optional_transaction_inputs_script(transaction));
                        script_transactions.extend(mapper.map_optional_transaction_outputs_script(transaction));
                    }
                }
                tx_id_cache.insert(transaction_id, ());
            }
        }
    }

    if log::log_enabled!(log::Level::Debug) {
        debug!("Received {} accepted transactions ({} to upsert)", accepted_transactions.len(), transactions.len());
    }

    let acceptances_handle =
        task::spawn(insert_transaction_acceptances(batch_scale, batch_concurrency, accepted_transactions, database.clone()));
    let tx_handle = task::spawn(insert_txs(batch_scale, batch_concurrency, transactions, true, database.clone()));
    let addr_tx_handle = if !exclude_tx_out_script_public_key_address {
        task::spawn(insert_tx_addr(batch_scale, batch_concurrency, address_transactions.into_iter().collect(), database.clone()))
    } else {
        task::spawn(insert_tx_script(batch_scale, batch_concurrency, script_transactions.into_iter().collect(), database.clone()))
    };

    let rows_affected_tx_acc = acceptances_handle.await.unwrap();
    let rows_affected_tx = tx_handle.await.unwrap();
    let rows_affected_tx_addr = addr_tx_handle.await.unwrap();

    (rows_affected_tx_acc, rows_affected_tx, rows_affected_tx_addr)
}

async fn insert_transaction_acceptances(
    batch_scale: f64,
    batch_concurrency: i8,
    values: Vec<TransactionAcceptance>,
    database: KaspaDbClient,
) -> u64 {
    let batch_size = min((1300f64 * batch_scale) as u16, 30000) as usize;
    let key = "transaction_acceptances";
    let start_time = Instant::now();
    debug!("Processing {} {}", values.len(), key);
    let mut values = values;
    values.sort_by(|a, b| a.transaction_id.cmp(&b.transaction_id));
    let chunks: Vec<Vec<_>> = values.chunks(batch_size).map(|c| c.to_vec()).collect();
    let rows_affected = stream::iter(chunks.into_iter().map(|chunk| {
        let db = database.clone();
        async move { db.insert_transaction_acceptances(&chunk).await.unwrap_or_else(|e| panic!("Insert {key} FAILED: {e}")) }
    }))
    .buffer_unordered(batch_concurrency as usize)
    .fold(0, |acc, rows| async move { acc + rows })
    .await;
    debug!("Committed {} {} in {}ms", rows_affected, key, start_time.elapsed().as_millis());
    rows_affected
}
