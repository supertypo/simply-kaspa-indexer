use simply_kaspa_database::client::KaspaDbClient;

pub const VAR_KEY_BLOCK_CHECKPOINT: &str = "block_checkpoint";
pub const VAR_KEY_VCP_CHECKPOINT: &str = "vcp_checkpoint";

pub async fn load_block_checkpoint(database: &KaspaDbClient) -> Result<String, ()> {
    database.select_var(VAR_KEY_BLOCK_CHECKPOINT).await.map_err(|_| ())
}

pub async fn save_block_checkpoint(block_hash: &String, database: &KaspaDbClient) -> Result<u64, ()> {
    database.upsert_var(VAR_KEY_BLOCK_CHECKPOINT, block_hash).await.map_err(|_| ())
}

pub async fn load_vcp_checkpoint(database: &KaspaDbClient) -> Result<String, ()> {
    database.select_var(VAR_KEY_VCP_CHECKPOINT).await.map_err(|_| ())
}

pub async fn save_vcp_checkpoint(block_hash: &String, database: &KaspaDbClient) -> Result<u64, ()> {
    database.upsert_var(VAR_KEY_VCP_CHECKPOINT, block_hash).await.map_err(|_| ())
}
