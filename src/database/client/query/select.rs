use sqlx::{Error, Pool, Postgres, Row};

use crate::database::models::subnetwork::Subnetwork;

pub async fn select_var(key: &str, pool: &Pool<Postgres>) -> Result<String, Error> {
    return sqlx::query("SELECT value FROM vars WHERE key = $1").bind(key).fetch_one(pool).await?.try_get(0);
}

pub async fn select_subnetworks(pool: &Pool<Postgres>) -> Result<Vec<Subnetwork>, Error> {
    let rows = sqlx::query("SELECT id, subnetwork_id FROM subnetworks").fetch_all(pool).await?;
    let subnetworks = rows.iter().map(|row| Subnetwork {
        id: row.get("id"),
        subnetwork_id: row.get("subnetwork_id"),
    }).collect();
    Ok(subnetworks)
}

pub async fn select_tx_count(block_hash: &Vec<u8>, pool: &Pool<Postgres>) -> Result<i64, Error> {
    sqlx::query("SELECT COUNT(*) FROM blocks_transactions WHERE block_hash = $1").bind(block_hash).fetch_one(pool).await?.try_get(0)
}

pub async fn select_is_chain_block(block_hash: &Vec<u8>, pool: &Pool<Postgres>) -> Result<bool, Error> {
    sqlx::query("SELECT EXISTS(SELECT 1 FROM chain_blocks WHERE block_hash = $1)").bind(block_hash).fetch_one(pool).await?.try_get(0)
}
