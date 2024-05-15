use std::hash::{Hash, Hasher};

use diesel::prelude::*;

#[derive(Queryable, Selectable, Insertable, Clone)]
#[diesel(table_name = crate::database::schema::transactions)]
#[diesel(primary_key(transaction_id))]
pub struct Transaction {
    pub transaction_id: Vec<u8>,
    pub subnetwork_id: i16,
    pub hash: Vec<u8>,
    pub mass: Option<i32>,
    pub block_time: i64,
}

impl Eq for Transaction {}

impl PartialEq for Transaction {
    fn eq(&self, other: &Self) -> bool {
        self.transaction_id == other.transaction_id
    }
}

impl Hash for Transaction {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.transaction_id.hash(state);
    }
}
