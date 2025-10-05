--------------------------------------------------------------
-- v20: Denormalized transactions
--------------------------------------------------------------
-- Update schema_version
UPDATE vars SET value = '20' WHERE key = 'schema_version';

-- Rename existing tables
ALTER TABLE transactions RENAME TO transactions_old;
ALTER TABLE transactions_inputs RENAME TO transactions_inputs_old;
ALTER TABLE transactions_outputs RENAME TO transactions_outputs_old;

-- Create new types
CREATE TYPE transactions_inputs AS
(
    index                    SMALLINT,
    previous_outpoint_hash   BYTEA,
    previous_outpoint_index  SMALLINT,
    signature_script         BYTEA,
    sig_op_count             SMALLINT,
    previous_outpoint_script BYTEA,
    previous_outpoint_amount BIGINT
);

CREATE TYPE transactions_outputs AS
(
    index                     SMALLINT,
    amount                    BIGINT,
    script_public_key         BYTEA,
    script_public_key_address VARCHAR
);

-- Migrate transactions_inputs and transactions_outputs to transactions
CREATE TABLE transactions
(
  transaction_id BYTEA,
  subnetwork_id  INTEGER,
  hash           BYTEA,
  mass           INTEGER,
  payload        BYTEA,
  block_time     BIGINT,
  inputs         transactions_inputs[],
  outputs        transactions_outputs[],
  outputs_spent  SMALLINT
);

INSERT INTO transactions (transaction_id, subnetwork_id, hash, mass, payload, block_time, inputs, outputs)
SELECT t.transaction_id,
       t.subnetwork_id,
       t.hash,
       t.mass,
       t.payload,
       t.block_time,
       i.inputs,
       o.outputs
FROM transactions_old t
LEFT JOIN LATERAL (
    SELECT ARRAY_AGG(
        ROW (
            i.index,
            i.previous_outpoint_hash,
            i.previous_outpoint_index,
            i.signature_script,
            i.sig_op_count,
            i.previous_outpoint_script,
            i.previous_outpoint_amount
        )::transactions_inputs
        ORDER BY i.index
    ) AS inputs
    FROM transactions_inputs_old i
    WHERE i.transaction_id = t.transaction_id
) i ON TRUE
LEFT JOIN LATERAL (
    SELECT ARRAY_AGG(
        ROW (
            o.index,
            o.amount,
            o.script_public_key,
            o.script_public_key_address
        )::transactions_outputs
        ORDER BY o.index
    ) AS outputs
    FROM transactions_outputs_old o
    WHERE o.transaction_id = t.transaction_id
) o ON TRUE;

DROP TABLE transactions_old;
DROP TABLE transactions_inputs_old;
DROP TABLE transactions_outputs_old;

ALTER TABLE transactions ADD PRIMARY KEY (transaction_id);
CREATE INDEX ON transactions (block_time DESC);

VACUUM ANALYZE transactions;
