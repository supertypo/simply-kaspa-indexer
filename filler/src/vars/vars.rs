use kaspa_database::client::client::KaspaDbClient;

pub const VAR_KEY_BLOCK_CHECKPOINT: &str = "block_checkpoint";
pub const VAR_KEY_LEGACY_CHECKPOINT: &str = "vspc_last_start_hash";

pub async fn load_block_checkpoint(database: &KaspaDbClient) -> Result<String, ()> {
    if let Ok(block_hash) = database.select_var(VAR_KEY_BLOCK_CHECKPOINT).await {
        return Ok(block_hash);
    }
    if let Ok(block_hash) = database.select_var(VAR_KEY_LEGACY_CHECKPOINT).await {
        return Ok(block_hash);
    }
    Err(())
}

pub async fn save_checkpoint(block_hash: &String, database: &KaspaDbClient) -> Result<u64, ()> {
    match database.upsert_var(VAR_KEY_BLOCK_CHECKPOINT, block_hash).await {
        Ok(rows_affected) => { Ok(rows_affected) }
        Err(_) => { Err(()) }
    }
}
