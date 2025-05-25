use crate::models::types::hash::Hash;
use sqlx::{Error, Executor, Pool, Postgres};

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
