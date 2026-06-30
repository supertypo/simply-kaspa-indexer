CREATE TABLE vars
(
    key   VARCHAR(255) PRIMARY KEY,
    value TEXT NOT NULL
);
INSERT INTO vars (key, value)
VALUES ('schema_version', '24');


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


CREATE TYPE transactions_inputs AS
(
    index                    SMALLINT,
    previous_outpoint_hash   BYTEA,
    previous_outpoint_index  SMALLINT,
    signature_script         BYTEA,
    sig_op_count             SMALLINT,
    previous_outpoint_script BYTEA,
    previous_outpoint_amount BIGINT,
    compute_budget           SMALLINT,
    covenant_id              BYTEA
);


CREATE TYPE transactions_outputs AS
(
    index                      SMALLINT,
    amount                     BIGINT,
    script_public_key          BYTEA,
    script_public_key_address  TEXT,
    covenant_authorizing_input SMALLINT,
    covenant_id                BYTEA
);


CREATE TABLE transactions
(
    transaction_id BYTEA PRIMARY KEY,
    subnetwork_id  BYTEA,
    hash           BYTEA,
    mass           INTEGER,
    payload        BYTEA,
    block_time     BIGINT,
    version        SMALLINT,
    inputs         transactions_inputs[],
    outputs        transactions_outputs[]
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


CREATE TABLE addresses_transactions
(
    address        TEXT,
    transaction_id BYTEA,
    block_time     BIGINT,
    PRIMARY KEY (address, block_time, transaction_id)
);
CREATE INDEX ON addresses_transactions (block_time DESC);


CREATE TABLE scripts_transactions
(
    script_public_key BYTEA,
    transaction_id    BYTEA,
    block_time        BIGINT,
    PRIMARY KEY (script_public_key, block_time, transaction_id)
);
CREATE INDEX ON scripts_transactions (block_time DESC);


CREATE TABLE toccata_metrics
(
    key   TEXT PRIMARY KEY,
    value BIGINT NOT NULL DEFAULT 0,
    updated_at BIGINT
);

INSERT INTO toccata_metrics (key, value)
VALUES
    ('tx_v1_count', 0),
    ('block_v2_count', 0),
    ('covenant_tx_count', 0),
    ('covenant_input_count', 0),
    ('covenant_output_count', 0),
    ('user_lane_tx_count', 0),
    ('seq_commit_block_count', 0);

CREATE TABLE toccata_covenants
(
    covenant_id       BYTEA PRIMARY KEY,
    tx_count          BIGINT NOT NULL DEFAULT 0,
    input_count       BIGINT NOT NULL DEFAULT 0,
    output_count      BIGINT NOT NULL DEFAULT 0,
    latest_tx_id      BYTEA,
    latest_block_time BIGINT
);

CREATE TABLE toccata_lanes
(
    lane_key          BYTEA PRIMARY KEY,
    tx_count          BIGINT NOT NULL DEFAULT 0,
    latest_tx_id      BYTEA,
    latest_block_time BIGINT
);

CREATE OR REPLACE FUNCTION bump_toccata_metric(metric_key TEXT, delta BIGINT)
RETURNS VOID AS $$
DECLARE
    now_ms BIGINT := (EXTRACT(EPOCH FROM NOW()) * 1000)::BIGINT;
BEGIN
    IF delta = 0 THEN
        RETURN;
    END IF;

    INSERT INTO toccata_metrics (key, value, updated_at)
    VALUES (metric_key, delta, now_ms)
    ON CONFLICT (key) DO UPDATE
    SET value = toccata_metrics.value + EXCLUDED.value,
        updated_at = EXCLUDED.updated_at;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION update_toccata_block_metrics()
RETURNS TRIGGER AS $$
BEGIN
    IF NEW.version >= 2 THEN
        PERFORM bump_toccata_metric('block_v2_count', 1);

        IF NEW.accepted_id_merkle_root IS NOT NULL THEN
            PERFORM bump_toccata_metric('seq_commit_block_count', 1);
        END IF;
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER blocks_toccata_metrics_insert
AFTER INSERT ON blocks
FOR EACH ROW
EXECUTE FUNCTION update_toccata_block_metrics();

CREATE OR REPLACE FUNCTION update_toccata_transaction_metrics()
RETURNS TRIGGER AS $$
DECLARE
    covenant_inputs BIGINT;
    covenant_outputs BIGINT;
BEGIN
    IF NEW.version >= 1 THEN
        PERFORM bump_toccata_metric('tx_v1_count', 1);
    END IF;

    IF NEW.subnetwork_id IS NOT NULL AND octet_length(NEW.subnetwork_id) BETWEEN 1 AND 4 THEN
        PERFORM bump_toccata_metric('user_lane_tx_count', 1);

        INSERT INTO toccata_lanes (lane_key, tx_count, latest_tx_id, latest_block_time)
        VALUES (NEW.subnetwork_id, 1, NEW.transaction_id, NEW.block_time)
        ON CONFLICT (lane_key) DO UPDATE
        SET tx_count = toccata_lanes.tx_count + 1,
            latest_tx_id = CASE
                WHEN EXCLUDED.latest_block_time >= COALESCE(toccata_lanes.latest_block_time, -1)
                THEN EXCLUDED.latest_tx_id
                ELSE toccata_lanes.latest_tx_id
            END,
            latest_block_time = GREATEST(COALESCE(toccata_lanes.latest_block_time, -1), EXCLUDED.latest_block_time);
    END IF;

    SELECT COUNT(*)
    INTO covenant_inputs
    FROM unnest(NEW.inputs) AS i
    WHERE (i).covenant_id IS NOT NULL;

    SELECT COUNT(*)
    INTO covenant_outputs
    FROM unnest(NEW.outputs) AS o
    WHERE (o).covenant_id IS NOT NULL;

    PERFORM bump_toccata_metric('covenant_input_count', covenant_inputs);
    PERFORM bump_toccata_metric('covenant_output_count', covenant_outputs);

    IF covenant_inputs + covenant_outputs > 0 THEN
        PERFORM bump_toccata_metric('covenant_tx_count', 1);

        INSERT INTO toccata_covenants (covenant_id, tx_count, input_count, output_count, latest_tx_id, latest_block_time)
        WITH covenant_events AS (
            SELECT (i).covenant_id, 1::BIGINT AS input_count, 0::BIGINT AS output_count
            FROM unnest(NEW.inputs) AS i
            WHERE (i).covenant_id IS NOT NULL
            UNION ALL
            SELECT (o).covenant_id, 0::BIGINT AS input_count, 1::BIGINT AS output_count
            FROM unnest(NEW.outputs) AS o
            WHERE (o).covenant_id IS NOT NULL
        )
        SELECT
            covenant_id,
            1::BIGINT AS tx_count,
            SUM(input_count)::BIGINT AS input_count,
            SUM(output_count)::BIGINT AS output_count,
            NEW.transaction_id AS latest_tx_id,
            NEW.block_time AS latest_block_time
        FROM covenant_events
        GROUP BY covenant_id
        ON CONFLICT (covenant_id) DO UPDATE
        SET tx_count = toccata_covenants.tx_count + EXCLUDED.tx_count,
            input_count = toccata_covenants.input_count + EXCLUDED.input_count,
            output_count = toccata_covenants.output_count + EXCLUDED.output_count,
            latest_tx_id = CASE
                WHEN EXCLUDED.latest_block_time >= COALESCE(toccata_covenants.latest_block_time, -1)
                THEN EXCLUDED.latest_tx_id
                ELSE toccata_covenants.latest_tx_id
            END,
            latest_block_time = GREATEST(COALESCE(toccata_covenants.latest_block_time, -1), EXCLUDED.latest_block_time);
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER transactions_toccata_metrics_insert
AFTER INSERT ON transactions
FOR EACH ROW
EXECUTE FUNCTION update_toccata_transaction_metrics();
