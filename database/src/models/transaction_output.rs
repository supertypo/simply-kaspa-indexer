use sqlx::Type;

#[derive(Type, Clone)]
#[sqlx(type_name = "transactions_outputs")]
pub struct TransactionOutput {
    pub amount: Option<i64>,
    pub script_public_key: Option<Vec<u8>>,
    pub script_public_key_address: Option<String>,
}
