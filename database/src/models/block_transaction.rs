use crate::models::types::hash::Hash;

#[derive(Eq, PartialEq, Hash)]
pub struct BlockTransaction {
    pub block_hash: Hash,
    pub transaction_id: Hash,
}
