use sqlx::{Error, Pool, Postgres, Row};

pub async fn select_var(key: &str, pool: &Pool<Postgres>) -> Result<String, Error> {
    return sqlx::query("SELECT value FROM vars WHERE key = $1").bind(key).fetch_one(pool).await?.try_get(0);
}

pub async fn select_tx_count(block_hash: &Vec<u8>, pool: &Pool<Postgres>) -> Result<i64, Error> {
    sqlx::query("SELECT COUNT(*) FROM blocks_transactions WHERE block_hash = $1").bind(block_hash).fetch_one(pool).await?.try_get(0)
}

pub async fn select_is_chain_block(block_hash: &Vec<u8>, pool: &Pool<Postgres>) -> Result<bool, Error> {
    sqlx::query("SELECT EXISTS(SELECT 1 FROM chain_blocks WHERE block_hash = $1)").bind(block_hash).fetch_one(pool).await?.try_get(0)
}
