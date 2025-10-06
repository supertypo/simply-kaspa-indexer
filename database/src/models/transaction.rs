use crate::models::types::hash::Hash;
use crate::models::types::payload::Payload;

#[derive(Clone)]
pub struct Transaction {
    pub transaction_id: Hash,
    pub subnetwork_id: Option<i32>,
    pub hash: Option<Hash>,
    pub mass: Option<i32>,
    pub payload: Option<Payload>,
    pub block_time: Option<i64>,
}

impl Eq for Transaction {}

impl PartialEq for Transaction {
    fn eq(&self, other: &Self) -> bool {
        self.transaction_id == other.transaction_id
    }
}

impl std::hash::Hash for Transaction {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.transaction_id.hash(state);
    }
}
