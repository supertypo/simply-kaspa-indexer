-- Rollback Category Column Migration v12

-- Drop category index
DROP INDEX IF EXISTS idx_tag_providers_category;

-- Drop category column
ALTER TABLE tag_providers DROP COLUMN IF EXISTS category;
