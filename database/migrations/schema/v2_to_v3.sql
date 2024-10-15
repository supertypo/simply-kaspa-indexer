----------------------------------------
-- v3: Drop unnecessary indexes
----------------------------------------

-- These indexes are already covered by being first in a primary key composite index
DROP INDEX IF EXISTS block_parent_block_hash_idx;
DROP INDEX IF EXISTS blocks_transactions_block_hash_idx;
DROP INDEX IF EXISTS transactions_inputs_transaction_id_idx;
DROP INDEX IF EXISTS transactions_outputs_transaction_id_idx;
DROP INDEX IF EXISTS addresses_transactions_address_idx;

-- Update schema_version
UPDATE vars SET value = '3' WHERE key = 'schema_version';
