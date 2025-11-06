-- Rollback Enhanced Indexer Migration v11

-- Remove tag column
ALTER TABLE transactions DROP COLUMN IF EXISTS tag;

-- Drop sequencing_commitments table
DROP TABLE IF EXISTS sequencing_commitments;
