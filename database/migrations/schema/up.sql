CREATE TABLE vars
(
    key   VARCHAR(255) PRIMARY KEY,
    value TEXT NOT NULL
);
INSERT INTO vars (key, value)
VALUES ('schema_version', '10');


CREATE TABLE blocks
(
    hash                    BIT(256) PRIMARY KEY,
    accepted_id_merkle_root BIT(256),
    merge_set_blues_hashes  BIT(256)[],
    merge_set_reds_hashes   BIT(256)[],
    selected_parent_hash    BIT(256),
    bits                    BIGINT,
    blue_score              BIGINT,
    blue_work               BYTEA,
    daa_score               BIGINT,
    hash_merkle_root        BIT(256),
    nonce                   BYTEA,
    pruning_point           BIT(256),
    "timestamp"             BIGINT,
    utxo_commitment         BIT(256),
    version                 SMALLINT
);
CREATE INDEX ON blocks (blue_score);


CREATE TABLE block_parent
(
    block_hash  BIT(256),
    parent_hash BIT(256),
    PRIMARY KEY (block_hash, parent_hash)
);
CREATE INDEX ON block_parent (parent_hash);


CREATE TABLE subnetworks
(
    id            SERIAL PRIMARY KEY,
    subnetwork_id VARCHAR(40) NOT NULL
);


CREATE TABLE utxos
(
    transaction_id            BIT(256),
    index                     SMALLINT,
    amount                    BIGINT,
    script_public_key         BYTEA,
    script_public_key_address TEXT
);


CREATE TYPE transactions_inputs AS
(
    index                    SMALLINT,
    previous_outpoint_hash   BIT(256),
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
    script_public_key_address TEXT
);


CREATE TABLE transactions
(
    transaction_id BIT(256) PRIMARY KEY,
    subnetwork_id  INTEGER,
    hash           BIT(256),
    mass           INTEGER,
    payload        BYTEA,
    block_time     BIGINT,
    inputs         transactions_inputs[],
    outputs        transactions_outputs[]
);
CREATE INDEX ON transactions (block_time DESC);


CREATE TABLE transactions_acceptances
(
    transaction_id BIT(256) UNIQUE,
    block_hash     BIT(256)
);
CREATE INDEX ON transactions_acceptances (block_hash);


CREATE TABLE blocks_transactions
(
    block_hash     BIT(256),
    transaction_id BIT(256),
    PRIMARY KEY (block_hash, transaction_id)
);
CREATE INDEX ON blocks_transactions (transaction_id);


CREATE TABLE addresses_transactions
(
    address        VARCHAR,
    transaction_id BIT(256),
    block_time     BIGINT,
    PRIMARY KEY (address, transaction_id)
);
CREATE INDEX ON addresses_transactions (address, block_time DESC);


CREATE TABLE scripts_transactions
(
    script_public_key BYTEA,
    transaction_id    BIT(256),
    block_time        BIGINT,
    PRIMARY KEY (script_public_key, transaction_id)
);
CREATE INDEX ON scripts_transactions (script_public_key, block_time DESC);
