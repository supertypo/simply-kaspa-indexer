# Kaspa Payload Retrofit

A retroactive tagging tool that applies YAML filter rules to historical transaction data without requiring a full blockchain re-sync. Essential for classifying previously indexed transactions when adding new protocol filters to an existing database.

## Overview

The retrofit tool allows you to:
- **Tag historical data**: Apply new filter rules to transactions already in your database
- **Update classifications**: Re-tag transactions when filter rules change
- **Test safely**: Dry-run mode to preview changes before applying
- **Batch process**: Efficiently update thousands of transactions with configurable batch sizes

## Features

- **Three Operating Modes**:
  - **null-only** (safe): Only updates transactions with NULL tag_id
  - **overwrite** (dangerous): Updates ALL matching transactions, replacing existing tags
  - **report** (dry-run): Shows what would change without modifying database

- **Filter Rule Application**: Uses the same YAML-based filter configuration as the main indexer
- **Tag Cache Integration**: Efficiently maps tag names to database tag_id values
- **Batch Processing**: Configurable batch sizes for optimal performance
- **Progress Tracking**: Real-time progress bars during processing
- **Match Statistics**: Grouped reporting by rule name with sample transactions

## Installation

Build the retrofit tool from the workspace root:

```bash
# Build in release mode for better performance
cargo build --release -p kaspa-payload-retrofit

# The binary will be at:
# ./target/release/kaspa-payload-retrofit
```

## Usage

### Safe Mode: Tag Unclassified Transactions

Apply tags only to transactions with NULL tag_id (recommended default):

```bash
cargo run --release -p kaspa-payload-retrofit -- \
  --database-url "postgres://user:password@localhost/kaspa" \
  --filter-config examples/filters_kasplex_only.yaml \
  --mode null-only
```

### Dry-Run: Preview Changes

Always test with report mode first to see what would change:

```bash
cargo run --release -p kaspa-payload-retrofit -- \
  --database-url "postgres://user:password@localhost/kaspa" \
  --filter-config my_new_rules.yaml \
  --mode report
```

### Overwrite Mode: Re-tag Transactions

⚠️ **DANGEROUS**: This overwrites existing tags. Test with report mode first!

```bash
# Test first
cargo run --release -p kaspa-payload-retrofit -- \
  --database-url "postgres://user:password@localhost/kaspa" \
  --filter-config updated_rules.yaml \
  --mode report \
  --limit 10000

# Then apply if results look correct
cargo run --release -p kaspa-payload-retrofit -- \
  --database-url "postgres://user:password@localhost/kaspa" \
  --filter-config updated_rules.yaml \
  --mode overwrite \
  --batch-size 5000
```

### Custom Batch Processing

Adjust batch size for your database performance:

```bash
cargo run --release -p kaspa-payload-retrofit -- \
  --database-url "postgres://user:password@localhost/kaspa" \
  --filter-config examples/filters_tagging_all.yaml \
  --batch-size 10000  # Default is 1000
```

### Process Limited Range

Test on a subset of transactions:

```bash
cargo run --release -p kaspa-payload-retrofit -- \
  --database-url "postgres://user:password@localhost/kaspa" \
  --filter-config new_protocol.yaml \
  --limit 50000 \
  --mode report
```

## Configuration Options

### Required Arguments

- `--database-url <URL>`: PostgreSQL connection string (format: `postgres://user:password@host:port/database`)
- `--filter-config <PATH>`: Path to YAML filter configuration file

### Optional Arguments

- `--mode <MODE>`: Operating mode (choices: `null-only`, `overwrite`, `report`) (default: `null-only`)
- `--batch-size <N>`: Number of transactions to update per batch (default: 1000)
- `--limit <N>`: Maximum number of transactions to process (default: all)

## Operating Modes

### 1. null-only (Safe Default)

**Behavior**: Only updates transactions where `tag_id IS NULL`

**Use Cases**:
- First-time tagging of historical data
- Adding new protocol filters to existing database
- Filling in gaps after initial indexing

**Safety**: ✅ Safe - never overwrites existing classifications

**Example**:
```bash
cargo run --release -p kaspa-payload-retrofit -- \
  --database-url $DB_URL \
  --filter-config examples/filters_mining_pools.yaml \
  --mode null-only
```

### 2. report (Dry-Run)

**Behavior**: Analyzes what would change but doesn't modify database

**Use Cases**:
- Testing new filter rules before applying
- Validating pattern matches
- Understanding impact of rule changes

**Safety**: ✅ Safe - read-only operation

**Output Example**:
```
Retrofit Report (Dry Run Mode)

Grouped by matched rule:
  kasplex-protocol: 45,231 matches
  Sample transactions:
    - abc123...
    - def456...

  mining-pool-f2pool: 21,347 matches
  Sample transactions:
    - 789xyz...

Total matches: 66,578
```

**Example**:
```bash
cargo run --release -p kaspa-payload-retrofit -- \
  --database-url $DB_URL \
  --filter-config my_rules.yaml \
  --mode report
```

### 3. overwrite (Dangerous)

**Behavior**: Updates ALL matching transactions, replacing existing tags

**Use Cases**:
- Fixing incorrect classifications from old rules
- Re-prioritizing filter rules (higher priority rule should win)
- Migrating from old filter schema to new schema

**Safety**: ⚠️ DANGEROUS - overwrites existing data

**Precautions**:
1. Always test with `--mode report` first
2. Test on limited subset with `--limit 10000`
3. Backup database before running
4. Verify results with queries after applying

**Example**:
```bash
# Step 1: Test with report mode
cargo run --release -p kaspa-payload-retrofit -- \
  --database-url $DB_URL \
  --filter-config updated_rules.yaml \
  --mode report

# Step 2: Test on small subset
cargo run --release -p kaspa-payload-retrofit -- \
  --database-url $DB_URL \
  --filter-config updated_rules.yaml \
  --mode overwrite \
  --limit 1000

# Step 3: Verify results
psql $DB_URL -c "SELECT tag, COUNT(*) FROM transactions t
                  JOIN tag_providers tp ON t.tag_id = tp.id
                  GROUP BY tag ORDER BY count DESC LIMIT 20"

# Step 4: Apply to all if satisfied
cargo run --release -p kaspa-payload-retrofit -- \
  --database-url $DB_URL \
  --filter-config updated_rules.yaml \
  --mode overwrite
```

## Typical Workflow

### Scenario: Adding New Protocol Filter

#### Step 1: Discover Patterns

Use the analyzer tool to identify patterns:

```bash
cargo run --release -p kaspa-payload-analyzer -- \
  --database-url $DB_URL \
  --untagged-only true \
  --min-count 100 \
  --generate-rules \
  --output new_protocol.yaml
```

#### Step 2: Refine YAML Rules

Edit `new_protocol.yaml`:
- Set meaningful tag names
- Adjust priorities
- Add module and category metadata
- Set `enabled: true`
- Consider `match_type: contains` for substring matching

#### Step 3: Test with Report Mode

```bash
cargo run --release -p kaspa-payload-retrofit -- \
  --database-url $DB_URL \
  --filter-config new_protocol.yaml \
  --mode report
```

Review the output to ensure patterns match correctly.

#### Step 4: Apply to Historical Data

```bash
cargo run --release -p kaspa-payload-retrofit -- \
  --database-url $DB_URL \
  --filter-config new_protocol.yaml \
  --mode null-only
```

#### Step 5: Verify Results

```sql
-- Check tag distribution
SELECT tp.tag, tp.module, COUNT(*) as tx_count
FROM transactions t
JOIN tag_providers tp ON t.tag_id = tp.id
WHERE tp.tag = 'your_new_tag'
GROUP BY tp.tag, tp.module;

-- Sample tagged transactions
SELECT t.transaction_id, t.payload, tp.tag
FROM transactions t
JOIN tag_providers tp ON t.tag_id = tp.id
WHERE tp.tag = 'your_new_tag'
LIMIT 10;
```

#### Step 6: Enable for Future Indexing

Add to main indexer configuration:

```bash
cargo run --release -p simply-kaspa-indexer -- \
  --database-url $DB_URL \
  --filter-config new_protocol.yaml
```

## Filter Rule Application Logic

The retrofit tool applies the same filter matching logic as the main indexer:

```rust
// Rules are processed in priority order (highest first)
for rule in sorted_enabled_rules {
    // Check TXID condition (if present)
    if rule.txid_matches(transaction_id) {
        // Check payload conditions (OR logic)
        if rule.payload_matches(payload) {
            return rule.tag;  // First match wins
        }
    }
}
```

**Key Points**:
- Rules processed by **priority** (highest number first)
- **First matching rule** determines the tag
- TXID and payload conditions use **AND** logic
- Multiple payload conditions use **OR** logic
- Three match types: `prefix`, `contains`, `regex`

## Database Requirements

The retrofit tool requires:
- PostgreSQL database with indexed transaction data
- Schema version 12+ (with `tag_id` and `tag_providers` table)
- Transactions with payload data stored
- Tag providers pre-populated (managed by main indexer)

## Performance Considerations

- **Batch Size**: Default 1000 is conservative
  - Increase to 5000-10000 for large databases with good hardware
  - Decrease to 500 if experiencing database contention

- **Processing Speed**: ~5K-10K transactions/second
  - Bottleneck: Usually database UPDATE performance
  - Optimization: Ensure index on `transaction_id` (primary key)

- **Memory Usage**: O(batch_size) + O(filter_rules)
  - Minimal memory footprint
  - Tag cache is small (typically <1MB)

- **Transaction Isolation**: Each batch is a separate transaction
  - Allows progress to be saved incrementally
  - Can resume after interruption

## Tag Cache and Tag Providers

The retrofit tool uses the `tag_providers` table to map filter rule tags to database `tag_id` values:

```sql
-- Tag providers table structure
CREATE TABLE tag_providers (
    id SERIAL PRIMARY KEY,
    tag VARCHAR(50) NOT NULL,
    module VARCHAR(50),
    category VARCHAR(50),
    -- ... other metadata
);
```

**Important**:
- Tags in your YAML filter config must exist in `tag_providers` table
- Main indexer automatically creates tag_providers entries from filter config
- If a tag is missing, retrofit will skip that rule and log a warning
- Run main indexer with your filter config at least once before using retrofit

## Troubleshooting

### No Matches Found

If retrofit reports zero matches:
- Check that filter config paths and patterns are correct
- Verify transactions have non-NULL `payload` column
- Test with `--mode report` to see detailed match information
- Check that tag exists in `tag_providers` table
- Review filter rule `enabled: true` status

### Tag Not Found Warning

If you see "Tag 'xyz' not found in tag cache":
- Run main indexer with that filter config first
- Indexer will create missing tag_providers entries
- Or manually insert into tag_providers table

### Slow Performance

To improve processing speed:
- Increase `--batch-size` (try 5000 or 10000)
- Ensure database has good hardware (SSD, adequate RAM)
- Run during low-traffic periods
- Consider using database replica for retrofit operations

### Memory Issues

If retrofit consumes excessive memory:
- Decrease `--batch-size` to 500 or lower
- Use `--limit` to process in chunks
- Check for memory leaks in complex regex patterns

## Safety Best Practices

1. **Always test with report mode first**
   ```bash
   --mode report  # Before any database changes
   ```

2. **Start with limited scope**
   ```bash
   --limit 1000  # Test on small subset
   ```

3. **Use null-only mode by default**
   ```bash
   --mode null-only  # Safest option
   ```

4. **Backup before overwrite mode**
   ```bash
   pg_dump $DB_URL > backup.sql  # Before using --mode overwrite
   ```

5. **Verify results after applying**
   ```sql
   SELECT tag, COUNT(*) FROM transactions t
   JOIN tag_providers tp ON t.tag_id = tp.id
   GROUP BY tag ORDER BY count DESC;
   ```

## Examples

### Tag Mining Pool Transactions

```bash
# Generate mining pool filter rules (or use existing)
cargo run --release -p kaspa-payload-retrofit -- \
  --database-url $DB_URL \
  --filter-config examples/filters_mining_pools.yaml \
  --mode null-only
```

### Update Kasplex Classifications

```bash
# If Kasplex filter rules changed
cargo run --release -p kaspa-payload-retrofit -- \
  --database-url $DB_URL \
  --filter-config examples/filters_kasplex_only.yaml \
  --mode overwrite \
  --batch-size 5000
```

### Retroactive Tagging for All Protocols

```bash
# Apply comprehensive filter config to entire database
cargo run --release -p kaspa-payload-retrofit -- \
  --database-url $DB_URL \
  --filter-config examples/filters_tagging_all.yaml \
  --mode null-only \
  --batch-size 10000
```

## Integration with Main Indexer

The retrofit tool is designed to work seamlessly with the main indexer:

```
┌─────────────────────────────────────────────────┐
│  Historical Data (already in database)          │
│            ↓                                     │
│      ANALYZER (discover patterns)                │
│            ↓                                     │
│  Filter Rules YAML (create/refine)               │
│            ↓                                     │
│      RETROFIT (apply to historical)              │
│                                                  │
│  ← ← ← ← ← ← ← ← ← ← ← ← ← ← ←                 │
│                                                  │
│  New Data (live indexing)                        │
│            ↓                                     │
│      MAIN INDEXER (real-time tagging)            │
└─────────────────────────────────────────────────┘
```

Both tools share the same:
- YAML filter configuration format
- Filter matching logic (prefix/contains/regex)
- Tag provider integration
- Priority-based rule evaluation

## Related Tools

- **kaspa-payload-analyzer**: Discover patterns and generate filter rules
- **simply-kaspa-indexer**: Main indexer with real-time filter application

## Support

For issues or questions:
- Check the main project README
- Review existing filter configurations in `examples/` directory
- Examine the analyzer tool for pattern discovery
- Test with `--mode report` before making database changes
