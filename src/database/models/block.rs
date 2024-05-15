use std::hash::{Hash, Hasher};

use diesel::prelude::*;

#[derive(Queryable, Selectable, Insertable)]
#[diesel(table_name = crate::database::schema::blocks)]
#[diesel(primary_key(hash))]
pub struct Block {
    pub hash: Vec<u8>,
    pub accepted_id_merkle_root: Vec<u8>,
    pub difficulty: f64,
    pub merge_set_blues_hashes: Vec<Vec<u8>>,
    pub merge_set_reds_hashes: Vec<Vec<u8>>,
    pub selected_parent_hash: Vec<u8>,
    pub bits: i64,
    pub blue_score: i64,
    pub blue_work: Vec<u8>,
    pub daa_score: i64,
    pub hash_merkle_root: Vec<u8>,
    pub nonce: Vec<u8>,
    pub parents: Vec<Vec<u8>>,
    pub pruning_point: Vec<u8>,
    pub timestamp: i64,
    pub utxo_commitment: Vec<u8>,
    pub version: i16,
}

impl Eq for Block {}

impl PartialEq for Block {
    fn eq(&self, other: &Self) -> bool {
        self.hash == other.hash
    }
}

impl Hash for Block {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.hash.hash(state);
    }
}
