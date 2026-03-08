ALTER TABLE transactions DROP COLUMN outputs_spent;

UPDATE vars SET value = '21' WHERE key = 'schema_version';
