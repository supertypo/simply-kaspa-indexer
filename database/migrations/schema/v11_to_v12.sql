-- Schema upgrade from v11 to v12
-- Adds category classification to tag_providers table

-- Add category column
ALTER TABLE tag_providers ADD COLUMN IF NOT EXISTS category VARCHAR(50);
CREATE INDEX IF NOT EXISTS idx_tag_providers_category ON tag_providers(category);

-- Update schema version
UPDATE vars SET value = '12' WHERE key = 'schema_version';
