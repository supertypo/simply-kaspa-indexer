use crate::models::types::hash::Hash;
use sqlx::{Error, Pool, Postgres};

pub async fn delete_transaction_acceptances(block_hashes: &[Hash], pool: &Pool<Postgres>) -> Result<u64, Error> {
    Ok(sqlx::query("DELETE FROM transactions_acceptances WHERE block_hash = ANY($1)")
        .bind(block_hashes)
        .execute(pool)
        .await?
        .rows_affected())
}

pub async fn delete_old_block_parents(block_time_lt: i64, pool: &Pool<Postgres>) -> Result<u64, Error> {
    let sql = "DELETE FROM block_parent bp USING blocks b WHERE bp.block_hash = b.hash AND b.timestamp < $1";
    Ok(sqlx::query(sql).bind(block_time_lt).execute(pool).await?.rows_affected())
}

pub async fn delete_old_blocks_transactions(block_time_lt: i64, pool: &Pool<Postgres>) -> Result<u64, Error> {
    let sql = "DELETE FROM blocks_transactions bt USING blocks b WHERE bt.block_hash = b.hash AND b.timestamp < $1";
    Ok(sqlx::query(sql).bind(block_time_lt).execute(pool).await?.rows_affected())
}

pub async fn delete_old_blocks(block_time_lt: i64, pool: &Pool<Postgres>) -> Result<u64, Error> {
    Ok(sqlx::query("DELETE FROM blocks WHERE timestamp < $1").bind(block_time_lt).execute(pool).await?.rows_affected())
}

pub async fn delete_old_transactions_acceptances(block_time_lt: i64, pool: &Pool<Postgres>) -> Result<u64, Error> {
    let sql = "DELETE FROM transactions_acceptances ta USING transactions t WHERE t.transaction_id = ta.transaction_id AND t.block_time < $1";
    Ok(sqlx::query(sql).bind(block_time_lt).execute(pool).await?.rows_affected())
}

pub async fn delete_old_transactions_outputs(block_time_lt: i64, pool: &Pool<Postgres>) -> Result<u64, Error> {
    let sql = "
        DELETE FROM transactions_outputs to_
        USING transactions t,
              transactions_inputs ti
        WHERE to_.transaction_id = t.transaction_id
          AND t.block_time < $1
          AND ti.previous_outpoint_hash = to_.transaction_id
          AND ti.previous_outpoint_index = to_.index;
        ";
    Ok(sqlx::query(sql).bind(block_time_lt).execute(pool).await?.rows_affected())
}

pub async fn delete_old_transactions_inputs(block_time_lt: i64, pool: &Pool<Postgres>) -> Result<u64, Error> {
    let sql =
        "DELETE FROM transactions_inputs ti USING transactions t WHERE t.transaction_id = ti.transaction_id AND t.block_time < $1";
    Ok(sqlx::query(sql).bind(block_time_lt).execute(pool).await?.rows_affected())
}

pub async fn delete_old_transactions(block_time_lt: i64, pool: &Pool<Postgres>) -> Result<u64, Error> {
    Ok(sqlx::query("DELETE FROM transactions WHERE block_time < $1").bind(block_time_lt).execute(pool).await?.rows_affected())
}

pub async fn delete_old_addresses_transactions(block_time_lt: i64, pool: &Pool<Postgres>) -> Result<u64, Error> {
    Ok(sqlx::query("DELETE FROM addresses_transactions WHERE block_time < $1").bind(block_time_lt).execute(pool).await?.rows_affected())
}

pub async fn delete_old_scripts_transactions(block_time_lt: i64, pool: &Pool<Postgres>) -> Result<u64, Error> {
    Ok(sqlx::query("DELETE FROM scripts_transactions WHERE block_time < $1").bind(block_time_lt).execute(pool).await?.rows_affected())
}
