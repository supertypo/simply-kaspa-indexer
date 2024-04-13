extern crate diesel;

use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use crossbeam_queue::ArrayQueue;
use diesel::{BoolExpressionMethods, Connection, ExpressionMethods, insert_into, QueryDsl, RunQueryDsl};
use diesel::pg::PgConnection;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::result::Error;
use log::info;
use tokio::time::sleep;

use crate::database::models::TransactionOutput;
use crate::database::schema::transactions_outputs;

pub async fn insert_tx_outputs(db_transactions_outputs_queue: Arc<ArrayQueue<TransactionOutput>>,
                               db_pool: Pool<ConnectionManager<PgConnection>>) -> Result<(), ()> {
    const INSERT_QUEUE_SIZE: usize = 7500;
    let mut insert_queue: HashSet<TransactionOutput> = HashSet::with_capacity(INSERT_QUEUE_SIZE);
    let mut last_commit_time = SystemTime::now();
    let mut rows_affected = 0;
    loop {
        if insert_queue.len() >= INSERT_QUEUE_SIZE || (insert_queue.len() >= 1 && SystemTime::now().duration_since(last_commit_time).unwrap().as_secs() > 2) {
            let con = &mut db_pool.get().expect("Database connection FAILED");
            con.transaction(|con| {
                // Find existing transaction outputs and remove them from the insert queue
                transactions_outputs::dsl::transactions_outputs
                    .filter(transactions_outputs::transaction_id.eq_any(insert_queue.iter().map(|ti| ti.transaction_id.clone()).collect::<Vec<Vec<u8>>>())
                        .and(transactions_outputs::index.eq_any(insert_queue.iter().map(|ti| ti.index).collect::<Vec<i16>>())))
                    .load::<TransactionOutput>(con)
                    .unwrap().iter()
                    .for_each(|b| { insert_queue.remove(b); });

                rows_affected = insert_into(transactions_outputs::dsl::transactions_outputs)
                    .values(Vec::from_iter(insert_queue.iter()))
                    .execute(con)
                    .expect("Commit transaction outputs to database FAILED");

                Ok::<_, Error>(())
            }).expect("Commit transaction outputs to database FAILED");

            info!("Committed {} new transaction outputs to database.", rows_affected);
            insert_queue = HashSet::with_capacity(INSERT_QUEUE_SIZE);
            last_commit_time = SystemTime::now();
        }
        let tx_output_option = db_transactions_outputs_queue.pop();
        if tx_output_option.is_some() {
            insert_queue.insert(tx_output_option.unwrap());
        } else {
            sleep(Duration::from_millis(100)).await;
        }
    }
}
