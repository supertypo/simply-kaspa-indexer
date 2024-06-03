use itertools::Itertools;
use sqlx::{Error, Executor, Pool, Postgres, Row};

use crate::database::models::address_transaction::AddressTransaction;
use crate::database::models::block::Block;
use crate::database::models::block_transaction::BlockTransaction;
use crate::database::models::chain_block::ChainBlock;
use crate::database::models::sql_hash::SqlHash;
use crate::database::models::transaction::Transaction;
use crate::database::models::transaction_acceptance::TransactionAcceptance;
use crate::database::models::transaction_input::TransactionInput;
use crate::database::models::transaction_output::TransactionOutput;

pub async fn insert_subnetwork(subnetwork_id: &String, pool: &Pool<Postgres>) -> Result<i16, Error> {
    sqlx::query("INSERT INTO subnetworks (subnetwork_id) VALUES ($1) ON CONFLICT DO NOTHING RETURNING id")
        .bind(&subnetwork_id)
        .fetch_one(pool)
        .await?
        .try_get(0)
}

pub async fn insert_blocks(blocks: &[Block], pool: &Pool<Postgres>) -> Result<u64, Error> {
    const COLS: usize = 17;
    let mut tx = pool.begin().await?;

    let sql = format!(
        "INSERT INTO blocks (hash, accepted_id_merkle_root, difficulty, merge_set_blues_hashes, merge_set_reds_hashes,
            selected_parent_hash, bits, blue_score, blue_work, daa_score, hash_merkle_root, nonce, parents, pruning_point,
            timestamp, utxo_commitment, version
        ) VALUES {} ON CONFLICT DO NOTHING",
        generate_placeholders(blocks.len(), COLS)
    );

    let mut query = sqlx::query(&sql);
    for block in blocks {
        query = query.bind(&block.hash);
        query = query.bind(&block.accepted_id_merkle_root);
        query = query.bind(&block.difficulty);
        query = query.bind((!&block.merge_set_blues_hashes.is_empty()).then_some(&block.merge_set_blues_hashes));
        query = query.bind((!&block.merge_set_reds_hashes.is_empty()).then_some(&block.merge_set_reds_hashes));
        query = query.bind(&block.selected_parent_hash);
        query = query.bind(&block.bits);
        query = query.bind(&block.blue_score);
        query = query.bind(&block.blue_work);
        query = query.bind(&block.daa_score);
        query = query.bind(&block.hash_merkle_root);
        query = query.bind(&block.nonce);
        query = query.bind(&block.parents);
        query = query.bind(&block.pruning_point);
        query = query.bind(&block.timestamp);
        query = query.bind(&block.utxo_commitment);
        query = query.bind(&block.version);
    }
    let rows_affected = tx.execute(query).await?.rows_affected();
    tx.commit().await?;
    Ok(rows_affected)
}

pub async fn insert_transactions(transactions: &[Transaction], pool: &Pool<Postgres>) -> Result<u64, Error> {
    const COLS: usize = 5;
    let sql = format!(
        "INSERT INTO transactions (transaction_id, subnetwork_id, hash, mass, block_time)
        VALUES {} ON CONFLICT DO NOTHING",
        generate_placeholders(transactions.len(), COLS)
    );
    let mut query = sqlx::query(&sql);
    for tx in transactions {
        query = query.bind(&tx.transaction_id);
        query = query.bind(&tx.subnetwork_id);
        query = query.bind(&tx.hash);
        query = query.bind((tx.mass != 0).then_some(tx.mass));
        query = query.bind(&tx.block_time);
    }
    Ok(query.execute(pool).await?.rows_affected())
}

pub async fn insert_transaction_inputs(transaction_inputs: &[TransactionInput], pool: &Pool<Postgres>) -> Result<u64, Error> {
    const COLS: usize = 6;
    let sql = format!(
        "INSERT INTO transactions_inputs (transaction_id, index, previous_outpoint_hash, previous_outpoint_index,
            signature_script, sig_op_count)
        VALUES {} ON CONFLICT DO NOTHING",
        generate_placeholders(transaction_inputs.len(), COLS)
    );
    let mut query = sqlx::query(&sql);
    for tin in transaction_inputs {
        query = query.bind(&tin.transaction_id);
        query = query.bind(&tin.index);
        query = query.bind(&tin.previous_outpoint_hash);
        query = query.bind(&tin.previous_outpoint_index);
        query = query.bind(&tin.signature_script);
        query = query.bind(&tin.sig_op_count);
    }
    Ok(query.execute(pool).await?.rows_affected())
}

pub async fn insert_transaction_outputs(transaction_outputs: &[TransactionOutput], pool: &Pool<Postgres>) -> Result<u64, Error> {
    const COLS: usize = 5;
    let sql = format!(
        "INSERT INTO transactions_outputs (transaction_id, index, amount, script_public_key, script_public_key_address)
        VALUES {} ON CONFLICT DO NOTHING",
        generate_placeholders(transaction_outputs.len(), COLS)
    );
    let mut query = sqlx::query(&sql);
    for tout in transaction_outputs {
        query = query.bind(&tout.transaction_id);
        query = query.bind(&tout.index);
        query = query.bind(&tout.amount);
        query = query.bind(&tout.script_public_key);
        query = query.bind(&tout.script_public_key_address);
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
        query = query.bind(&address_transaction.block_time);
    }
    Ok(query.execute(pool).await?.rows_affected())
}

pub async fn insert_address_transactions_from_inputs(transaction_ids: &[SqlHash], pool: &Pool<Postgres>) -> Result<u64, Error> {
    let sql = "
    INSERT INTO addresses_transactions (address, transaction_id, block_time)
        SELECT o.script_public_key_address, i.transaction_id, t.block_time
            FROM transactions_inputs i
            JOIN transactions t ON t.transaction_id = i.transaction_id
            JOIN transactions_outputs o ON o.transaction_id = i.previous_outpoint_hash AND o.index = i.previous_outpoint_index
        WHERE i.transaction_id = ANY($1) AND t.transaction_id = ANY($1)
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

pub async fn insert_chain_blocks(chain_blocks: &[ChainBlock], pool: &Pool<Postgres>) -> Result<u64, Error> {
    const COLS: usize = 1;
    let sql = format!(
        "INSERT INTO chain_blocks (block_hash) VALUES {} ON CONFLICT DO NOTHING",
        generate_placeholders(chain_blocks.len(), COLS)
    );
    let mut query = sqlx::query(&sql);
    for cb in chain_blocks {
        query = query.bind(&cb.block_hash);
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

fn generate_placeholders(rows: usize, columns: usize) -> String {
    (0..rows).map(|i| format!("({})", (1..=columns).map(|c| format!("${}", c + i * columns)).join(", "))).join(", ")
}
