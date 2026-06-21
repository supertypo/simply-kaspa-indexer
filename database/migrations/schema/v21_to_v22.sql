--------------------------------------------------------------
-- v22: Toccata HF - additional fields
--------------------------------------------------------------

ALTER TYPE transactions_inputs ADD ATTRIBUTE compute_budget SMALLINT;
ALTER TYPE transactions_inputs ADD ATTRIBUTE covenant_id BYTEA;

ALTER TYPE transactions_outputs ADD ATTRIBUTE covenant_authorizing_input SMALLINT;
ALTER TYPE transactions_outputs ADD ATTRIBUTE covenant_id BYTEA;

-- Update schema_version
UPDATE vars SET value = '22' WHERE key = 'schema_version';
