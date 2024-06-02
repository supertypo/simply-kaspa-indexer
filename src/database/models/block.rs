use std::hash::{Hash, Hasher};

pub struct Block {
    pub hash: [u8; 32],
    pub accepted_id_merkle_root: [u8; 32],
    pub difficulty: f64,
    pub merge_set_blues_hashes: Vec<[u8; 32]>,
    pub merge_set_reds_hashes: Vec<[u8; 32]>,
    pub selected_parent_hash: [u8; 32],
    pub bits: i64,
    pub blue_score: i64,
    pub blue_work: [u8; 24],
    pub daa_score: i64,
    pub hash_merkle_root: [u8; 32],
    pub nonce: [u8; 8],
    pub parents: Vec<[u8; 32]>,
    pub pruning_point: [u8; 32],
    pub timestamp: i64,
    pub utxo_commitment: [u8; 32],
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
