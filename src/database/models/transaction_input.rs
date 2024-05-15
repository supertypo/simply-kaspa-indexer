use std::hash::{Hash, Hasher};

use diesel::prelude::*;

#[derive(Queryable, Selectable, Insertable, Identifiable, Clone)]
#[diesel(table_name = crate::database::schema::transactions_inputs)]
#[diesel(primary_key(transaction_id, index))]
pub struct TransactionInput {
    pub transaction_id: Vec<u8>,
    pub index: i16,
    pub previous_outpoint_hash: Vec<u8>,
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
