use crate::models::types::blue_work::BlueWork;
use crate::models::types::hash::Hash;
use crate::models::types::nonce::Nonce;

pub struct Block {
    pub hash: Hash,
    pub accepted_id_merkle_root: Hash,
    pub difficulty: f64,
    pub merge_set_blues_hashes: Vec<Hash>,
    pub merge_set_reds_hashes: Vec<Hash>,
    pub selected_parent_hash: Hash,
    pub bits: i64,
    pub blue_score: i64,
    pub blue_work: BlueWork,
    pub daa_score: i64,
    pub hash_merkle_root: Hash,
    pub nonce: Nonce,
    pub parents: Vec<Hash>,
    pub pruning_point: Hash,
    pub timestamp: i64,
    pub utxo_commitment: Hash,
    pub version: i16,
}

impl Eq for Block {}

impl PartialEq for Block {
    fn eq(&self, other: &Self) -> bool {
        self.hash == other.hash
    }
}

impl std::hash::Hash for Block {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.hash.hash(state);
    }
}
