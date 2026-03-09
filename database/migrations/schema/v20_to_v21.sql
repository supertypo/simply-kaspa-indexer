ALTER TABLE transactions DROP COLUMN outputs_spent;

DROP TABLE utxos;

UPDATE vars SET value = '21' WHERE key = 'schema_version';
