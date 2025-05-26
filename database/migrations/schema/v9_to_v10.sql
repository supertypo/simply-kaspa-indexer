--------------------------------------------------------------
-- v10: Support data retention period
--------------------------------------------------------------

-- Update schema_version
UPDATE vars SET value = '10' WHERE key = 'schema_version';
