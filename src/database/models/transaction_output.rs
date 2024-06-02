use std::hash::{Hash, Hasher};

pub struct TransactionOutput {
    pub transaction_id: [u8; 32],
    pub index: i16,
    pub amount: i64,
    pub script_public_key: Vec<u8>,
    pub script_public_key_address: String,
}

impl Eq for TransactionOutput {}

impl PartialEq for TransactionOutput {
    fn eq(&self, other: &Self) -> bool {
        self.transaction_id == other.transaction_id && self.index == other.index
    }
}

impl Hash for TransactionOutput {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.transaction_id.hash(state);
        self.index.hash(state);
    }
}
