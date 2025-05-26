CREATE TABLE vars
(
    key   VARCHAR(255) PRIMARY KEY,
    value TEXT NOT NULL
);
INSERT INTO vars (key, value) VALUES ('schema_version', '10');


CREATE TABLE blocks
(
    hash                    BYTEA PRIMARY KEY,
    accepted_id_merkle_root BYTEA,
    merge_set_blues_hashes  BYTEA[],
    merge_set_reds_hashes   BYTEA[],
    selected_parent_hash    BYTEA,
    bits                    BIGINT,
    blue_score              BIGINT,
    blue_work               BYTEA,
    daa_score               BIGINT,
    hash_merkle_root        BYTEA,
    nonce                   BYTEA,
    pruning_point           BYTEA,
    "timestamp"             BIGINT,
    utxo_commitment         BYTEA,
    version                 SMALLINT
);
CREATE INDEX ON blocks (blue_score);


CREATE TABLE block_parent
(
    block_hash  BYTEA,
    parent_hash BYTEA,
    PRIMARY KEY (block_hash, parent_hash)
);
CREATE INDEX ON block_parent (parent_hash);


CREATE TABLE subnetworks
(
    id            SERIAL PRIMARY KEY,
    subnetwork_id VARCHAR(40) NOT NULL
);


CREATE TABLE transactions
(
    transaction_id BYTEA PRIMARY KEY,
    subnetwork_id  INTEGER,
    hash           BYTEA,
    mass           INTEGER,
    payload        BYTEA,
    block_time     BIGINT
);
CREATE INDEX ON transactions (block_time DESC);


CREATE TABLE transactions_acceptances
(
    transaction_id BYTEA UNIQUE,
    block_hash     BYTEA
);
CREATE INDEX ON transactions_acceptances (block_hash);


CREATE TABLE blocks_transactions
(
    block_hash     BYTEA,
    transaction_id BYTEA,
    PRIMARY KEY (block_hash, transaction_id)
);
CREATE INDEX ON blocks_transactions (transaction_id);


CREATE TABLE transactions_inputs
(
    transaction_id           BYTEA,
    index                    SMALLINT,
    previous_outpoint_hash   BYTEA,
    previous_outpoint_index  SMALLINT,
    signature_script         BYTEA,
    sig_op_count             SMALLINT,
    block_time               BIGINT,
    previous_outpoint_script BYTEA,
    previous_outpoint_amount BIGINT,
    PRIMARY KEY (transaction_id, index)
);
-- Create index (optional, if you need to search txs for address)
--CREATE INDEX ON transactions_inputs (previous_outpoint_script, block_time DESC);


CREATE TABLE transactions_outputs
(
    transaction_id            BYTEA,
    index                     SMALLINT,
    amount                    BIGINT,
    script_public_key         BYTEA,
    script_public_key_address VARCHAR,
    block_time                BIGINT,
    PRIMARY KEY (transaction_id, index)
);
-- Create index (optional, if you need to search txs for address)
--CREATE INDEX ON transactions_outputs (script_public_key, block_time DESC);


CREATE TABLE addresses_transactions
(
    address        VARCHAR,
    transaction_id BYTEA,
    block_time     BIGINT,
    PRIMARY KEY (address, transaction_id)
);
CREATE INDEX ON addresses_transactions (address, block_time DESC);


CREATE TABLE scripts_transactions
(
    script_public_key BYTEA,
    transaction_id    BYTEA,
    block_time        BIGINT,
    PRIMARY KEY (script_public_key, transaction_id)
);
CREATE INDEX ON scripts_transactions (script_public_key, block_time DESC);
