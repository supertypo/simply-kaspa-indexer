extern crate diesel;

use std::cmp::min;

use itertools::Itertools;
use kaspa_rpc_core::{RpcAcceptedTransactionIds, RpcHash};
use log::{debug, info, trace};
use sqlx::{Pool, Postgres};

use crate::database::models::transaction_acceptance::TransactionAcceptance;

pub async fn update_txs(
    batch_scale: f64,
    removed_hashes: &Vec<RpcHash>,
    accepted_transaction_ids: &Vec<RpcAcceptedTransactionIds>,
    last_accepting_time: u64,
    db_pool: &Pool<Postgres>,
) {
    let batch_size = min((1000f64 * batch_scale) as usize, 7500);
    if log::log_enabled!(log::Level::Debug) {
        let accepted_count = accepted_transaction_ids.iter().map(|t| t.accepted_transaction_ids.len()).sum::<usize>();
        debug!("Received {} accepted transactions and {} removed chain blocks", accepted_count, removed_hashes.len());
        trace!("Accepted transaction ids: \n{:#?}", accepted_transaction_ids);
        trace!("Removed chain blocks: \n{:#?}", removed_hashes);
    }

    let mut rows_removed = 0;
    let mut rows_added = 0;

    let removed_blocks = removed_hashes.into_iter().map(|h| h.as_bytes().to_vec()).collect::<Vec<Vec<u8>>>();
    for removed_blocks_chunk in removed_blocks.chunks(batch_size) {
        debug!("Processing {} removed chain blocks", removed_blocks_chunk.len());
        rows_removed += delete_tas(removed_blocks_chunk, db_pool).await.expect("Delete accepted transactions FAILED");
    }
    let mut accepted_transactions = vec![];
    for accepted_id in accepted_transaction_ids {
        for transaction_id in accepted_id.accepted_transaction_ids.iter() {
            accepted_transactions.push(TransactionAcceptance {
                transaction_id: transaction_id.as_bytes().to_vec(),
                block_hash: accepted_id.accepting_block_hash.as_bytes().to_vec(),
            });
        }
    }
    for accepted_transactions_chunk in accepted_transactions.chunks(batch_size) {
        debug!("Processing {} accepted transactions", accepted_transactions_chunk.len());
        rows_added += commit_tas(accepted_transactions_chunk, db_pool).await.expect("Commit accepted transactions FAILED");
    }

    info!(
        "Committed {} accepted and {} rejected transactions. Last accepted: {}",
        rows_added,
        rows_removed,
        chrono::DateTime::from_timestamp_millis(last_accepting_time as i64 / 1000 * 1000).unwrap()
    );
}

async fn delete_tas(block_hashes: &[Vec<u8>], db_pool: &Pool<Postgres>) -> Result<u64, sqlx::Error> {
    Ok(sqlx::query("DELETE FROM transactions_acceptances WHERE block_hash = ANY($1)")
        .bind(&block_hashes)
        .execute(db_pool)
        .await?
        .rows_affected())
}

async fn commit_tas(tas: &[TransactionAcceptance], db_pool: &Pool<Postgres>) -> Result<u64, sqlx::Error> {
    const COLS: usize = 2;
    let sql = format!(
        "INSERT INTO transactions_acceptances (transaction_id, block_hash) VALUES {} ON CONFLICT DO NOTHING",
        (0..tas.len()).map(|i| format!("({})", (1..=COLS).map(|c| format!("${}", c + i * COLS)).join(", "))).join(", ")
    );
    let mut query = sqlx::query(&sql);
    for ta in tas {
        query = query.bind(&ta.transaction_id);
        query = query.bind(&ta.block_hash);
    }
    Ok(query.execute(db_pool).await?.rows_affected())
}
