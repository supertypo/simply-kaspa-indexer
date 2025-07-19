use crate::models::types::hash::Hash;
use log::{debug, trace};
use sqlx::{Error, Executor, Pool, Postgres};
use std::collections::HashSet;

pub async fn delete_transaction_acceptances(block_hashes: &[Hash], pool: &Pool<Postgres>) -> Result<u64, Error> {
    Ok(sqlx::query("DELETE FROM transactions_acceptances WHERE block_hash = ANY($1)")
        .bind(block_hashes)
        .execute(pool)
        .await?
        .rows_affected())
}

pub async fn prune_block_parent(block_time_lt: i64, pool: &Pool<Postgres>) -> Result<u64, Error> {
    let sql = "DELETE FROM block_parent bp USING blocks b WHERE bp.block_hash = b.hash AND b.timestamp < $1";
    Ok(sqlx::query(sql).bind(block_time_lt).execute(pool).await?.rows_affected())
}

pub async fn prune_blocks_transactions_using_blocks(block_time_lt: i64, pool: &Pool<Postgres>) -> Result<u64, Error> {
    let sql = "DELETE FROM blocks_transactions bt USING blocks b WHERE bt.block_hash = b.hash AND b.timestamp < $1";
    Ok(sqlx::query(sql).bind(block_time_lt).execute(pool).await?.rows_affected())
}

pub async fn prune_transactions_acceptances_using_blocks(block_time_lt: i64, pool: &Pool<Postgres>) -> Result<u64, Error> {
    let sql = "DELETE FROM transactions_acceptances ta USING blocks b WHERE ta.block_hash = b.hash AND b.timestamp < $1";
    Ok(sqlx::query(sql).bind(block_time_lt).execute(pool).await?.rows_affected())
}

pub async fn prune_blocks(block_time_lt: i64, pool: &Pool<Postgres>) -> Result<u64, Error> {
    Ok(sqlx::query("DELETE FROM blocks WHERE timestamp < $1").bind(block_time_lt).execute(pool).await?.rows_affected())
}

pub async fn prune_blocks_transactions_using_transactions(block_time_lt: i64, pool: &Pool<Postgres>) -> Result<u64, Error> {
    let sql =
        "DELETE FROM blocks_transactions bt USING transactions t WHERE bt.transaction_id = t.transaction_id AND t.block_time < $1";
    Ok(sqlx::query(sql).bind(block_time_lt).execute(pool).await?.rows_affected())
}

pub async fn prune_unspendable_transactions_outputs(block_time_lt: i64, pool: &Pool<Postgres>) -> Result<u64, Error> {
    let mut tx = pool.begin().await?;
    let sql = "
        CREATE TEMP TABLE tmp_relevant_transactions AS
        SELECT transaction_id
        FROM transactions
        WHERE block_time < $1;
        ";
    tx.execute(sqlx::query(sql).bind(block_time_lt)).await?;
    tx.execute(sqlx::query("CREATE INDEX ON tmp_relevant_transactions (transaction_id)")).await?;
    tx.execute(sqlx::query("ANALYZE tmp_relevant_transactions")).await?;
    let sql = "
        DELETE FROM transactions_outputs to_
        USING tmp_relevant_transactions rt
        WHERE to_.transaction_id = rt.transaction_id
        ";
    let mut rows_affected = tx.execute(sqlx::query(sql)).await?.rows_affected();
    let sql = "
        CREATE TEMP TABLE temp_relevant_inputs AS
        SELECT ti.previous_outpoint_hash, ti.previous_outpoint_index
        FROM transactions_inputs ti
        JOIN tmp_relevant_transactions rt ON rt.transaction_id = ti.previous_outpoint_hash
        ";
    tx.execute(sqlx::query(sql)).await?;
    tx.execute(sqlx::query("CREATE INDEX ON temp_relevant_inputs(previous_outpoint_hash, previous_outpoint_index)")).await?;
    tx.execute(sqlx::query("ANALYZE temp_relevant_inputs")).await?;
    let sql = "
        DELETE FROM transactions_outputs to_
        USING temp_relevant_inputs ri
        WHERE to_.transaction_id = ri.previous_outpoint_hash
        AND to_.index = ri.previous_outpoint_index
        ";
    rows_affected += tx.execute(sqlx::query(sql)).await?.rows_affected();
    tx.commit().await?;
    Ok(rows_affected)
}

pub async fn prune_transactions_inputs(block_time_lt: i64, pool: &Pool<Postgres>) -> Result<u64, Error> {
    let sql =
        "DELETE FROM transactions_inputs ti USING transactions t WHERE t.transaction_id = ti.transaction_id AND t.block_time < $1";
    Ok(sqlx::query(sql).bind(block_time_lt).execute(pool).await?.rows_affected())
}

pub async fn prune_transactions_acceptances_using_transactions(block_time_lt: i64, pool: &Pool<Postgres>) -> Result<u64, Error> {
    let sql = "DELETE FROM transactions_acceptances ta USING transactions t WHERE t.transaction_id = ta.transaction_id AND t.block_time < $1";
    Ok(sqlx::query(sql).bind(block_time_lt).execute(pool).await?.rows_affected())
}

pub async fn prune_transactions(block_time_lt: i64, pool: &Pool<Postgres>) -> Result<u64, Error> {
    const BATCH_SIZE: usize = 10_000;
    let mut total_rows_affected = 0;
    let mut tx = pool.begin().await?;

    // Find & delete old transactions
    let sql = "DELETE FROM transactions WHERE block_time < $1 RETURNING transaction_id";
    let expired_txids = sqlx::query_scalar::<_, Hash>(sql).bind(block_time_lt).fetch_all(tx.as_mut()).await?;
    total_rows_affected += expired_txids.len() as u64;
    debug!("prune_transactions: Found & deleted {} expired transactions", expired_txids.len());

    // Find rejected transactions
    let mut accepted_txids = HashSet::new();
    for expired_txs_chunk in expired_txids.chunks(BATCH_SIZE) {
        let sql = "SELECT transaction_id FROM transactions_acceptances WHERE transaction_id = ANY($1)";
        accepted_txids.extend(sqlx::query_scalar::<_, Hash>(sql).bind(expired_txs_chunk).fetch_all(tx.as_mut()).await?);
    }
    let rejected_txids: Vec<Hash> = expired_txids.iter().filter(|id| !accepted_txids.contains(id)).cloned().collect();
    debug!("prune_transactions: Found {} expired rejected transactions", rejected_txids.len());

    // Delete rejected transaction inputs
    let mut rows_affected = 0;
    for rejected_txids_chunk in rejected_txids.chunks(BATCH_SIZE) {
        let sql = "DELETE FROM transactions_inputs WHERE transaction_id = ANY($1)";
        rows_affected += sqlx::query(sql).bind(rejected_txids_chunk).execute(tx.as_mut()).await?.rows_affected();
        trace!("prune_transactions: Deleted {rows_affected} expired rejected transactions_inputs")
    }
    debug!("prune_transactions: Deleted {rows_affected} expired rejected transactions_inputs");
    total_rows_affected += rows_affected;

    // Delete rejected transaction outputs
    rows_affected = 0;
    for rejected_txids_chunk in rejected_txids.chunks(BATCH_SIZE) {
        let sql = "DELETE FROM transactions_outputs WHERE transaction_id = ANY($1)";
        rows_affected += sqlx::query(sql).bind(rejected_txids_chunk).execute(tx.as_mut()).await?.rows_affected();
        trace!("prune_transactions: Deleted {rows_affected} expired rejected transactions_outputs")
    }
    debug!("prune_transactions: Deleted {rows_affected} expired rejected transactions_outputs");
    total_rows_affected += rows_affected;

    // Find spent transaction outputs while deleting transaction_inputs
    let mut spent_tx_outputs: Vec<(Hash, i16)> = vec![];
    for accepted_txids_chunk in accepted_txids.into_iter().collect::<Vec<_>>().chunks(BATCH_SIZE) {
        let sql = "
            DELETE FROM transactions_inputs WHERE transaction_id = ANY($1)
            RETURNING previous_outpoint_hash, previous_outpoint_index
            ";
        spent_tx_outputs.extend(sqlx::query_as::<_, (Hash, i16)>(sql).bind(accepted_txids_chunk).fetch_all(tx.as_mut()).await?);
        trace!("prune_transactions: Deleted {} expired transactions_inputs", spent_tx_outputs.len());
    }
    debug!("prune_transactions: Deleted {} expired transactions_inputs", spent_tx_outputs.len());
    total_rows_affected += spent_tx_outputs.len() as u64;

    // Delete spent transaction outputs
    rows_affected = 0;
    for spent_tx_outputs_chunk in spent_tx_outputs.chunks(BATCH_SIZE) {
        let sql = "DELETE FROM transactions_outputs WHERE (transaction_id, index) IN (SELECT * FROM UNNEST($1, $2))";
        let (t, i): (Vec<_>, Vec<_>) = spent_tx_outputs_chunk.iter().cloned().unzip();
        rows_affected += sqlx::query(&sql).bind(t).bind(i).execute(tx.as_mut()).await?.rows_affected();
        trace!("prune_transactions: Deleted {rows_affected} expired spent transactions_outputs")
    }
    debug!("prune_transactions: Deleted {rows_affected} expired spent transactions_outputs");
    total_rows_affected += rows_affected;

    // Find fully spent transactions
    let possibly_spent_txids: HashSet<_> = spent_tx_outputs.into_iter().map(|(t, _)| t).collect();
    let mut unspent_txids = HashSet::new();
    for possibly_spent_txids_chunk in possibly_spent_txids.iter().collect::<Vec<_>>().chunks(BATCH_SIZE) {
        let sql = "SELECT transaction_id FROM transactions_outputs WHERE transaction_id = ANY($1)";
        unspent_txids.extend(sqlx::query_scalar::<_, Hash>(sql).bind(possibly_spent_txids_chunk).fetch_all(tx.as_mut()).await?);
    }
    let fully_spent_txids: Vec<Hash> = possibly_spent_txids.iter().filter(|id| !unspent_txids.contains(id)).cloned().collect();
    debug!("prune_transactions: Found {} expired fully spent transactions", fully_spent_txids.len());

    // Delete acceptances for fully spent transactions
    rows_affected = 0;
    for fully_spent_txids_chunk in fully_spent_txids.chunks(BATCH_SIZE) {
        let sql = "DELETE FROM transactions_acceptances WHERE transaction_id = ANY($1)";
        rows_affected += sqlx::query(sql).bind(fully_spent_txids_chunk).execute(tx.as_mut()).await?.rows_affected();
        trace!("prune_transactions: Pruned {rows_affected} expired spent transactions_acceptances")
    }
    debug!("prune_transactions: Pruned {rows_affected} expired spent transactions_acceptances");
    total_rows_affected += rows_affected;

    tx.commit().await?;
    Ok(total_rows_affected)
}

pub async fn prune_addresses_transactions(block_time_lt: i64, pool: &Pool<Postgres>) -> Result<u64, Error> {
    Ok(sqlx::query("DELETE FROM addresses_transactions WHERE block_time < $1")
        .bind(block_time_lt)
        .execute(pool)
        .await?
        .rows_affected())
}

pub async fn prune_scripts_transactions(block_time_lt: i64, pool: &Pool<Postgres>) -> Result<u64, Error> {
    Ok(sqlx::query("DELETE FROM scripts_transactions WHERE block_time < $1").bind(block_time_lt).execute(pool).await?.rows_affected())
}
