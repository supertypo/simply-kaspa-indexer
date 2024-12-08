use crate::models::types::blue_work::BlueWork;
use crate::models::types::hash::Hash;
use crate::models::types::nonce::Nonce;

pub struct Block {
    pub hash: Hash,
    pub accepted_id_merkle_root: Option<Hash>,
    pub merge_set_blues_hashes: Option<Vec<Hash>>,
    pub merge_set_reds_hashes: Option<Vec<Hash>>,
    pub selected_parent_hash: Option<Hash>,
    pub bits: Option<i64>,
    pub blue_score: i64,
    pub blue_work: Option<BlueWork>,
    pub daa_score: Option<i64>,
    pub hash_merkle_root: Option<Hash>,
    pub nonce: Option<Nonce>,
    pub pruning_point: Option<Hash>,
    pub timestamp: Option<i64>,
    pub utxo_commitment: Option<Hash>,
    pub version: Option<i16>,
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
