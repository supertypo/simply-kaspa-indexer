----------------------------------------
-- v4: Fix subnetwork.id type mismatch
----------------------------------------

-- Migrated transaction.subnetwork_id is already INT4, change subnetwork.id to match
ALTER TABLE subnetworks ALTER COLUMN id TYPE INTEGER;

-- If db wasn't migrated transaction.subnetwork_id also needs to be changed
ALTER TABLE transactions ALTER COLUMN subnetwork_id TYPE INTEGER;

-- Update schema_version
UPDATE vars SET value = '4' WHERE key = 'schema_version';
