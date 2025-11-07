-- Add Category Column to Tag Providers Migration v12
-- Adds category field for protocol classification (mining, l2_gaming, social, etc.)

-- Add category column to tag_providers table
ALTER TABLE tag_providers ADD COLUMN IF NOT EXISTS category VARCHAR(50);

-- Create index for efficient category-based queries
CREATE INDEX IF NOT EXISTS idx_tag_providers_category ON tag_providers(category);
