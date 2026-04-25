use crate::models::types::hash::Hash;
use log::debug;
use sqlx::{Error, Pool, Postgres};

pub async fn delete_transaction_acceptances(block_hashes: &[Hash], pool: &Pool<Postgres>) -> Result<u64, Error> {
    Ok(sqlx::query("DELETE FROM transactions_acceptances WHERE block_hash = ANY($1)")
        .bind(block_hashes)
        .execute(pool)
        .await?
        .rows_affected())
}

pub async fn prune_block_parent(blue_score_lt: i64, batch_size: i32, pool: &Pool<Postgres>) -> Result<u64, Error> {
    let sql = r#"
        DELETE FROM block_parent
        WHERE ctid IN (
            SELECT bp.ctid
            FROM block_parent bp
            JOIN blocks b ON bp.block_hash = b.hash
            WHERE b.blue_score < $1
            LIMIT $2
        )
    "#;
    let mut total_rows_affected: u64 = 0;
    loop {
        let rows_affected = sqlx::query(sql).bind(blue_score_lt).bind(batch_size).execute(pool).await?.rows_affected();
        if rows_affected == 0 {
            break;
        }
        debug!("prune_block_parent: Deleted {rows_affected} expired block_parent rows");
        total_rows_affected += rows_affected;
    }
    Ok(total_rows_affected)
}

pub async fn prune_transactions_acceptances_using_blocks(
    blue_score_lt: i64,
    batch_size: i32,
    pool: &Pool<Postgres>,
) -> Result<u64, Error> {
    let sql = r#"
        DELETE FROM transactions_acceptances
        WHERE ctid IN (
            SELECT ta.ctid
            FROM transactions_acceptances ta
            JOIN blocks b ON ta.block_hash = b.hash
            WHERE b.blue_score < $1
            LIMIT $2
        )
    "#;
    let mut total_rows_affected: u64 = 0;
    loop {
        let rows_affected = sqlx::query(sql).bind(blue_score_lt).bind(batch_size).execute(pool).await?.rows_affected();
        if rows_affected == 0 {
            break;
        }
        debug!("prune_transactions_acceptances_using_blocks: Deleted {rows_affected} expired transactions_acceptances rows");
        total_rows_affected += rows_affected;
    }
    Ok(total_rows_affected)
}

pub async fn prune_transactions_acceptances_using_transactions(
    block_time_lt: i64,
    batch_size: i32,
    pool: &Pool<Postgres>,
) -> Result<u64, Error> {
    let sql = r#"
        DELETE FROM transactions_acceptances
        WHERE ctid IN (
            SELECT ta.ctid
            FROM transactions_acceptances ta
            JOIN transactions t ON ta.transaction_id = t.transaction_id
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
        debug!("prune_transactions_acceptances_using_transactions: Deleted {rows_affected} expired transactions_acceptances rows");
        total_rows_affected += rows_affected;
    }
    Ok(total_rows_affected)
}

pub async fn prune_blocks(blue_score_lt: i64, batch_size: i32, pool: &Pool<Postgres>) -> Result<u64, Error> {
    let sql = r#"
        DELETE FROM blocks
        WHERE ctid IN (
            SELECT b.ctid
            FROM blocks b
            WHERE b.blue_score < $1
            LIMIT $2
        )
    "#;
    let mut total_rows_affected: u64 = 0;
    loop {
        let rows_affected = sqlx::query(sql).bind(blue_score_lt).bind(batch_size).execute(pool).await?.rows_affected();
        if rows_affected == 0 {
            break;
        }
        debug!("prune_blocks: Deleted {rows_affected} expired blocks rows");
        total_rows_affected += rows_affected;
    }
    Ok(total_rows_affected)
}

pub async fn prune_transactions(block_time_lt: i64, batch_size: i32, pool: &Pool<Postgres>) -> Result<u64, Error> {
    let sql = r#"
        DELETE FROM transactions
        WHERE ctid IN (
            SELECT t.ctid
            FROM transactions t
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
        debug!("prune_transactions: Deleted {rows_affected} expired transactions");
        total_rows_affected += rows_affected;
    }
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
        debug!("prune_addresses_transactions: Deleted {rows_affected} expired addresses_transactions rows");
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
        debug!("prune_scripts_transactions: Deleted {rows_affected} expired scripts_transactions rows");
        total_rows_affected += rows_affected;
    }
    Ok(total_rows_affected)
}
