use sqlx::{Error, Executor, Pool, Postgres, Row};

use crate::models::address_transaction::AddressTransaction;
use crate::models::block::Block;
use crate::models::block_parent::BlockParent;
use crate::models::block_transaction::BlockTransaction;
use crate::models::script_transaction::ScriptTransaction;
use crate::models::transaction::Transaction;
use crate::models::transaction_acceptance::TransactionAcceptance;
use crate::models::types::hash::Hash;
use crate::models::utxo::Utxo;
use crate::query::common::generate_placeholders;

pub async fn insert_subnetwork(subnetwork_id: &String, pool: &Pool<Postgres>) -> Result<i32, Error> {
    sqlx::query("INSERT INTO subnetworks (subnetwork_id) VALUES ($1) ON CONFLICT DO NOTHING RETURNING id")
        .bind(subnetwork_id)
        .fetch_one(pool)
        .await?
        .try_get(0)
}

pub async fn insert_utxos(utxos: &[Utxo], pool: &Pool<Postgres>) -> Result<u64, Error> {
    const COLS: usize = 5;
    let sql = format!(
        "INSERT INTO utxos (transaction_id, index, amount, script_public_key, script_public_key_address)
        VALUES {} ON CONFLICT DO NOTHING",
        generate_placeholders(utxos.len(), COLS)
    );
    let mut query = sqlx::query(&sql);
    for utxo in utxos {
        query = query.bind(&utxo.transaction_id);
        query = query.bind(utxo.index);
        query = query.bind(utxo.amount);
        query = query.bind(&utxo.script_public_key);
        query = query.bind(&utxo.script_public_key_address);
    }
    Ok(query.execute(pool).await?.rows_affected())
}

pub async fn insert_utxos_to_transactions(pool: &Pool<Postgres>) -> Result<u64, Error> {
    let sql = "
        INSERT INTO transactions (transaction_id, outputs)
        SELECT transaction_id,
            array_agg(ROW(index, amount, script_public_key, script_public_key_address)::transactions_outputs ORDER BY index)
        FROM utxos GROUP BY transaction_id";
    Ok(sqlx::query(sql).execute(pool).await?.rows_affected())
}

pub async fn insert_blocks(blocks: &[Block], pool: &Pool<Postgres>) -> Result<u64, Error> {
    const COLS: usize = 15;
    let mut tx = pool.begin().await?;

    let sql = format!(
        "INSERT INTO blocks (hash, accepted_id_merkle_root, merge_set_blues_hashes, merge_set_reds_hashes,
            selected_parent_hash, bits, blue_score, blue_work, daa_score, hash_merkle_root, nonce, pruning_point,
            timestamp, utxo_commitment, version
        ) VALUES {} ON CONFLICT DO NOTHING",
        generate_placeholders(blocks.len(), COLS)
    );

    let mut query = sqlx::query(&sql);
    for block in blocks {
        query = query.bind(&block.hash);
        query = query.bind(&block.accepted_id_merkle_root);
        query = query.bind(&block.merge_set_blues_hashes);
        query = query.bind(&block.merge_set_reds_hashes);
        query = query.bind(&block.selected_parent_hash);
        query = query.bind(block.bits);
        query = query.bind(block.blue_score);
        query = query.bind(&block.blue_work);
        query = query.bind(block.daa_score);
        query = query.bind(&block.hash_merkle_root);
        query = query.bind(&block.nonce);
        query = query.bind(&block.pruning_point);
        query = query.bind(block.timestamp);
        query = query.bind(&block.utxo_commitment);
        query = query.bind(block.version);
    }
    let rows_affected = tx.execute(query).await?.rows_affected();
    tx.commit().await?;
    Ok(rows_affected)
}

pub async fn insert_block_parents(block_parents: &[BlockParent], pool: &Pool<Postgres>) -> Result<u64, Error> {
    const COLS: usize = 2;
    let sql = format!(
        "INSERT INTO block_parent (block_hash, parent_hash)
        VALUES {} ON CONFLICT DO NOTHING",
        generate_placeholders(block_parents.len(), COLS)
    );
    let mut query = sqlx::query(&sql);
    for block_transaction in block_parents {
        query = query.bind(&block_transaction.block_hash);
        query = query.bind(&block_transaction.parent_hash);
    }
    Ok(query.execute(pool).await?.rows_affected())
}

pub async fn insert_transactions(
    resolve_previous_outpoints: bool,
    transactions: &[Transaction],
    pool: &Pool<Postgres>,
) -> Result<u64, Error> {
    const COLS: usize = 8;
    let sql = if resolve_previous_outpoints {
        format!(
            "INSERT INTO transactions (transaction_id, subnetwork_id, hash, mass, payload, block_time, inputs, outputs)
             SELECT v.transaction_id, v.subnetwork_id, v.hash, v.mass, v.payload, v.block_time,
               ARRAY(
                 SELECT ROW(
                   i.index,
                   i.previous_outpoint_hash,
                   i.previous_outpoint_index,
                   i.signature_script,
                   i.sig_op_count,
                   COALESCE(i.previous_outpoint_script, o.script_public_key),
                   COALESCE(i.previous_outpoint_amount, o.amount)
                 )::transactions_inputs
                 FROM UNNEST(v.inputs) AS i
                 LEFT JOIN transactions output_t ON output_t.transaction_id = i.previous_outpoint_hash
                 LEFT JOIN LATERAL (
                   SELECT amount, script_public_key
                   FROM UNNEST(output_t.outputs)
                   WHERE index = i.previous_outpoint_index
                   LIMIT 1
                 ) o ON true
               ),
               v.outputs
             FROM (VALUES {}) AS v(transaction_id, subnetwork_id, hash, mass, payload, block_time, inputs, outputs)
             ON CONFLICT DO NOTHING",
            generate_placeholders(transactions.len(), COLS)
        )
    } else {
        format!(
            "INSERT INTO transactions (transaction_id, subnetwork_id, hash, mass, payload, block_time, inputs, outputs)
             VALUES {}
             ON CONFLICT DO NOTHING",
            generate_placeholders(transactions.len(), COLS)
        )
    };

    let mut query = sqlx::query(&sql);
    for tx in transactions {
        query = query.bind(&tx.transaction_id);
        query = query.bind(tx.subnetwork_id);
        query = query.bind(&tx.hash);
        query = query.bind(tx.mass);
        query = query.bind(&tx.payload);
        query = query.bind(tx.block_time);
        query = query.bind(&tx.inputs);
        query = query.bind(&tx.outputs);
    }
    Ok(query.execute(pool).await?.rows_affected())
}

pub async fn insert_address_transactions(address_transactions: &[AddressTransaction], pool: &Pool<Postgres>) -> Result<u64, Error> {
    const COLS: usize = 3;
    let sql = format!(
        "INSERT INTO addresses_transactions (address, transaction_id, block_time)
        VALUES {} ON CONFLICT DO NOTHING",
        generate_placeholders(address_transactions.len(), COLS)
    );
    let mut query = sqlx::query(&sql);
    for address_transaction in address_transactions {
        query = query.bind(&address_transaction.address);
        query = query.bind(&address_transaction.transaction_id);
        query = query.bind(address_transaction.block_time);
    }
    Ok(query.execute(pool).await?.rows_affected())
}

pub async fn insert_script_transactions(script_transactions: &[ScriptTransaction], pool: &Pool<Postgres>) -> Result<u64, Error> {
    const COLS: usize = 3;
    let sql = format!(
        "INSERT INTO scripts_transactions (script_public_key, transaction_id, block_time)
        VALUES {} ON CONFLICT DO NOTHING",
        generate_placeholders(script_transactions.len(), COLS)
    );
    let mut query = sqlx::query(&sql);
    for script_transaction in script_transactions {
        query = query.bind(&script_transaction.script_public_key);
        query = query.bind(&script_transaction.transaction_id);
        query = query.bind(script_transaction.block_time);
    }
    Ok(query.execute(pool).await?.rows_affected())
}

pub async fn insert_address_transactions_from_inputs(transaction_ids: &[Hash], pool: &Pool<Postgres>) -> Result<u64, Error> {
    let sql = "
        INSERT INTO addresses_transactions (address, transaction_id, block_time)
        SELECT (o.outputs[i.previous_outpoint_index+1]).script_public_key_address,
               t.transaction_id,
               t.block_time
        FROM transactions t, LATERAL UNNEST(t.inputs) AS i
        JOIN transactions o ON o.transaction_id = i.previous_outpoint_hash
        WHERE t.transaction_id = ANY($1)
        ON CONFLICT DO NOTHING";
    Ok(sqlx::query(sql).bind(transaction_ids).execute(pool).await?.rows_affected())
}

pub async fn insert_script_transactions_from_inputs(transaction_ids: &[Hash], pool: &Pool<Postgres>) -> Result<u64, Error> {
    let sql = "
        INSERT INTO scripts_transactions (script_public_key, transaction_id, block_time)
        SELECT (o.outputs[i.previous_outpoint_index+1]).script_public_key,
               t.transaction_id,
               t.block_time
        FROM transactions t, LATERAL UNNEST(t.inputs) AS i
        JOIN transactions o ON o.transaction_id = i.previous_outpoint_hash
        WHERE t.transaction_id = ANY($1)
        ON CONFLICT DO NOTHING";
    Ok(sqlx::query(sql).bind(transaction_ids).execute(pool).await?.rows_affected())
}

pub async fn insert_block_transactions(block_transactions: &[BlockTransaction], pool: &Pool<Postgres>) -> Result<u64, Error> {
    const COLS: usize = 2;
    let sql = format!(
        "INSERT INTO blocks_transactions (block_hash, transaction_id)
        VALUES {} ON CONFLICT DO NOTHING",
        generate_placeholders(block_transactions.len(), COLS)
    );
    let mut query = sqlx::query(&sql);
    for block_transaction in block_transactions {
        query = query.bind(&block_transaction.block_hash);
        query = query.bind(&block_transaction.transaction_id);
    }
    Ok(query.execute(pool).await?.rows_affected())
}

pub async fn insert_transaction_acceptances(tx_acceptances: &[TransactionAcceptance], pool: &Pool<Postgres>) -> Result<u64, Error> {
    const COLS: usize = 2;
    let sql = format!(
        "INSERT INTO transactions_acceptances (transaction_id, block_hash) VALUES {} ON CONFLICT DO NOTHING",
        generate_placeholders(tx_acceptances.len(), COLS)
    );
    let mut query = sqlx::query(&sql);
    for ta in tx_acceptances {
        query = query.bind(&ta.transaction_id);
        query = query.bind(&ta.block_hash);
    }
    Ok(query.execute(pool).await?.rows_affected())
}
