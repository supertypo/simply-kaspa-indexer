# Kaspa Payload Analyzer

A discovery and analysis tool for identifying recurring payload patterns in Kaspa transaction data. This tool helps operators discover new protocols, understand payload usage patterns, and generate filter rules for previously unknown transaction types.

## Overview

The analyzer scans historical transaction data stored in your PostgreSQL database to identify:
- **Text protocols**: UTF-8 encoded payloads with recognizable text prefixes
- **Binary protocols**: Non-UTF-8 payloads with hex prefix patterns
- **Frequency patterns**: How often each pattern appears in the blockchain
- **Sample data**: Transaction IDs and payload examples for each pattern

## Features

- **Pattern Discovery**: Automatically identifies recurring prefix patterns in transaction payloads
- **Dual Format Support**: Handles both text (UTF-8) and binary (hex) payload prefixes
- **Frequency Analysis**: Ranks patterns by occurrence count
- **Sample Collection**: Stores up to 5 sample transaction IDs and 3 sample payloads per pattern
- **Untagged Focus**: Optionally analyzes only unclassified transactions (NULL tag_id)
- **Multiple Output Modes**:
  - **Report mode**: Generates detailed markdown reports with pattern statistics
  - **YAML rules mode**: Auto-generates filter configuration files for discovered patterns
- **Progress Tracking**: Real-time progress bars during analysis

## Installation

Build the analyzer from the workspace root:

```bash
# Build in release mode for better performance
cargo build --release -p kaspa-payload-analyzer

# The binary will be at:
# ./target/release/kaspa-payload-analyzer
```

## Usage

### Basic Pattern Discovery

Discover patterns in untagged transactions:

```bash
cargo run --release -p kaspa-payload-analyzer -- \
  --database-url "postgres://user:password@localhost/kaspa" \
  --output patterns_report.md
```

### Generate YAML Filter Rules

Auto-generate filter configuration from discovered patterns:

```bash
cargo run --release -p kaspa-payload-analyzer -- \
  --database-url "postgres://user:password@localhost/kaspa" \
  --min-count 100 \
  --generate-rules \
  --output discovered_filters.yaml
```

### Analyze Recent Transactions Only

Limit analysis to most recent N transactions:

```bash
cargo run --release -p kaspa-payload-analyzer -- \
  --database-url "postgres://user:password@localhost/kaspa" \
  --limit 100000 \
  --output recent_patterns.md
```

### Custom Prefix Lengths

Adjust prefix extraction lengths for better pattern matching:

```bash
cargo run --release -p kaspa-payload-analyzer -- \
  --database-url "postgres://user:password@localhost/kaspa" \
  --text-prefix-length 30 \
  --hex-prefix-length 12 \
  --output detailed_patterns.md
```

### Analyze All Transactions (Including Tagged)

By default, only untagged transactions are analyzed. To include all:

```bash
cargo run --release -p kaspa-payload-analyzer -- \
  --database-url "postgres://user:password@localhost/kaspa" \
  --untagged-only false \
  --output all_patterns.md
```

## Configuration Options

### Required Arguments

- `--database-url <URL>`: PostgreSQL connection string (format: `postgres://user:password@host:port/database`)
- `--output <PATH>`: Output file path for report or YAML rules

### Optional Arguments

- `--min-count <N>`: Minimum occurrences to include pattern in output (default: 10)
- `--text-prefix-length <N>`: Length of text prefixes to extract (default: 20)
- `--hex-prefix-length <N>`: Length of hex prefixes to extract in bytes (default: 8)
- `--limit <N>`: Maximum number of transactions to analyze (default: all)
- `--untagged-only <BOOL>`: Only analyze untagged transactions (default: true)
- `--generate-rules`: Generate YAML filter rules instead of markdown report

## Output Formats

### Markdown Report

When run without `--generate-rules`, produces a detailed report:

```markdown
# Kaspa Payload Pattern Analysis

Analysis of 150,000 transactions with payloads
Found 25 distinct patterns (minimum 10 occurrences)

## Pattern Rankings

### 1. kasplex (Text) - 45,231 occurrences
Prefix: `kasplex`
Hex: 6b617370...

Sample transactions:
- abc123...
- def456...

Sample payloads (truncated):
- kasplex:op=deploy...
- kasplex:op=mint...
```

### YAML Filter Rules

When run with `--generate-rules`, produces filter configuration:

```yaml
version: "1.0"

settings:
  default_store_payload: true

rules:
  - name: pattern_1
    priority: 300
    enabled: false  # Enable manually after review
    tag: pattern_1
    module: discovered
    category: unknown
    store_payload: true
    conditions:
      payload:
        - prefix: "kasplex"
          match_type: prefix
```

**Note**: All generated rules are initially `enabled: false` for safety. Review and enable them manually.

## Typical Workflow

### 1. Discovery Phase

Run the analyzer on untagged transactions:

```bash
cargo run --release -p kaspa-payload-analyzer -- \
  --database-url $DATABASE_URL \
  --untagged-only true \
  --min-count 100 \
  --output discovery_report.md
```

Review the report to identify interesting protocol patterns.

### 2. Rule Generation

Generate initial YAML rules:

```bash
cargo run --release -p kaspa-payload-analyzer -- \
  --database-url $DATABASE_URL \
  --min-count 100 \
  --generate-rules \
  --output discovered_protocols.yaml
```

### 3. Rule Refinement

Manually edit the generated YAML file:
- Set meaningful tag names (replace `pattern_1`, `pattern_2`)
- Add proper module names and categories
- Adjust priorities based on protocol importance
- Add repository URLs and descriptions
- Set `enabled: true` for rules you want to use
- Consider changing `match_type` from `prefix` to `contains` if needed

### 4. Apply to Historical Data

Use the retrofit tool to apply tags to existing transactions:

```bash
cargo run --release -p kaspa-payload-retrofit -- \
  --database-url $DATABASE_URL \
  --filter-config discovered_protocols.yaml \
  --mode report  # Test first

# Then apply:
cargo run --release -p kaspa-payload-retrofit -- \
  --database-url $DATABASE_URL \
  --filter-config discovered_protocols.yaml \
  --mode null-only
```

### 5. Enable for Future Indexing

Add the filter config to your main indexer:

```bash
cargo run --release -p simply-kaspa-indexer -- \
  --database-url $DATABASE_URL \
  --filter-config discovered_protocols.yaml
```

## Integration with Filter System

The analyzer integrates with the YAML-based transaction filtering system:

```
Historical Data → Analyzer → Filter Rules YAML → Retrofit → Tagged Database
                                    ↓
                            Main Indexer (ongoing)
```

- **Analyzer**: Discovers patterns and generates YAML rules
- **Retrofit**: Applies rules to historical data
- **Indexer**: Uses rules to tag new transactions in real-time

## Database Requirements

The analyzer requires:
- PostgreSQL database with indexed transaction data
- Schema version 12+ (with `tag_id` and `tag_providers` table)
- Transactions with payload data stored
- Recommended index: `CREATE INDEX ON transactions (payload, tag_id, block_time) WHERE payload IS NOT NULL`

## Performance Considerations

- **Query Performance**: Depends on database size and indexing
  - Expected: 10K-100K transactions/second
  - Bottleneck: Usually disk I/O for payload reads

- **Memory Usage**: O(unique_patterns)
  - Each pattern stores only prefixes and limited samples
  - Typical memory: 10-100MB for millions of transactions

- **Processing Time**:
  - 100K transactions: ~10-30 seconds
  - 1M transactions: ~2-5 minutes
  - 10M+ transactions: Consider using `--limit` for sampling

## Troubleshooting

### No Patterns Found

If analyzer reports zero patterns:
- Check that transactions have non-NULL `payload` column
- Lower `--min-count` threshold
- Verify `--untagged-only` setting matches your intent
- Check database connection and query permissions

### Memory Issues

If analyzer consumes excessive memory:
- Use `--limit` to process fewer transactions
- Increase `--min-count` to filter out rare patterns
- Process in batches by date range (future enhancement)

### Slow Performance

To improve analysis speed:
- Add database index: `CREATE INDEX ON transactions (tag_id, block_time) WHERE payload IS NOT NULL`
- Use `--limit` to sample recent transactions instead of full history
- Run on a dedicated database replica to avoid impacting production

## Examples

### Find Mining Pool Patterns

```bash
# Look for pool signatures in payloads
cargo run --release -p kaspa-payload-analyzer -- \
  --database-url $DB_URL \
  --text-prefix-length 50 \
  --min-count 50 \
  --output mining_pools.md
```

### Discover Token Protocols

```bash
# Focus on short text prefixes common in token protocols
cargo run --release -p kaspa-payload-analyzer -- \
  --database-url $DB_URL \
  --text-prefix-length 15 \
  --min-count 200 \
  --generate-rules \
  --output token_protocols.yaml
```

### Identify Binary Protocols

```bash
# Look at hex prefixes for binary protocol detection
cargo run --release -p kaspa-payload-analyzer -- \
  --database-url $DB_URL \
  --hex-prefix-length 16 \
  --min-count 100 \
  --output binary_protocols.md
```

## Related Tools

- **kaspa-payload-retrofit**: Apply discovered filter rules to historical data
- **simply-kaspa-indexer**: Main indexer with real-time filter application

## Support

For issues or questions:
- Check the main project README
- Review existing filter configurations in `examples/` directory
- Examine the retrofit tool for applying discovered patterns
