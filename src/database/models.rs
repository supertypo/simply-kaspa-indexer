use bigdecimal::BigDecimal;
use diesel::prelude::*;

#[derive(Queryable, Selectable, Insertable)]
#[diesel(table_name = crate::database::schema::blocks)]
#[diesel(primary_key(hash))]
pub struct Block {
    pub hash: Vec<u8>,
    pub accepted_id_merkle_root: Option<Vec<u8>>,
    pub difficulty: Option<f64>,
    pub is_chain_block: Option<bool>,
    pub merge_set_blues_hashes: Option<Vec<Option<Vec<u8>>>>,
    pub merge_set_reds_hashes: Option<Vec<Option<Vec<u8>>>>,
    pub selected_parent_hash: Option<Vec<u8>>,
    pub bits: i64,
    pub blue_score: i64,
    pub blue_work: Vec<u8>,
    pub daa_score: i64,
    pub hash_merkle_root: Vec<u8>,
    pub nonce: BigDecimal,
    pub parents: Vec<Option<Vec<u8>>>,
    pub pruning_point: Vec<u8>,
    pub timestamp: i32,
    pub utxo_commitment: Vec<u8>,
    pub version: i16,
}

pub const VAR_KEY_START_HASH: &str = "vspc_last_start_hash";

#[derive(Queryable, Selectable, Insertable)]
#[diesel(table_name = crate::database::schema::vars)]
#[diesel(primary_key(key))]
pub struct Var {
    pub key: String,
    pub value: String,
}
