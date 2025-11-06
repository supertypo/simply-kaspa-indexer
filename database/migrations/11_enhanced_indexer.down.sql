-- Rollback Enhanced Indexer Migration v11

-- Remove tag_id column (FK to tag_providers)
ALTER TABLE transactions DROP COLUMN IF EXISTS tag_id;

-- Drop tag_providers table
DROP TABLE IF EXISTS tag_providers CASCADE;

-- Drop sequencing_commitments table
DROP TABLE IF EXISTS sequencing_commitments;
