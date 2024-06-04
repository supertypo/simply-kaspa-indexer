use crate::models::types::hash::Hash;

#[derive(Eq, PartialEq, Hash)]
pub struct ChainBlock {
    pub block_hash: Hash,
}
