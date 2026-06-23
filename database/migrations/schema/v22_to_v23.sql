--------------------------------------------------------------
-- v22: Denormalize blocks_transactions
--------------------------------------------------------------

-- Consider adjusting work_mem and max_parallel_workers_per_gather first
SET synchronous_commit = off;

--
-- Add the first seen block (by blue_score) to each transaction
--
-- Create deduplication helper table
CREATE TABLE blocks_transactions_dedup AS
SELECT DISTINCT ON (bt.transaction_id)
    bt.transaction_id,
    bt.block_hash
FROM blocks_transactions bt
         JOIN blocks b ON b.hash = bt.block_hash
ORDER BY bt.transaction_id, b.blue_score ASC NULLS LAST;

-- Update planner stats
ANALYZE blocks_transactions_dedup;

-- Build combined table
CREATE TABLE transactions_new AS
SELECT
    t.transaction_id,
    t.subnetwork_id,
    t.hash,
    t.mass,
    t.payload,
    t.block_time,
    t.version,
    t.inputs,
    t.outputs,
    d.block_hash
FROM transactions t
         LEFT JOIN blocks_transactions_dedup d USING (transaction_id);

-- Replace old transactions table
DROP TABLE transactions;
ALTER TABLE transactions_new RENAME TO transactions;

-- Readd constraints and indexes
ALTER TABLE transactions ADD PRIMARY KEY (transaction_id);
CREATE INDEX ON transactions (block_time DESC);

-- Update planner stats
ANALYZE transactions;

-- Drop dedup table
DROP TABLE blocks_transactions_dedup;


--
-- Add all transaction_ids to each block
--
-- Build combined table (column order matches up.sql)
CREATE TABLE blocks_new AS
SELECT
    b.hash,
    b.accepted_id_merkle_root,
    bt.transaction_ids,
    b.merge_set_blues_hashes,
    b.merge_set_reds_hashes,
    b.selected_parent_hash,
    b.bits,
    b.blue_score,
    b.blue_work,
    b.daa_score,
    b.hash_merkle_root,
    b.nonce,
    b.pruning_point,
    b."timestamp",
    b.utxo_commitment,
    b.version
FROM blocks b
         LEFT JOIN (
    SELECT block_hash, array_agg(transaction_id) AS transaction_ids
    FROM blocks_transactions
    GROUP BY block_hash
) bt ON b.hash = bt.block_hash;

-- Replace old blocks table
DROP TABLE blocks;
ALTER TABLE blocks_new RENAME TO blocks;

-- Readd constraints and indexes
ALTER TABLE blocks ADD PRIMARY KEY (hash);
CREATE INDEX ON blocks (blue_score);

-- Update planner stats
ANALYZE blocks;

-- Drop the now denormalized juntion table
DROP TABLE blocks_transactions;


-- Update schema_version
UPDATE vars SET value = '23' WHERE key = 'schema_version';
