use std::hash::Hash;

#[derive(Eq, PartialEq, Hash)]
pub struct TransactionAcceptance {
    pub transaction_id: [u8; 32],
    pub block_hash: [u8; 32],
}
