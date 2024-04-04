use bigdecimal::BigDecimal;
use diesel::prelude::*;

#[derive(Queryable, Selectable, Insertable)]
#[diesel(table_name = crate::database::schema::blocks)]
#[diesel(primary_key(hash))]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Block {
    pub hash: Vec<u8>,
    pub accepted_id_merkle_root: Option<Vec<u8>>,
    pub difficulty: Option<f64>,
    pub is_chain_block: Option<bool>,
    pub merge_set_blues_hashes: Option<Vec<Option<Vec<u8>>>>,
    pub merge_set_reds_hashes: Option<Vec<Option<Vec<u8>>>>,
    pub selected_parent_hash: Option<Vec<u8>>,
    pub bits: Option<i32>,
    pub blue_score: Option<i64>,
    pub blue_work: Option<Vec<u8>>,
    pub daa_score: Option<i64>,
    pub hash_merkle_root: Option<Vec<u8>>,
    pub nonce: Option<BigDecimal>,
    pub parents: Option<Vec<Option<Vec<u8>>>>,
    pub pruning_point: Option<Vec<u8>>,
    pub timestamp: Option<i32>,
    pub utxo_commitment: Option<Vec<u8>>,
    pub version: Option<i16>,
}
