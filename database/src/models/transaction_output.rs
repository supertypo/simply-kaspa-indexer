use crate::models::types::hash::Hash;

#[derive(Clone)]
pub struct TransactionOutput {
    pub transaction_id: Hash,
    pub index: i16,
    pub amount: Option<i64>,
    pub script_public_key: Option<Vec<u8>>,
    pub script_public_key_address: Option<String>,
    pub block_time: Option<i64>,
}

impl Eq for TransactionOutput {}

impl PartialEq for TransactionOutput {
    fn eq(&self, other: &Self) -> bool {
        self.transaction_id == other.transaction_id && self.index == other.index
    }
}

impl std::hash::Hash for TransactionOutput {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.transaction_id.hash(state);
        self.index.hash(state);
    }
}
