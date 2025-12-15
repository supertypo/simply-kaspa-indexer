-- Enhanced Indexer Migration v11
-- Adds KIP-15 Sequencing Commitment support and transaction filtering/tagging with normalized tag providers

-- Add sequencing_commitments table for KIP-15 compliance
CREATE TABLE IF NOT EXISTS sequencing_commitments (
    block_hash BYTEA PRIMARY KEY,
    seqcom_hash BYTEA NOT NULL,
    parent_seqcom_hash BYTEA
);

-- Add tag_providers table for normalized protocol metadata
CREATE TABLE IF NOT EXISTS tag_providers (
    id SERIAL PRIMARY KEY,
    tag VARCHAR(50) NOT NULL,
    module VARCHAR(50) NOT NULL,
    prefix VARCHAR(100) NOT NULL,
    repository_url TEXT,
    description TEXT,
    created_at TIMESTAMP DEFAULT NOW(),
    UNIQUE (tag, module)
);
CREATE INDEX IF NOT EXISTS idx_tag_providers_tag_module ON tag_providers(tag, module);

-- Add tag_id column for transaction filtering/categorization (FK to tag_providers)
ALTER TABLE transactions ADD COLUMN IF NOT EXISTS tag_id INTEGER REFERENCES tag_providers(id) ON DELETE SET NULL;
CREATE INDEX IF NOT EXISTS idx_transactions_tag_id ON transactions(tag_id);
