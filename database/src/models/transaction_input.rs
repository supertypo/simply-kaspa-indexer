use sqlx::Type;
use crate::models::types::hash::Hash;

#[derive(Type, Clone)]
#[sqlx(type_name = "transactions_inputs")]
pub struct TransactionInput {
    pub previous_outpoint_hash: Option<Hash>,
    pub previous_outpoint_index: Option<i16>,
    pub signature_script: Option<Vec<u8>>,
    pub sig_op_count: Option<i16>,
    pub previous_outpoint_script: Option<Vec<u8>>,
    pub previous_outpoint_amount: Option<i64>,
}
