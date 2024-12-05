-------------------------------------------
-- v5: Remove redundant data
-------------------------------------------

-- Remove chain_blocks as transactions_acceptances can fullfill the same purpose
DROP TABLE chain_blocks;

-- Update schema_version
UPDATE vars SET value = '5' WHERE key = 'schema_version';
