use crate::models::types::hash::Hash;

#[derive(Eq, PartialEq, Hash)]
pub struct BlockParent {
    pub block_hash: Hash,
    pub parent_hash: Hash,
}
