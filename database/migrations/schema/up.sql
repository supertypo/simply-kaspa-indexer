CREATE TABLE vars
(
    key   VARCHAR(255) PRIMARY KEY,
    value TEXT NOT NULL
);


CREATE TABLE blocks
(
    hash                    BYTEA PRIMARY KEY,
    accepted_id_merkle_root BYTEA,
    difficulty              DOUBLE PRECISION,
    merge_set_blues_hashes  BYTEA[],
    merge_set_reds_hashes   BYTEA[],
    selected_parent_hash    BYTEA,
    bits                    BIGINT,
    blue_score              BIGINT,
    blue_work               BYTEA,
    daa_score               BIGINT,
    hash_merkle_root        BYTEA,
    nonce                   BYTEA,
    parents                 BYTEA[],
    pruning_point           BYTEA,
    "timestamp"             BIGINT,
    utxo_commitment         BYTEA,
    version                 SMALLINT
);
CREATE INDEX ON blocks (blue_score);


CREATE TABLE chain_blocks
(
    block_hash BYTEA PRIMARY KEY
);


CREATE TABLE subnetworks
(
    id            SMALLSERIAL PRIMARY KEY,
    subnetwork_id VARCHAR(40) NOT NULL
);


CREATE TABLE transactions
(
    transaction_id BYTEA PRIMARY KEY,
    subnetwork_id  SMALLINT,
    hash           BYTEA,
    mass           INTEGER,
    block_time     BIGINT
);
CREATE INDEX ON transactions (block_time DESC);


CREATE TABLE transactions_acceptances
(
    transaction_id BYTEA PRIMARY KEY,
    block_hash     BYTEA
);
CREATE INDEX ON transactions_acceptances (block_hash);


CREATE TABLE blocks_transactions
(
    block_hash     BYTEA,
    transaction_id BYTEA,
    PRIMARY KEY (block_hash, transaction_id)
);
CREATE INDEX ON blocks_transactions (block_hash);
CREATE INDEX ON blocks_transactions (transaction_id);


CREATE TABLE transactions_inputs
(
    transaction_id          BYTEA,
    index                   SMALLINT,
    previous_outpoint_hash  BYTEA,
    previous_outpoint_index SMALLINT,
    signature_script        BYTEA,
    sig_op_count            SMALLINT,
    PRIMARY KEY (transaction_id, index)
);
CREATE INDEX ON transactions_inputs (transaction_id);
CREATE INDEX ON transactions_inputs (previous_outpoint_hash, previous_outpoint_index);


CREATE TABLE transactions_outputs
(
    transaction_id            BYTEA,
    index                     SMALLINT,
    amount                    BIGINT,
    script_public_key         BYTEA,
    script_public_key_address VARCHAR,
    PRIMARY KEY (transaction_id, index)
);
CREATE INDEX ON transactions_outputs (transaction_id);
CREATE INDEX ON transactions_outputs (script_public_key_address);


CREATE TABLE addresses_transactions
(
    address        VARCHAR,
    transaction_id BYTEA,
    block_time     BIGINT,
    PRIMARY KEY (address, transaction_id)
);
CREATE INDEX ON addresses_transactions (address);
CREATE INDEX ON addresses_transactions (transaction_id);
CREATE INDEX ON addresses_transactions (block_time DESC);