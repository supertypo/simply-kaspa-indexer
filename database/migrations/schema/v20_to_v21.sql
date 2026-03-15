--------------------------------------------------------------
-- v21: Vspc v2 related improvements
--------------------------------------------------------------

-- Remove spent output tracking
ALTER TABLE transactions DROP COLUMN outputs_spent;


-- Removed initial utxo import
DROP TABLE utxos;


-- Add tx version column (NULL maps to version 0; no rewrite needed for existing rows)
ALTER TABLE transactions ADD COLUMN version SMALLINT;


-- Migrate transactions.subnetwork_id from INTEGER FK to BYTEA (trailing-zeros stripped)
CREATE TEMP TABLE subnetwork_compressed AS
SELECT id, NULLIF(rtrim(decode(subnetwork_id, 'hex'), '\x00'::bytea), ''::bytea) AS compressed
FROM subnetworks
WHERE NULLIF(rtrim(decode(subnetwork_id, 'hex'), '\x00'::bytea), ''::bytea) IS NOT NULL;

ALTER TABLE transactions ADD COLUMN subnetwork_id_new BYTEA;

CREATE OR REPLACE PROCEDURE _v21_migrate_subnetworks()
LANGUAGE plpgsql
AS $$
DECLARE
    batch_ms      BIGINT := 24 * 3600 * 1000;
    t_min         BIGINT;
    t_max         BIGINT;
    t_cur         BIGINT;
    checkpoint    TEXT;
    rows_updated  BIGINT;
    total_updated BIGINT := 0;
    subnet_count  INTEGER;
BEGIN
    SELECT COUNT(*) INTO subnet_count FROM subnetwork_compressed;
    RAISE NOTICE 'Subnetworks to migrate (non-native): %', subnet_count;

    SELECT value INTO checkpoint FROM vars WHERE key = 'v21_compressed_subnetworks_checkpoint';

    SELECT MAX(block_time) INTO t_max FROM transactions;

    IF checkpoint IS NOT NULL THEN
        t_cur := checkpoint::BIGINT;
        RAISE NOTICE 'Resuming from checkpoint: % (%)', t_cur, to_char(to_timestamp(t_cur / 1000), 'YYYY-MM-DD');
    ELSE
        SELECT MIN(block_time) INTO t_min FROM transactions;
        t_cur := t_min;
    END IF;

    LOOP
        UPDATE transactions t
        SET subnetwork_id_new = sub.compressed
        FROM subnetwork_compressed sub
        WHERE t.block_time >= t_cur
          AND t.block_time <  t_cur + batch_ms
          AND t.subnetwork_id = sub.id;

        GET DIAGNOSTICS rows_updated = ROW_COUNT;
        total_updated := total_updated + rows_updated;
        RAISE NOTICE '% (%): % rows (total: %)',
            t_cur,
            to_char(to_timestamp(t_cur / 1000), 'YYYY-MM-DD'),
            rows_updated,
            total_updated;

        INSERT INTO vars (key, value) VALUES ('v21_compressed_subnetworks_checkpoint', t_cur::TEXT)
        ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value;

        COMMIT;

        t_cur := t_cur + batch_ms;
        EXIT WHEN t_cur > t_max;
    END LOOP;

    RAISE NOTICE 'Done: % total rows', total_updated;
END $$;

CALL _v21_migrate_subnetworks();
DROP PROCEDURE _v21_migrate_subnetworks();

DELETE FROM vars WHERE key = 'v21_compressed_subnetworks_checkpoint';
DROP TABLE subnetwork_compressed;

ALTER TABLE transactions DROP COLUMN subnetwork_id;
ALTER TABLE transactions RENAME COLUMN subnetwork_id_new TO subnetwork_id;
DROP TABLE subnetworks;


-- Update schema_version
UPDATE vars SET value = '21' WHERE key = 'schema_version';
