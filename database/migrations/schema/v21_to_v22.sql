--------------------------------------------------------------
-- v22: Denormalize blocks_transactions
--------------------------------------------------------------

ALTER TABLE blocks ADD COLUMN transaction_ids BYTEA[];
ALTER TABLE transactions ADD COLUMN block_hash BYTEA;

-- TODO: DATA MIGRATION

DROP TABLE blocks_transactions;

-- Update schema_version
UPDATE vars SET value = '22' WHERE key = 'schema_version';
