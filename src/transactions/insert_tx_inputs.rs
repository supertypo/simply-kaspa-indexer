extern crate diesel;

use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use crossbeam_queue::ArrayQueue;
use diesel::{Connection, insert_into, RunQueryDsl, sql_query};
use diesel::pg::PgConnection;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::result::Error;
use log::{debug, info};
use tokio::time::sleep;

use crate::database::models::{Similar, TransactionInput};
use crate::database::schema::transactions_inputs;

pub async fn insert_tx_inputs(db_transactions_inputs_queue: Arc<ArrayQueue<TransactionInput>>,
                              db_pool: Pool<ConnectionManager<PgConnection>>) -> Result<(), ()> {
    const INSERT_QUEUE_SIZE: usize = 7500;
    let mut insert_queue: HashSet<TransactionInput> = HashSet::with_capacity(INSERT_QUEUE_SIZE);
    let mut last_commit_time = SystemTime::now();
    let mut rows_affected = 0;
    loop {
        if insert_queue.len() >= INSERT_QUEUE_SIZE || (insert_queue.len() >= 1 && SystemTime::now().duration_since(last_commit_time).unwrap().as_secs() > 2) {
            let con = &mut db_pool.get().expect("Database connection FAILED");
            con.transaction(|con| {
                //
                // Find existing transaction inputs and remove them from the insert queue
                let now = SystemTime::now();
                debug!("Size before filtering existing inputs: {}", insert_queue.len());
                sql_query(format!("SELECT * FROM transactions_inputs WHERE (transaction_id, index) IN ({})",
                                  insert_queue.iter().map(|ti| format!("('\\x{}', {})", hex::encode(ti.transaction_id.clone()), ti.index)).collect::<Vec<String>>().join(", ")))
                    .load::<TransactionInput>(con)
                    .unwrap().iter()
                    .for_each(|ti| {
                        let new = insert_queue.get(ti).unwrap();
                        if !ti.is_similar(new) { panic!("Was about to remove a different transaction input:\n\nOriginal: {:#?}\n\nNew: {:#?}", ti, new) };
                        insert_queue.remove(ti);
                    });
                debug!("Size after filtering existing inputs: {}, took {}ms", insert_queue.len(), SystemTime::now().duration_since(now).unwrap().as_millis());

                rows_affected = insert_into(transactions_inputs::dsl::transactions_inputs)
                    .values(Vec::from_iter(insert_queue.iter()))
                    .execute(con)
                    .expect("Commit transaction inputs to database FAILED");

                Ok::<_, Error>(())
            }).expect("Commit transaction inputs to database FAILED");

            info!("Committed {} new transaction inputs to database.", rows_affected);
            insert_queue = HashSet::with_capacity(INSERT_QUEUE_SIZE);
            last_commit_time = SystemTime::now();
        }
        let tx_input_option = db_transactions_inputs_queue.pop();
        if tx_input_option.is_some() {
            insert_queue.insert(tx_input_option.unwrap());
        } else {
            sleep(Duration::from_millis(100)).await;
        }
    }
}
