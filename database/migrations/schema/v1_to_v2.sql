----------------------------------------
-- v2: Indexed parent-child relations
----------------------------------------

-- Create a new table for block to parent block mappings
CREATE TABLE block_parents
(
    block_hash  BYTEA,
    parent_hash BYTEA
);

-- Insert mappings from blocks.parents
INSERT INTO block_parents (block_hash, parent_hash)
SELECT b.hash AS block_hash,
       p      AS parent_hash
FROM blocks b CROSS JOIN LATERAL UNNEST(b.parents) AS p;

-- Create constraints/indexes
ALTER TABLE block_parents ADD PRIMARY KEY (block_hash, parent_hash);
CREATE INDEX ON block_parents (block_hash);
CREATE INDEX ON block_parents (parent_hash);

-- Drop obsolete columns
ALTER TABLE blocks DROP COLUMN parents;
