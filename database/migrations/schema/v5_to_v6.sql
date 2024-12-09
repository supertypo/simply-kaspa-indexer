--------------------------------------------------------------
-- v6: Support fast on-demand address to transactions resolve
--------------------------------------------------------------

-- Add indexed block_time to transactions_outputs
ALTER TABLE transactions_inputs ADD COLUMN block_time BIGINT;
-- Create indexes
CREATE INDEX ON transactions_inputs (previous_outpoint_hash, previous_outpoint_index);
CREATE INDEX ON transactions_inputs (block_time DESC);

-- Add block_time to transactions_outputs
ALTER TABLE transactions_outputs ADD COLUMN block_time BIGINT;
-- Create indexes
CREATE INDEX ON transactions_outputs (script_public_key_address);
CREATE INDEX ON transactions_outputs (block_time DESC);

-- Update schema_version
UPDATE vars SET value = '6' WHERE key = 'schema_version';
