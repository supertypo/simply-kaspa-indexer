use sqlx::{Error, Pool, Postgres};

pub async fn delete_chain_blocks(block_hashes: &[Vec<u8>], pool: &Pool<Postgres>) -> Result<u64, Error> {
    Ok(sqlx::query("DELETE FROM chain_blocks WHERE block_hash = ANY($1)").bind(block_hashes).execute(pool).await?.rows_affected())
}

pub async fn delete_transaction_acceptances(block_hashes: &[Vec<u8>], pool: &Pool<Postgres>) -> Result<u64, Error> {
    Ok(sqlx::query("DELETE FROM transactions_acceptances WHERE block_hash = ANY($1)")
        .bind(&block_hashes)
        .execute(pool)
        .await?
        .rows_affected())
}
