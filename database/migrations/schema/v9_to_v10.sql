--------------------------------------------------------------
-- v10: Support data retention period
--------------------------------------------------------------

-- Need an index on inputs to quickly look for unspent outputs
CREATE INDEX ON transactions_inputs (previous_outpoint_hash, previous_outpoint_index);

-- Update schema_version
UPDATE vars SET value = '10' WHERE key = 'schema_version';
