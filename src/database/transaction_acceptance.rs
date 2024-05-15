use std::hash::Hash;

use diesel::prelude::*;

#[derive(Queryable, Selectable, Insertable, Clone, Eq, PartialEq, Hash)]
#[diesel(table_name = crate::database::schema::transactions_acceptances)]
#[diesel(primary_key(transaction_id))]
pub struct TransactionAcceptance {
    pub transaction_id: Vec<u8>,
    pub block_hash: Vec<u8>,
}
