use std::hash::Hash;

#[derive(Clone, Eq, PartialEq, Hash)]
pub struct BlockTransaction {
    pub block_hash: Vec<u8>,
    pub transaction_id: Vec<u8>,
}
