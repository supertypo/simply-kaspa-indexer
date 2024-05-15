extern crate diesel;

use std::cmp::min;
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crossbeam_queue::ArrayQueue;
use diesel::pg::PgConnection;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::result::Error;
use diesel::sql_types::{Array, Bytea};
use diesel::{insert_into, sql_query, Connection, RunQueryDsl};
use log::{debug, info, trace};
use tokio::task;
use tokio::time::sleep;

use crate::database::address_transaction::AddressTransaction;
use crate::database::block_transaction::BlockTransaction;
use crate::database::schema::{addresses_transactions, blocks_transactions, transactions, transactions_inputs, transactions_outputs};
use crate::database::transaction::Transaction;
use crate::database::transaction_input::TransactionInput;
use crate::database::transaction_output::TransactionOutput;

pub async fn insert_txs_ins_outs(
    run: Arc<AtomicBool>,
    batch_scale: f64,
    db_transactions_queue: Arc<
        ArrayQueue<(Transaction, BlockTransaction, Vec<TransactionInput>, Vec<TransactionOutput>, Vec<AddressTransaction>)>,
    >,
    db_pool: Pool<ConnectionManager<PgConnection>>,
) {
    let batch_size = min((1000f64 * batch_scale) as u16, 7500);
    let set_size = 3 * batch_size as usize; // Large sets helps us to filter duplicates during catch-up
    let mut transactions: HashSet<Transaction> = HashSet::with_capacity(set_size);
    let mut block_tx: HashSet<BlockTransaction> = HashSet::with_capacity(set_size);
    let mut tx_inputs: HashSet<TransactionInput> = HashSet::with_capacity(set_size * 2);
    let mut tx_outputs: HashSet<TransactionOutput> = HashSet::with_capacity(set_size * 2);
    let mut tx_addresses: HashSet<AddressTransaction> = HashSet::with_capacity(set_size * 2);
    let mut last_block_timestamp;
    let mut last_commit_time = Instant::now();

    while run.load(Ordering::Relaxed) {
        if let Some((transaction, block_transactions, inputs, outputs, addresses)) = db_transactions_queue.pop() {
            last_block_timestamp = transaction.block_time;
            transactions.insert(transaction);
            block_tx.insert(block_transactions);
            tx_inputs.extend(inputs.into_iter());
            tx_outputs.extend(outputs.into_iter());
            tx_addresses.extend(addresses.into_iter());

            if block_tx.len() >= set_size || (block_tx.len() >= 1 && Instant::now().duration_since(last_commit_time).as_secs() > 2) {
                let start_commit_time = Instant::now();
                debug!(
                    "Committing {} transactions ({} block/tx, {} inputs, {} outputs)",
                    transactions.len(),
                    block_tx.len(),
                    tx_inputs.len(),
                    tx_outputs.len()
                );
                // We used a HashSet first to filter some amount of duplicates locally, now we can switch back to vector:
                let transactions_len = transactions.len();
                let transactions_vec: Vec<Transaction> = transactions.into_iter().collect();
                let transaction_ids = transactions_vec.iter().map(|t| t.transaction_id.clone()).collect();
                let block_transactions_vec = block_tx.into_iter().collect();
                let inputs_vec = tx_inputs.into_iter().collect();
                let outputs_vec = tx_outputs.into_iter().collect();
                let addresses_vec = tx_addresses.into_iter().collect();

                let db_pool_clone = db_pool.clone();
                let tx_handle = task::spawn_blocking(move || insert_txs(batch_size, transactions_vec, db_pool_clone));
                let db_pool_clone = db_pool.clone();
                let tx_inputs_handle = task::spawn_blocking(move || insert_tx_inputs(batch_size, inputs_vec, db_pool_clone));
                let db_pool_clone = db_pool.clone();
                let tx_outputs_handle = task::spawn_blocking(move || insert_tx_outputs(batch_size, outputs_vec, db_pool_clone));
                let db_pool_clone = db_pool.clone();
                let tx_out_addr_handle = task::spawn_blocking(move || insert_output_tx_addr(batch_size, addresses_vec, db_pool_clone));

                let rows_affected_tx = tx_handle.await.unwrap();
                let rows_affected_tx_inputs = tx_inputs_handle.await.unwrap();
                let rows_affected_tx_outputs = tx_outputs_handle.await.unwrap();
                let mut rows_affected_tx_addresses = tx_out_addr_handle.await.unwrap();
                // ^Input address resolving can only happen after the transaction + inputs + outputs are committed
                let db_pool_clone = db_pool.clone();
                let tx_in_addr_handle = task::spawn_blocking(move || insert_input_tx_addr(batch_size, transaction_ids, db_pool_clone));
                rows_affected_tx_addresses += tx_in_addr_handle.await.unwrap();
                // ^All other transaction details needs to be committed before linking to blocks, to avoid incomplete checkpoints
                let db_pool_clone = db_pool.clone();
                let blk_tx_handle = task::spawn_blocking(move || insert_block_txs(batch_size, block_transactions_vec, db_pool_clone));
                let rows_affected_block_tx = blk_tx_handle.await.unwrap();

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
                    chrono::DateTime::from_timestamp_millis(last_block_timestamp / 1000 * 1000).unwrap()
                );

                transactions = HashSet::with_capacity(set_size);
                block_tx = HashSet::with_capacity(set_size);
                tx_inputs = HashSet::with_capacity(set_size * 2);
                tx_outputs = HashSet::with_capacity(set_size * 2);
                tx_addresses = HashSet::with_capacity(set_size * 2);
                last_commit_time = Instant::now();
            }
        } else {
            sleep(Duration::from_millis(100)).await;
        }
    }
}

fn insert_input_tx_addr(max_batch_size: u16, values: Vec<Vec<u8>>, db_pool: Pool<ConnectionManager<PgConnection>>) -> usize {
    let key = "input addresses_transactions";
    let start_time = Instant::now();
    debug!("Processing {} transactions for {}", values.len(), key);
    let mut rows_affected = 0;
    let con = &mut db_pool.get().expect("Database connection FAILED");
    for batch_values in values.chunks(max_batch_size as usize) {
        con.transaction(|con| {
            let query = "\
            INSERT INTO addresses_transactions (address, transaction_id, block_time) \
                    SELECT o.script_public_key_address, i.transaction_id, t.block_time \
                        FROM transactions_inputs i \
                        JOIN transactions t ON t.transaction_id = i.transaction_id \
                        JOIN transactions_outputs o ON o.transaction_id = i.previous_outpoint_hash AND o.index = i.previous_outpoint_index \
                    WHERE i.transaction_id = ANY($1) AND t.transaction_id = ANY($1) \
                    ON CONFLICT DO NOTHING";
            trace!("Executing {} query:\n{}", key, query);
            rows_affected += sql_query(query)
                .bind::<Array<Bytea>, _>(batch_values)
                .execute(con)
                .expect(format!("Commit {} FAILED", key).as_str());
            Ok::<_, Error>(())
        }).expect(format!("Commit {} FAILED", key).as_str());
    }
    debug!("Committed {} {} in {}ms", rows_affected, key, Instant::now().duration_since(start_time).as_millis());
    return rows_affected;
}

fn insert_output_tx_addr(
    max_batch_size: u16,
    values: Vec<AddressTransaction>,
    db_pool: Pool<ConnectionManager<PgConnection>>,
) -> usize {
    let key = "output addresses_transactions";
    let start_time = Instant::now();
    debug!("Processing {} {}", values.len(), key);
    let mut rows_affected = 0;
    let con = &mut db_pool.get().expect("Database connection FAILED");
    for batch_values in values.chunks(max_batch_size as usize) {
        con.transaction(|con| {
            rows_affected += insert_into(addresses_transactions::dsl::addresses_transactions)
                .values(batch_values)
                .on_conflict_do_nothing() //Ignore conflicts as any conflicting rows will be identical
                .execute(con)
                .expect(format!("Commit {} FAILED", key).as_str());
            Ok::<_, Error>(())
        })
        .expect(format!("Commit {} FAILED", key).as_str());
    }
    debug!("Committed {} {} in {}ms", rows_affected, key, Instant::now().duration_since(start_time).as_millis());
    return rows_affected;
}

fn insert_tx_outputs(max_batch_size: u16, values: Vec<TransactionOutput>, db_pool: Pool<ConnectionManager<PgConnection>>) -> usize {
    let key = "transactions_outputs";
    let start_time = Instant::now();
    debug!("Processing {} {}", values.len(), key);
    let mut rows_affected = 0;
    let con = &mut db_pool.get().expect("Database connection FAILED");
    for batch_values in values.chunks(max_batch_size as usize) {
        con.transaction(|con| {
            rows_affected += insert_into(transactions_outputs::dsl::transactions_outputs)
                .values(batch_values)
                .on_conflict_do_nothing() //Ignore conflicts as any conflicting rows will be identical
                .execute(con)
                .expect(format!("Commit {} FAILED", key).as_str());
            Ok::<_, Error>(())
        })
        .expect(format!("Commit {} FAILED", key).as_str());
    }
    debug!("Committed {} {} in {}ms", rows_affected, key, Instant::now().duration_since(start_time).as_millis());
    return rows_affected;
}

fn insert_tx_inputs(max_batch_size: u16, values: Vec<TransactionInput>, db_pool: Pool<ConnectionManager<PgConnection>>) -> usize {
    let key = "transaction_inputs";
    let start_time = Instant::now();
    debug!("Processing {} {}", values.len(), key);
    let mut rows_affected = 0;
    let con = &mut db_pool.get().expect("Database connection FAILED");
    for batch_values in values.chunks(max_batch_size as usize) {
        con.transaction(|con| {
            rows_affected += insert_into(transactions_inputs::dsl::transactions_inputs)
                .values(batch_values)
                .on_conflict_do_nothing() //Ignore conflicts as any conflicting rows will be identical
                .execute(con)
                .expect(format!("Commit {} FAILED", key).as_str());
            Ok::<_, Error>(())
        })
        .expect(format!("Commit {} FAILED", key).as_str());
    }
    debug!("Committed {} {} in {}ms", rows_affected, key, Instant::now().duration_since(start_time).as_millis());
    return rows_affected;
}

fn insert_txs(max_batch_size: u16, values: Vec<Transaction>, db_pool: Pool<ConnectionManager<PgConnection>>) -> usize {
    let key = "transactions";
    let start_time = Instant::now();
    debug!("Processing {} {}", values.len(), key);
    let mut rows_affected = 0;
    let con = &mut db_pool.get().expect("Database connection FAILED");
    for batch_values in values.chunks(max_batch_size as usize) {
        con.transaction(|con| {
            rows_affected += insert_into(transactions::dsl::transactions)
                .values(batch_values)
                .on_conflict_do_nothing() //Ignore conflicts as any conflicting rows will be identical
                .execute(con)
                .expect(format!("Commit {} FAILED", key).as_str());
            Ok::<_, Error>(())
        })
        .expect(format!("Commit {} FAILED", key).as_str());
    }
    debug!("Committed {} {} in {}ms", rows_affected, key, Instant::now().duration_since(start_time).as_millis());
    return rows_affected;
}

fn insert_block_txs(max_batch_size: u16, values: Vec<BlockTransaction>, db_pool: Pool<ConnectionManager<PgConnection>>) -> usize {
    let key = "block/transaction mappings";
    let start_time = Instant::now();
    debug!("Processing {} {}", values.len(), key);
    let mut rows_affected = 0;
    let con = &mut db_pool.get().expect("Database connection FAILED");
    for batch_values in values.chunks(max_batch_size as usize) {
        con.transaction(|con| {
            rows_affected = insert_into(blocks_transactions::dsl::blocks_transactions)
                .values(batch_values)
                .on_conflict_do_nothing() //Ignore conflicts as any conflicting rows will be identical
                .execute(con)
                .expect(format!("Commit {} FAILED", key).as_str());
            Ok::<_, Error>(())
        })
        .expect(format!("Commit {} FAILED", key).as_str());
    }
    debug!("Committed {} {} in {}ms", rows_affected, key, Instant::now().duration_since(start_time).as_millis());
    return rows_affected;
}
