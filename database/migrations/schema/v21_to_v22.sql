--------------------------------------------------------------
-- v22: Drop blocks_transactions junction table
--      Inline transaction_ids as BYTEA[] on blocks
--      Add block_hash BYTEA to transactions
--------------------------------------------------------------

CREATE TEMP TABLE bt_by_block AS
SELECT block_hash, ARRAY_AGG(transaction_id) AS transaction_ids
FROM blocks_transactions
GROUP BY block_hash;

CREATE TEMP TABLE bt_by_tx AS
SELECT DISTINCT ON (transaction_id) transaction_id, block_hash
FROM blocks_transactions;

CREATE TABLE blocks_new AS
SELECT b.hash,
       b.accepted_id_merkle_root,
       b.merge_set_blues_hashes,
       b.merge_set_reds_hashes,
       b.selected_parent_hash,
       agg.transaction_ids,
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
LEFT JOIN bt_by_block agg ON b.hash = agg.block_hash;

ALTER TABLE blocks_new ADD PRIMARY KEY (hash);
CREATE INDEX ON blocks_new (blue_score);

CREATE TABLE transactions_new AS
SELECT t.transaction_id,
       t.subnetwork_id,
       t.hash,
       t.mass,
       t.payload,
       bt.block_hash,
       t.block_time,
       t.version,
       t.inputs,
       t.outputs
FROM transactions t
LEFT JOIN bt_by_tx bt ON t.transaction_id = bt.transaction_id;

ALTER TABLE transactions_new ADD PRIMARY KEY (transaction_id);
CREATE INDEX ON transactions_new (block_time DESC);

ALTER TABLE blocks RENAME TO blocks_old;
ALTER TABLE blocks_new RENAME TO blocks;
ALTER TABLE transactions RENAME TO transactions_old;
ALTER TABLE transactions_new RENAME TO transactions;

DROP TABLE blocks_old;
DROP TABLE transactions_old;
DROP TABLE bt_by_block;
DROP TABLE bt_by_tx;
DROP TABLE blocks_transactions;

UPDATE vars SET value = '22' WHERE key = 'schema_version';
