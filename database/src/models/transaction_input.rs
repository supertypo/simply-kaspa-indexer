use crate::models::types::hash::Hash;

pub struct TransactionInput {
    pub transaction_id: Hash,
    pub index: i16,
    pub previous_outpoint_hash: Hash,
    pub previous_outpoint_index: i16,
    pub signature_script: Option<Vec<u8>>,
    pub sig_op_count: Option<i16>,
    pub block_time: Option<i64>,
}

impl Eq for TransactionInput {}

impl PartialEq for TransactionInput {
    fn eq(&self, other: &Self) -> bool {
        self.transaction_id == other.transaction_id && self.index == other.index
    }
}

impl std::hash::Hash for TransactionInput {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.transaction_id.hash(state);
        self.index.hash(state);
    }
}
