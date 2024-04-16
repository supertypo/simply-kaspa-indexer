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

use crate::database::models::{Similar, TransactionOutput};
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
                //
                // Find existing transaction outputs and remove them from the insert queue
                let now = SystemTime::now();
                debug!("Size before filtering existing outputs: {}", insert_queue.len());
                sql_query(format!("SELECT * FROM transactions_outputs WHERE (transaction_id, index) IN ({})",
                                  insert_queue.iter().map(|ti| format!("('\\x{}', {})", hex::encode(ti.transaction_id.clone()), ti.index)).collect::<Vec<String>>().join(", ")))
                    .load::<TransactionOutput>(con)
                    .unwrap().iter()
                    .for_each(|to| {
                        if !to.is_similar(insert_queue.get(to).unwrap()) { panic!("Was about to remove a different transaction output") };
                        insert_queue.remove(to);
                    });
                debug!("Size after filtering existing outputs: {}, took {}ms", insert_queue.len(), SystemTime::now().duration_since(now).unwrap().as_millis());

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
