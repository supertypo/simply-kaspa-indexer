CREATE TABLE IF NOT EXISTS "vars"
(
    key   VARCHAR(255) PRIMARY KEY,
    value TEXT NOT NULL
);


CREATE TABLE IF NOT EXISTS "blocks"
(
    hash                    BYTEA PRIMARY KEY,
    accepted_id_merkle_root BYTEA,
    difficulty              DOUBLE PRECISION,
    is_chain_block          BOOLEAN,
    merge_set_blues_hashes  BYTEA[],
    merge_set_reds_hashes   BYTEA[],
    selected_parent_hash    BYTEA,
    bits                    BIGINT         NOT NULL,
    blue_score              BIGINT         NOT NULL,
    blue_work               BYTEA          NOT NULL,
    daa_score               BIGINT         NOT NULL,
    hash_merkle_root        BYTEA          NOT NULL,
    nonce                   NUMERIC(32, 0) NOT NULL,
    parents                 BYTEA[]        NOT NULL,
    pruning_point           BYTEA          NOT NULL,
    "timestamp"             INTEGER        NOT NULL,
    utxo_commitment         BYTEA          NOT NULL,
    version                 SMALLINT       NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_block_is_chain_block ON blocks (is_chain_block);
CREATE INDEX IF NOT EXISTS idx_blue_score ON blocks (blue_score);
CREATE INDEX IF NOT EXISTS idx_daa_score ON blocks (daa_score);


CREATE TABLE IF NOT EXISTS "subnetworks"
(
    id            SERIAL PRIMARY KEY,
    subnetwork_id VARCHAR(40) NOT NULL
);


CREATE TABLE IF NOT EXISTS "transactions"
(
    transaction_id       BYTEA PRIMARY KEY,
    subnetwork           INT,
    hash                 BYTEA,
    mass                 INTEGER,
    block_hash           BYTEA[] NOT NULL,
    block_time           INTEGER,
    is_accepted          BOOLEAN NOT NULL,
    accepting_block_hash BYTEA,
    CONSTRAINT fk_subnetwork FOREIGN KEY (subnetwork) REFERENCES subnetworks (id)
);

CREATE INDEX IF NOT EXISTS block_time_idx ON transactions (block_time);
CREATE INDEX IF NOT EXISTS idx_accepting_block ON transactions (accepting_block_hash);
CREATE INDEX IF NOT EXISTS idx_block_hash ON transactions (block_hash);


CREATE TABLE IF NOT EXISTS "transactions_outputs"
(
    id                        BIGSERIAL PRIMARY KEY,
    transaction_id            BYTEA,
    index                     SMALLINT,
    amount                    BIGINT,
    script_public_key         BYTEA[],
    script_public_key_address VARCHAR(128),
    script_public_key_type    VARCHAR(32),
    accepting_block_hash      BYTEA
);

CREATE INDEX IF NOT EXISTS idx_txouts ON transactions_outputs (transaction_id);
CREATE INDEX IF NOT EXISTS idx_txouts_addr ON transactions_outputs (script_public_key_address);
CREATE INDEX IF NOT EXISTS tx_id_and_index ON transactions_outputs (transaction_id, index);


CREATE TABLE IF NOT EXISTS "transactions_inputs"
(
    id                      BIGSERIAL PRIMARY KEY,
    transaction_id          BYTEA,
    index                   SMALLINT,
    previous_outpoint_hash  BYTEA[],
    previous_outpoint_index SMALLINT,
    signature_script        BYTEA,
    sig_op_count            SMALLINT
);

CREATE INDEX IF NOT EXISTS idx_txin_prev ON transactions_inputs (previous_outpoint_hash);
CREATE INDEX IF NOT EXISTS idx_txin ON transactions_inputs (transaction_id);


CREATE TABLE IF NOT EXISTS "tx_id_address_mapping"
(
    id             BIGSERIAL PRIMARY KEY,
    transaction_id BYTEA        NOT NULL,
    address        VARCHAR(128) NOT NULL,
    block_time     INTEGER      NOT NULL,
    is_accepted    BOOLEAN      NOT NULL,
    CONSTRAINT tx_id_address_mapping_transaction_id_address_key UNIQUE (transaction_id, address)
);

CREATE INDEX IF NOT EXISTS idx_address_block_time ON tx_id_address_mapping (address, block_time);
CREATE INDEX IF NOT EXISTS idx_block_time ON tx_id_address_mapping (block_time);
CREATE INDEX IF NOT EXISTS idx_tx_id ON tx_id_address_mapping (transaction_id);
