--------------------------------------------------------------
-- v24: Toccata rollup freshness
--------------------------------------------------------------

ALTER TABLE toccata_metrics ADD COLUMN updated_at BIGINT;

UPDATE toccata_metrics
SET updated_at = (EXTRACT(EPOCH FROM NOW()) * 1000)::BIGINT
WHERE value > 0;

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

-- Update schema_version
UPDATE vars SET value = '24' WHERE key = 'schema_version';
