use std::hash::Hash;

#[derive(Clone, Eq, PartialEq, Hash)]
pub struct TransactionAcceptance {
    pub transaction_id: Vec<u8>,
    pub block_hash: Vec<u8>,
}
