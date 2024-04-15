extern crate diesel;


use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use crossbeam_queue::ArrayQueue;
use diesel::{Connection, insert_into, RunQueryDsl, sql_query};
use diesel::pg::PgConnection;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::result::Error;
use log::{debug, info};
use tokio::time::sleep;

use crate::database::models::{Similar, TransactionInput, TransactionOutput};
use crate::database::schema::transactions_inputs;
use crate::database::schema::transactions_outputs;

const INSERT_QUEUE_SIZE: usize = 7500;

pub async fn insert_tx_inputs_outputs(db_transactions_inputs_outputs_queue: Arc<ArrayQueue<(Vec<TransactionInput>, Vec<TransactionOutput>)>>,
                                      db_pool: Pool<ConnectionManager<PgConnection>>) -> Result<(), ()> {
    let mut insert_queue: Vec<(Vec<TransactionInput>, Vec<TransactionOutput>)> = vec![];
    let mut last_commit_time = SystemTime::now();
    loop {
        let tx_inputs_outputs_option = db_transactions_inputs_outputs_queue.pop();
        if tx_inputs_outputs_option.is_none() {
            sleep(Duration::from_secs(1)).await;
            continue;
        }
        insert_queue.push(tx_inputs_outputs_option.unwrap());

        if insert_queue.len() >= INSERT_QUEUE_SIZE || (insert_queue.len() >= 1 && SystemTime::now().duration_since(last_commit_time).unwrap().as_secs() > 2) {
            let mut merged_inputs = vec![];
            let mut merged_outputs = vec![];
            for (input, output) in insert_queue {
                for i in input {
                    merged_inputs.push(i);
                }
                for o in output {
                    merged_outputs.push(o);
                }
            }
            debug!("Enqueueing {} outputs for transaction_id={}", merged_outputs.len(), merged_outputs.first().map(|i| hex::encode(&i.transaction_id)).unwrap_or_default());
            insert_outputs(merged_outputs, db_pool.clone());
            debug!("Enqueueing {} inputs for transaction_id={}", merged_inputs.len(), merged_inputs.first().map(|i| hex::encode(&i.transaction_id)).unwrap_or_default());
            insert_inputs(merged_inputs, db_pool.clone());

            insert_queue = vec![];
            last_commit_time = SystemTime::now();
        }
    }
}

fn insert_outputs(outputs: Vec<TransactionOutput>, db_pool: Pool<ConnectionManager<PgConnection>>) {
    for outputs_chunked in outputs.chunks(INSERT_QUEUE_SIZE) {
        debug!("Output chunk: {}", outputs_chunked.len());
        let mut outputs_set: HashSet<&TransactionOutput> = HashSet::from_iter(outputs_chunked.into_iter());
        let mut rows_affected = 0;
        let con = &mut db_pool.get().expect("Database connection FAILED");
        con.transaction(|con| {
            // Find existing transaction outputs and remove them from the insert queue
            debug!("Size before filtering existing outputs: {}", outputs_set.len());
            sql_query(format!("SELECT * FROM transactions_outputs WHERE (transaction_id, index) IN ({})",
                              outputs_set.iter().map(|ti| format!("('\\x{}', {})", hex::encode(ti.transaction_id.clone()), ti.index)).collect::<Vec<String>>().join(", ")))
                .load::<TransactionOutput>(con)
                .unwrap().iter()
                .for_each(|to| {
                    if !to.is_similar(outputs_set.get(to).unwrap()) { panic!("Was about to remove a different transaction output") };
                    outputs_set.remove(to);
                });
            debug!("Size after filtering existing outputs: {}", outputs_set.len());

            rows_affected = insert_into(transactions_outputs::dsl::transactions_outputs)
                .values(Vec::from_iter(outputs_set.into_iter()))
                .execute(con)
                .expect("Commit transaction outputs to database FAILED");

            Ok::<_, Error>(())
        }).expect("Commit transaction outputs to database FAILED");

        info!("Committed {} new transaction inputs to database.", rows_affected);
    }
}

fn insert_inputs(inputs: Vec<TransactionInput>, db_pool: Pool<ConnectionManager<PgConnection>>) {
    for inputs_chunked in inputs.chunks(5000) {
        let mut inputs_set: HashSet<&TransactionInput> = HashSet::from_iter(inputs_chunked.into_iter());
        let mut rows_affected = 0;
        let con = &mut db_pool.get().expect("Database connection FAILED");
        con.transaction(|con| {
            let now = SystemTime::now();
            debug!("Size before filtering existing inputs: {}", inputs_set.len());
            sql_query(format!("SELECT * FROM transactions_inputs WHERE (transaction_id, index) IN ({})",
                              inputs_set.iter().map(|ti| format!("('\\x{}', {})", hex::encode(ti.transaction_id.clone()), ti.index)).collect::<Vec<String>>().join(", ")))
                .load::<TransactionInput>(con)
                .unwrap().iter()
                .for_each(|ti| {
                    let new = inputs_set.get(ti).unwrap();
                    if !ti.is_similar(new) { panic!("Was about to remove a different transaction input:\n\nOriginal: {:#?}\n\nNew: {:#?}", ti, new) };
                    inputs_set.remove(ti);
                });
            debug!("Size after filtering existing inputs: {}, took {}ms", inputs_set.len(), SystemTime::now().duration_since(now).unwrap().as_millis());

            if !inputs_set.is_empty() {
                let previous_outpoint_address_map = sql_query(
                    format!("SELECT * FROM transactions_outputs WHERE (transaction_id, index) IN ({})",
                            inputs_set.iter().map(|ti| format!("('\\x{}', {})", hex::encode(ti.previous_outpoint_hash.clone()), ti.previous_outpoint_index)).collect::<Vec<String>>().join(", ")))
                    .load::<TransactionOutput>(con)
                    .unwrap().into_iter()
                    .map(|to| ((to.transaction_id, to.index), to.script_public_key_address))
                    .collect::<HashMap<(Vec<u8>, i16), String>>();
                debug!("Found {} matching previous outputs", previous_outpoint_address_map.len());

                // Map addresses to inputs
                let inputs_with_address = inputs_set.into_iter().map(|i| {
                    let mut input = i.clone();
                    input.script_public_key_address = previous_outpoint_address_map.get(&(i.previous_outpoint_hash.clone(), i.previous_outpoint_index)).cloned();
                    return input;
                }).collect::<Vec<TransactionInput>>();

                rows_affected = insert_into(transactions_inputs::dsl::transactions_inputs)
                    .values(&inputs_with_address)
                    .execute(con)
                    .expect("Commit transaction inputs to database FAILED");
            }
            Ok::<_, Error>(())
        }).expect("Commit transaction inputs to database FAILED");

        info!("Committed {} new transaction inputs to database.", rows_affected);
    }
}
