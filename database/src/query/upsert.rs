use log::trace;
use sqlx::{Error, Pool, Postgres};
use crate::models::transaction_output::TransactionOutput;
use crate::models::types::hash::Hash;

use crate::query::common::generate_placeholders;

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

pub async fn upsert_utxos(
    transaction_outputs: &[(Hash, i16, Option<i32>, TransactionOutput)],
    pool: &Pool<Postgres>,
) -> Result<u64, Error> {
    const COLS: usize = 6;
    let sql = format!(
        "INSERT INTO transactions (transaction_id, block_time, outputs)
        SELECT v.txid, v.block_time,
            array_fill(NULL::transactions_outputs, ARRAY[v.idx])
                || ARRAY[ROW(v.amount, v.script_public_key, v.script_public_key_address)::transactions_outputs]
        FROM (VALUES {}) AS v(txid, block_time, idx, amount, script_public_key, script_public_key_address)
        ON CONFLICT (transaction_id) DO UPDATE
        SET outputs[v.idx+1] = ROW(v.amount, v.script_public_key, v.script_public_key_address)::transactions_outputs",
        generate_placeholders(transaction_outputs.len(), COLS)
    );
    let mut query = sqlx::query(&sql);
    for (transaction_id, idx, block_time, tout) in transaction_outputs {
        query = query.bind(transaction_id);
        query = query.bind(block_time);
        query = query.bind(idx);
        query = query.bind(tout.amount);
        query = query.bind(&tout.script_public_key);
        query = query.bind(&tout.script_public_key_address);
    }
    Ok(query.execute(pool).await?.rows_affected())
}
