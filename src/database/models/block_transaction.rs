use std::hash::Hash;

#[derive(Eq, PartialEq, Hash)]
pub struct BlockTransaction {
    pub block_hash: [u8; 32],
    pub transaction_id: [u8; 32],
}
