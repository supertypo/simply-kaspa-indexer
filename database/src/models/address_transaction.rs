use crate::models::types::hash::Hash;

#[derive(Clone)]
pub struct AddressTransaction {
    pub address: String,
    pub transaction_id: Hash,
    pub block_time: i64,
}

impl Eq for AddressTransaction {}

impl PartialEq for AddressTransaction {
    fn eq(&self, other: &Self) -> bool {
        self.address == other.address && self.transaction_id == other.transaction_id
    }
}

impl std::hash::Hash for AddressTransaction {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.address.hash(state);
        self.transaction_id.hash(state);
    }
}
