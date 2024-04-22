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
    "timestamp"             INTEGER,
    utxo_commitment         BYTEA,
    version                 SMALLINT
);
CREATE INDEX IF NOT EXISTS idx_blocks_blue_score ON blocks (blue_score);


CREATE TABLE IF NOT EXISTS "chain_blocks"
(
    block_hash BYTEA PRIMARY KEY
);


CREATE TABLE IF NOT EXISTS "subnetworks"
(
    id            SMALLSERIAL PRIMARY KEY,
    subnetwork_id VARCHAR(40) NOT NULL
);


CREATE TABLE IF NOT EXISTS "transactions"
(
    transaction_id BYTEA PRIMARY KEY,
    subnetwork_id  SMALLINT,
    hash           BYTEA,
    mass           INTEGER,
    block_time     INTEGER
);
CREATE INDEX IF NOT EXISTS idx_transactions_block_time ON transactions (block_time);


CREATE TABLE IF NOT EXISTS "transactions_acceptances"
(
    transaction_id BYTEA PRIMARY KEY,
    block_hash     BYTEA NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_transactions_acceptances_accepting_block ON transactions_acceptances (block_hash);


CREATE TABLE IF NOT EXISTS "blocks_transactions"
(
    block_hash     BYTEA NOT NULL,
    transaction_id BYTEA NOT NULL,
    CONSTRAINT pk_blocks_transactions PRIMARY KEY (block_hash, transaction_id)
);
CREATE INDEX IF NOT EXISTS idx_blocks_transactions_block_hash ON blocks_transactions (block_hash);
CREATE INDEX IF NOT EXISTS idx_blocks_transactions_transaction_id ON blocks_transactions (transaction_id);


CREATE TABLE IF NOT EXISTS "transactions_inputs"
(
    transaction_id          BYTEA    NOT NULL,
    index                   SMALLINT NOT NULL,
    previous_outpoint_hash  BYTEA    NOT NULL,
    previous_outpoint_index SMALLINT NOT NULL,
    signature_script        BYTEA    NOT NULL,
    sig_op_count            SMALLINT NOT NULL,
    CONSTRAINT pk_transactions_inputs PRIMARY KEY (transaction_id, index)
);
CREATE INDEX IF NOT EXISTS idx_transactions_inputs_transaction_id ON transactions_inputs (transaction_id);
CREATE INDEX IF NOT EXISTS idx_transactions_inputs_previous_outpoint_hash ON transactions_inputs (previous_outpoint_hash);


CREATE TABLE IF NOT EXISTS "transactions_outputs"
(
    transaction_id            BYTEA    NOT NULL,
    index                     SMALLINT NOT NULL,
    amount                    BIGINT   NOT NULL,
    script_public_key         BYTEA    NOT NULL,
    script_public_key_address VARCHAR  NOT NULL,
    script_public_key_type    VARCHAR  NOT NULL,
    block_time                INTEGER  NOT NULL,
    CONSTRAINT pk_transactions_outputs PRIMARY KEY (transaction_id, index)
);
CREATE INDEX IF NOT EXISTS idx_transactions_outputs_transaction_id ON transactions_outputs (transaction_id);
CREATE INDEX IF NOT EXISTS idx_transactions_outputs_script_public_key_address ON transactions_outputs (script_public_key_address);
