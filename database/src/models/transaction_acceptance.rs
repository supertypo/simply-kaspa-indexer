use crate::models::types::hash::Hash;

#[derive(Eq, PartialEq, Hash)]
pub struct TransactionAcceptance {
    pub transaction_id: Option<Hash>,
    pub block_hash: Option<Hash>,
}
