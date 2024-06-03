use crate::database::models::sql_hash::SqlHash;
use std::hash::{Hash, Hasher};

pub struct Block {
    pub hash: SqlHash,
    pub accepted_id_merkle_root: SqlHash,
    pub difficulty: f64,
    pub merge_set_blues_hashes: Vec<SqlHash>,
    pub merge_set_reds_hashes: Vec<SqlHash>,
    pub selected_parent_hash: SqlHash,
    pub bits: i64,
    pub blue_score: i64,
    pub blue_work: [u8; 24],
    pub daa_score: i64,
    pub hash_merkle_root: SqlHash,
    pub nonce: [u8; 8],
    pub parents: Vec<SqlHash>,
    pub pruning_point: SqlHash,
    pub timestamp: i64,
    pub utxo_commitment: SqlHash,
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
