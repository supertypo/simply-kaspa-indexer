use std::fmt::Debug;
use std::hash::{Hash, Hasher};

#[derive(Clone, Debug)]
pub struct AddressTransaction {
    pub address: String,
    pub transaction_id: Vec<u8>,
    pub block_time: i64,
}

impl Eq for AddressTransaction {}

impl PartialEq for AddressTransaction {
    fn eq(&self, other: &Self) -> bool {
        self.address == other.address && self.transaction_id == other.transaction_id
    }
}

impl Hash for AddressTransaction {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.address.hash(state);
        self.transaction_id.hash(state);
    }
}
