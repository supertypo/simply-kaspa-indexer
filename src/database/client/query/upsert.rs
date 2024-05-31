use log::trace;
use sqlx::{Error, Pool, Postgres};

pub async fn upsert_var(key: &str, value: &String, pool: &Pool<Postgres>) -> Result<u64, Error> {
    trace!("Saving database var with key '{}' value: {}", key, value);
    let rows_affected =
        sqlx::query("INSERT INTO vars (key, value) VALUES ($1, $2) ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value")
            .bind(key)
            .bind(value)
            .execute(pool)
            .await?
            .rows_affected();
    Ok(rows_affected)
}
