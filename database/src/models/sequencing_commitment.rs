use crate::models::types::hash::Hash;
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow)]
pub struct SequencingCommitment {
    pub block_hash: Hash,
    pub seqcom_hash: Hash,
    pub parent_seqcom_hash: Option<Hash>,
}
