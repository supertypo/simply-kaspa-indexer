use crate::database::models::{VAR_KEY_BLOCK_CHECKPOINT, VAR_KEY_LEGACY_CHECKPOINT};
use log::trace;
use sqlx::{Postgres, Row};

pub async fn load_block_checkpoint(db_pool: &sqlx::Pool<Postgres>) -> Option<String> {
    load(VAR_KEY_BLOCK_CHECKPOINT, db_pool).await.or(load(VAR_KEY_LEGACY_CHECKPOINT, db_pool).await)
}

pub async fn save_checkpoint(block_hash: &String, db_pool: &sqlx::Pool<Postgres>) {
    save(VAR_KEY_BLOCK_CHECKPOINT, block_hash, db_pool).await;
}

pub async fn load(key: &str, db_pool: &sqlx::Pool<Postgres>) -> Option<String> {
    return sqlx::query("SELECT value FROM vars WHERE key = $1")
        .bind(key)
        .fetch_optional(db_pool)
        .await
        .expect(format!("Loading var '{}' from database FAILED", key).as_str())
        .map(|row| row.get(0));
}

pub async fn save(key: &str, value: &String, db_pool: &sqlx::Pool<Postgres>) {
    trace!("Saving database var with key '{}' value: {}", key, value);
    sqlx::query("INSERT INTO vars (key, value) VALUES ($1, $2) ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value")
        .bind(key)
        .bind(value)
        .execute(db_pool)
        .await
        .expect(format!("Saving var {} FAILED", key).as_str());
}
