use crate::models::types::hash::Hash;

pub struct Utxo {
    pub transaction_id: Hash,
    pub index: i16,
    pub amount: Option<i64>,
    pub script_public_key: Option<Vec<u8>>,
    pub script_public_key_address: Option<String>,
}
