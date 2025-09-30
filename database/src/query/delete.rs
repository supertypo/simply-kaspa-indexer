use crate::models::types::hash::Hash;
use log::debug;
use sqlx::{Error, Pool, Postgres};
use std::collections::HashSet;

pub async fn delete_transaction_acceptances(block_hashes: &[Hash], pool: &Pool<Postgres>) -> Result<u64, Error> {
    Ok(sqlx::query("DELETE FROM transactions_acceptances WHERE block_hash = ANY($1)")
        .bind(block_hashes)
        .execute(pool)
        .await?
        .rows_affected())
}

pub async fn prune_block_parent(block_time_lt: i64, batch_size: i32, pool: &Pool<Postgres>) -> Result<u64, Error> {
    let sql = r#"
        DELETE FROM block_parent
        WHERE ctid IN (
            SELECT bp.ctid
            FROM block_parent bp
            JOIN blocks b ON bp.block_hash = b.hash
            WHERE b.timestamp < $1
            LIMIT $2
        )
    "#;
    let mut total_rows_affected: u64 = 0;
    loop {
        let rows_affected = sqlx::query(sql).bind(block_time_lt).bind(batch_size).execute(pool).await?.rows_affected();
        if rows_affected == 0 {
            break;
        }
        total_rows_affected += rows_affected;
    }
    Ok(total_rows_affected)
}

pub async fn prune_blocks_transactions_using_blocks(block_time_lt: i64, batch_size: i32, pool: &Pool<Postgres>) -> Result<u64, Error> {
    let sql = r#"
        DELETE FROM blocks_transactions
        WHERE ctid IN (
            SELECT bt.ctid
            FROM blocks_transactions bt
            JOIN blocks b ON bt.block_hash = b.hash
            WHERE b.timestamp < $1
            LIMIT $2
        )
    "#;
    let mut total_rows_affected: u64 = 0;
    loop {
        let rows_affected = sqlx::query(sql).bind(block_time_lt).bind(batch_size).execute(pool).await?.rows_affected();
        if rows_affected == 0 {
            break;
        }
        total_rows_affected += rows_affected;
    }
    Ok(total_rows_affected)
}

pub async fn prune_blocks_transactions_using_transactions(
    block_time_lt: i64,
    batch_size: i32,
    pool: &Pool<Postgres>,
) -> Result<u64, Error> {
    let sql = r#"
        DELETE FROM blocks_transactions
        WHERE ctid IN (
            SELECT bt.ctid
            FROM blocks_transactions bt
            JOIN transactions t ON bt.transaction_id = t.transaction_id
            WHERE t.block_time < $1
            LIMIT $2
        )
    "#;
    let mut total_rows_affected: u64 = 0;
    loop {
        let rows_affected = sqlx::query(sql).bind(block_time_lt).bind(batch_size).execute(pool).await?.rows_affected();
        if rows_affected == 0 {
            break;
        }
        total_rows_affected += rows_affected;
    }
    Ok(total_rows_affected)
}

pub async fn prune_transactions_acceptances_using_blocks(
    block_time_lt: i64,
    batch_size: i32,
    pool: &Pool<Postgres>,
) -> Result<u64, Error> {
    let sql = r#"
        DELETE FROM transactions_acceptances
        WHERE ctid IN (
            SELECT ta.ctid
            FROM transactions_acceptances ta
            JOIN blocks b ON ta.block_hash = b.hash
            WHERE b.timestamp < $1
            LIMIT $2
        )
    "#;
    let mut total_rows_affected: u64 = 0;
    loop {
        let rows_affected = sqlx::query(sql).bind(block_time_lt).bind(batch_size).execute(pool).await?.rows_affected();
        if rows_affected == 0 {
            break;
        }
        total_rows_affected += rows_affected;
    }
    Ok(total_rows_affected)
}

pub async fn prune_blocks(block_time_lt: i64, batch_size: i32, pool: &Pool<Postgres>) -> Result<u64, Error> {
    let sql = r#"
        DELETE FROM blocks
        WHERE ctid IN (
            SELECT b.ctid
            FROM blocks b
            WHERE b.timestamp < $1
            LIMIT $2
        )
    "#;
    let mut total_rows_affected: u64 = 0;
    loop {
        let rows_affected = sqlx::query(sql).bind(block_time_lt).bind(batch_size).execute(pool).await?.rows_affected();
        if rows_affected == 0 {
            break;
        }
        total_rows_affected += rows_affected;
    }
    Ok(total_rows_affected)
}

pub async fn prune_transactions(block_time_lt: i64, batch_size: i32, pool: &Pool<Postgres>) -> Result<u64, Error> {
    let mut total_rows_affected = 0;
    loop {
        let rows_affected = prune_transactions_chunk(block_time_lt, batch_size, pool).await?;
        if rows_affected == 0 {
            break;
        }
        total_rows_affected += rows_affected;
    }
    Ok(total_rows_affected)
}

pub async fn prune_transactions_chunk(block_time_lt: i64, batch_size: i32, pool: &Pool<Postgres>) -> Result<u64, Error> {
    let mut total_rows_affected = 0;
    let mut tx = pool.begin().await?;

    // Force disable seqscan, as it will never be a good idea when pruning
    sqlx::query("SET LOCAL enable_seqscan = off").execute(tx.as_mut()).await?;

    // Find & delete old transactions
    let sql = "DELETE FROM transactions WHERE ctid IN (SELECT t.ctid FROM transactions t WHERE t.block_time < $1 LIMIT $2) RETURNING transaction_id";
    let expired_txids = sqlx::query_scalar::<_, Hash>(sql).bind(block_time_lt).bind(batch_size).fetch_all(tx.as_mut()).await?;
    total_rows_affected += expired_txids.len() as u64;
    debug!("prune_transactions: Found & deleted {} expired transactions", expired_txids.len());

    // Find rejected transactions
    let sql = "SELECT transaction_id FROM transactions_acceptances WHERE transaction_id = ANY($1)";
    let accepted_txids: HashSet<_> =
        sqlx::query_scalar::<_, Hash>(sql).bind(&expired_txids).fetch_all(tx.as_mut()).await?.into_iter().collect();
    let rejected_txids: Vec<_> = expired_txids.iter().filter(|id| !accepted_txids.contains(id)).cloned().collect();
    debug!("prune_transactions: Found {} expired rejected transactions", rejected_txids.len());

    // Delete rejected transaction inputs
    let sql = "DELETE FROM transactions_inputs WHERE transaction_id = ANY($1)";
    let rows_affected = sqlx::query(sql).bind(&rejected_txids).execute(tx.as_mut()).await?.rows_affected();
    debug!("prune_transactions: Deleted {rows_affected} expired rejected transactions_inputs");
    total_rows_affected += rows_affected;

    // Delete rejected transaction outputs
    let sql = "DELETE FROM transactions_outputs WHERE transaction_id = ANY($1)";
    let rows_affected = sqlx::query(sql).bind(&rejected_txids).execute(tx.as_mut()).await?.rows_affected();
    debug!("prune_transactions: Deleted {rows_affected} expired rejected transactions_outputs");
    total_rows_affected += rows_affected;

    // Find spent transaction outputs while deleting transaction_inputs
    let sql =
        "DELETE FROM transactions_inputs WHERE transaction_id = ANY($1) RETURNING previous_outpoint_hash, previous_outpoint_index";
    let spent_tx_outputs: Vec<_> =
        sqlx::query_as::<_, (Hash, i16)>(sql).bind(accepted_txids.iter().collect::<Vec<_>>()).fetch_all(tx.as_mut()).await?;
    debug!("prune_transactions: Deleted {} expired transactions_inputs", spent_tx_outputs.len());
    total_rows_affected += spent_tx_outputs.len() as u64;

    // Delete spent transaction outputs
    let sql = "DELETE FROM transactions_outputs WHERE (transaction_id, index) IN (SELECT * FROM UNNEST($1, $2))";
    let (t, i): (Vec<_>, Vec<_>) = spent_tx_outputs.iter().cloned().unzip();
    let rows_affected = sqlx::query(sql).bind(t).bind(i).execute(tx.as_mut()).await?.rows_affected();
    debug!("prune_transactions: Deleted {rows_affected} expired spent transactions_outputs");
    total_rows_affected += rows_affected;

    // Find fully spent transactions
    let possibly_spent_txids: HashSet<_> = spent_tx_outputs.into_iter().map(|(t, _)| t).collect();
    let sql = "SELECT transaction_id FROM transactions_outputs WHERE transaction_id = ANY($1)";
    let unspent_txids: HashSet<_> = sqlx::query_scalar::<_, Hash>(sql)
        .bind(possibly_spent_txids.iter().collect::<Vec<_>>())
        .fetch_all(tx.as_mut())
        .await?
        .into_iter()
        .collect();
    let fully_spent_txids: Vec<_> = possibly_spent_txids.iter().filter(|id| !unspent_txids.contains(id)).cloned().collect();
    debug!("prune_transactions: Found {} expired fully spent transactions", fully_spent_txids.len());

    // Delete acceptances for fully spent transactions
    let sql = "DELETE FROM transactions_acceptances WHERE transaction_id = ANY($1)";
    let rows_affected = sqlx::query(sql).bind(&fully_spent_txids).execute(tx.as_mut()).await?.rows_affected();
    debug!("prune_transactions: Pruned {rows_affected} expired spent transactions_acceptances");
    total_rows_affected += rows_affected;

    tx.commit().await?;
    Ok(total_rows_affected)
}

pub async fn prune_addresses_transactions(block_time_lt: i64, batch_size: i32, pool: &Pool<Postgres>) -> Result<u64, Error> {
    let sql = r#"
        DELETE FROM addresses_transactions
        WHERE ctid IN (
            SELECT a.ctid
            FROM addresses_transactions a
            WHERE a.block_time < $1
            LIMIT $2
        )
    "#;
    let mut total_rows_affected: u64 = 0;
    loop {
        let rows_affected = sqlx::query(sql).bind(block_time_lt).bind(batch_size).execute(pool).await?.rows_affected();
        if rows_affected == 0 {
            break;
        }
        total_rows_affected += rows_affected;
    }
    Ok(total_rows_affected)
}

pub async fn prune_scripts_transactions(block_time_lt: i64, batch_size: i32, pool: &Pool<Postgres>) -> Result<u64, Error> {
    let sql = r#"
        DELETE FROM scripts_transactions
        WHERE ctid IN (
            SELECT s.ctid
            FROM scripts_transactions s
            WHERE s.block_time < $1
            LIMIT $2
        )
    "#;
    let mut total_rows_affected: u64 = 0;
    loop {
        let rows_affected = sqlx::query(sql).bind(block_time_lt).bind(batch_size).execute(pool).await?.rows_affected();
        if rows_affected == 0 {
            break;
        }
        total_rows_affected += rows_affected;
    }
    Ok(total_rows_affected)
}
