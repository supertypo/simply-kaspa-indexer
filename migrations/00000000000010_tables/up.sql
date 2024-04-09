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
    subnetwork           INT     NOT NULL,
    hash                 BYTEA   NOT NULL,
    mass                 INTEGER NOT NULL,
    block_hash           BYTEA[] NOT NULL,
    block_time           INTEGER NOT NULL,
    is_accepted          BOOLEAN NOT NULL,
    accepting_block_hash BYTEA,
    CONSTRAINT fk_subnetwork FOREIGN KEY (subnetwork) REFERENCES subnetworks (id)
);
