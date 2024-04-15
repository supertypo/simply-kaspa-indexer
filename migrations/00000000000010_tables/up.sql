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
    is_chain_block          BOOLEAN NOT NULL DEFAULT false,
    merge_set_blues_hashes  BYTEA[],
    merge_set_reds_hashes   BYTEA[],
    selected_parent_hash    BYTEA,
    bits                    BIGINT,
    blue_score              BIGINT,
    blue_work               BYTEA,
    daa_score               BIGINT,
    hash_merkle_root        BYTEA,
    nonce                   NUMERIC(32, 0),
    parents                 BYTEA[],
    pruning_point           BYTEA,
    "timestamp"             INTEGER,
    utxo_commitment         BYTEA,
    version                 SMALLINT
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
    block_time           INTEGER,
    is_accepted          BOOLEAN NOT NULL,
    accepting_block_hash BYTEA,
    CONSTRAINT fk_subnetwork FOREIGN KEY (subnetwork) REFERENCES subnetworks (id)
);

CREATE INDEX IF NOT EXISTS block_time_idx ON transactions (block_time);
CREATE INDEX IF NOT EXISTS idx_accepting_block ON transactions (accepting_block_hash);


CREATE TABLE IF NOT EXISTS "blocks_transactions"
(
    block_hash     BYTEA NOT NULL,
    transaction_id BYTEA NOT NULL,
    CONSTRAINT pk_blocks_transactions PRIMARY KEY (block_hash, transaction_id),
    CONSTRAINT fk_transaction_id FOREIGN KEY (transaction_id) REFERENCES transactions (transaction_id)
);

CREATE INDEX IF NOT EXISTS idx_block_hash ON blocks_transactions (block_hash);
CREATE INDEX IF NOT EXISTS idx_transaction_id ON blocks_transactions (transaction_id);


CREATE TABLE IF NOT EXISTS "transactions_inputs"
(
    transaction_id            BYTEA        NOT NULL,
    index                     SMALLINT     NOT NULL,
    previous_outpoint_hash    BYTEA        NOT NULL,
    previous_outpoint_index   SMALLINT     NOT NULL,
    script_public_key_address VARCHAR(128),
    signature_script          BYTEA        NOT NULL,
    sig_op_count              SMALLINT     NOT NULL,
    CONSTRAINT pk_transactions_inputs PRIMARY KEY (transaction_id, index)
);

CREATE INDEX IF NOT EXISTS idx_txin_prev ON transactions_inputs (previous_outpoint_hash);
CREATE INDEX IF NOT EXISTS idx_txin ON transactions_inputs (transaction_id);


CREATE TABLE IF NOT EXISTS "transactions_outputs"
(
    transaction_id            BYTEA        NOT NULL,
    index                     SMALLINT     NOT NULL,
    amount                    BIGINT       NOT NULL,
    script_public_key         BYTEA        NOT NULL,
    script_public_key_address VARCHAR(128) NOT NULL,
    script_public_key_type    VARCHAR(32)  NOT NULL,
    CONSTRAINT pk_transactions_outputs PRIMARY KEY (transaction_id, index)
);

CREATE INDEX IF NOT EXISTS idx_txouts ON transactions_outputs (transaction_id);
CREATE INDEX IF NOT EXISTS idx_txouts_addr ON transactions_outputs (script_public_key_address);
