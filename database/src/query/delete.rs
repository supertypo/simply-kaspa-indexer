use crate::models::types::hash::Hash;
use sqlx::{Error, Pool, Postgres};

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

pub async fn prune_transactions_acceptances_using_transactions(block_time_lt: i64, pool: &Pool<Postgres>) -> Result<u64, Error> {
    let sql = "DELETE FROM transactions_acceptances ta USING transactions t WHERE t.transaction_id = ta.transaction_id AND t.block_time < $1";
    Ok(sqlx::query(sql).bind(block_time_lt).execute(pool).await?.rows_affected())
}

pub async fn prune_unspendable_transactions_outputs(block_time_lt: i64, pool: &Pool<Postgres>) -> Result<u64, Error> {
    let sql = "
        WITH relevant_transactions AS MATERIALIZED (
          SELECT transaction_id
          FROM transactions
          WHERE block_time < 747540800000
        ), rejected AS MATERIALIZED (
          SELECT transaction_id
          FROM relevant_transactions rt
          WHERE NOT EXISTS (
            SELECT 1 FROM transactions_acceptances ta
            WHERE ta.transaction_id = rt.transaction_id
          )
        ), spent AS MATERIALIZED (
          SELECT to_.transaction_id, to_.index
          FROM transactions_outputs to_
          JOIN relevant_transactions rt ON rt.transaction_id = to_.transaction_id
          WHERE EXISTS (
            SELECT 1
            FROM transactions_inputs ti
            WHERE ti.previous_outpoint_hash = rt.transaction_id
            AND ti.previous_outpoint_index = to_.index
          )
        )
        DELETE FROM transactions_outputs to_
        USING rejected, spent
        WHERE to_.transaction_id = rejected.transaction_id
        OR (to_.transaction_id = spent.transaction_id AND to_.index = spent.index)
        ";
    Ok(sqlx::query(sql).bind(block_time_lt).execute(pool).await?.rows_affected())
}

pub async fn prune_transactions_inputs(block_time_lt: i64, pool: &Pool<Postgres>) -> Result<u64, Error> {
    let sql =
        "DELETE FROM transactions_inputs ti USING transactions t WHERE t.transaction_id = ti.transaction_id AND t.block_time < $1";
    Ok(sqlx::query(sql).bind(block_time_lt).execute(pool).await?.rows_affected())
}

pub async fn prune_transactions(block_time_lt: i64, pool: &Pool<Postgres>) -> Result<u64, Error> {
    Ok(sqlx::query("DELETE FROM transactions WHERE block_time < $1").bind(block_time_lt).execute(pool).await?.rows_affected())
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
