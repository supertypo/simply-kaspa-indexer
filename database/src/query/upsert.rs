use crate::models::transaction_output::TransactionOutput;
use crate::models::types::hash::Hash;
use log::trace;
use sqlx::{Error, Pool, Postgres};

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
    transaction_outputs: &[(Hash, Option<i64>, i16, TransactionOutput)],
    pool: &Pool<Postgres>,
) -> Result<u64, Error> {
    let sql = format!(
        "INSERT INTO transactions (transaction_id, block_time, outputs)
        SELECT v.transaction_id, v.block_time, ARRAY[]::transactions_outputs[]
        FROM (VALUES {}) AS v(transaction_id, block_time)
        ON CONFLICT DO NOTHING",
        generate_placeholders(transaction_outputs.len(), 2)
    );
    let mut query = sqlx::query(&sql);
    for (transaction_id, block_time, ..) in transaction_outputs {
        query = query.bind(transaction_id).bind(block_time);
    }
    query.execute(pool).await?;

    let sql = format!(
        "UPDATE transactions t
        SET outputs[v.idx+1] = ROW(v.amount, v.script_public_key, v.script_public_key_address)::transactions_outputs
        FROM (VALUES {}) AS v(transaction_id, idx, amount, script_public_key, script_public_key_address)
        WHERE t.transaction_id = v.transaction_id",
        generate_placeholders(transaction_outputs.len(), 5)
    );
    let mut query = sqlx::query(&sql);
    for (transaction_id, _, idx, tout) in transaction_outputs {
        query = query.bind(transaction_id);
        query = query.bind(idx);
        query = query.bind(tout.amount);
        query = query.bind(&tout.script_public_key);
        query = query.bind(&tout.script_public_key_address);
    }
    Ok(query.execute(pool).await?.rows_affected())
}
