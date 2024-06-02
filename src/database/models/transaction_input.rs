use std::hash::{Hash, Hasher};

pub struct TransactionInput {
    pub transaction_id: [u8; 32],
    pub index: i16,
    pub previous_outpoint_hash: [u8; 32],
    pub previous_outpoint_index: i16,
    pub signature_script: Vec<u8>,
    pub sig_op_count: i16,
}

impl Eq for TransactionInput {}

impl PartialEq for TransactionInput {
    fn eq(&self, other: &Self) -> bool {
        self.transaction_id == other.transaction_id && self.index == other.index
    }
}

impl Hash for TransactionInput {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.transaction_id.hash(state);
        self.index.hash(state);
    }
}
