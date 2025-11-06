-- Enhanced Indexer Migration v11
-- Adds KIP-15 Sequencing Commitment support and transaction filtering/tagging

-- Add sequencing_commitments table for KIP-15 compliance
CREATE TABLE IF NOT EXISTS sequencing_commitments (
    block_hash BYTEA PRIMARY KEY,
    seqcom_hash BYTEA NOT NULL,
    parent_seqcom_hash BYTEA
);

-- Add tag column for transaction filtering/categorization
ALTER TABLE transactions ADD COLUMN IF NOT EXISTS tag VARCHAR(50);
