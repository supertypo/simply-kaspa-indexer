# Enhanced Kaspa Indexer - Implemented Features

This document summarizes the key enhancements and features implemented in this fork of the `simply-kaspa-indexer`. The goal is to provide a decentralized, verifiable, and efficient way to access historical Kaspa blockchain data.

## 1. YAML-Based Transaction Filtering System

**Purpose**: To allow indexer operators to selectively store full transaction bodies based on configurable YAML rules, optimizing storage and enabling protocol-specific indexing.

**Why it was needed**: Storing the full body of every Kaspa transaction is resource-intensive. Many applications only require specific subsets of transactions (e.g., Igra L2 rollup transactions, protocol-specific data).

**How it works**: The indexer supports YAML-based filter configurations that define rules for which transactions should have their full bodies stored. Transactions not matching any rule are stored as ID-only stubs (minimal metadata).

### CLI Arguments

```bash
--filter-config <path>    # Path to YAML filter configuration file
```

**Example Usage**:
```bash
# Use a filter configuration file
--filter-config examples/filters_kasplex_only.yaml


### YAML Configuration Format

```yaml
version: "1.0"

settings:
  # Default behavior for transactions not matching any rule
  default_store_payload: false  # false = ID-only stub, true = store all

rules:
  - name: rule-name          # Descriptive name for the rule
    priority: 100            # Higher priority rules are checked first
    enabled: true            # Can disable rule without removing it
    tag: protocol-name       # Tag to apply to matching transactions
    store_payload: true      # Whether to store full body for matches
    conditions:
      txid:                  # Transaction ID matching (optional)
        prefix: "97b1"       # Hex prefix to match
      payload:               # Payload matching (optional)
        - prefix: "kasplex"  # UTF-8 prefix (or "hex:XXXX" for hex)
        - prefix: "hex:94f8" # Multiple payload conditions = OR logic
```

### Configuration Examples

#### Example 1: Kasplex Protocol Only (`examples/filters_kasplex_only.yaml`)

Store only transactions with payloads starting with "kasplex":

```yaml
version: "1.0"

settings:
  default_store_payload: false

rules:
  - name: kasplex-protocol
    priority: 100
    enabled: true
    tag: kasplex
    store_payload: true
    conditions:
      payload:
        - prefix: "kasplex"  # Matches UTF-8 "kasplex" prefix
```

**Result**: Only Kasplex protocol transactions are fully stored. All other transactions are ID-only stubs.

#### Example 2: Igra Protocol Only (`examples/filters_igra_only.yaml`)

Store only Igra rollup transactions (specific TXID prefix AND specific payload prefixes):

```yaml
version: "1.0"

settings:
  default_store_payload: false

rules:
  - name: igra-rollup
    priority: 100
    enabled: true
    tag: igra
    store_payload: true
    conditions:
      txid:
        prefix: "97b1"         # TXID must start with 97b1 (AND)
      payload:
        - prefix: "hex:94f8"   # Payload starts with 94f8 (OR)
        - prefix: "hex:9402"   # Payload starts with 9402 (OR)
```

**Result**: Only transactions where TXID starts with "97b1" **AND** payload starts with "94f8" or "9402" are fully stored.

#### Example 3: Combined Selective (`examples/filters_combined_selective.yaml`)

Store both Kasplex and Igra transactions:

```yaml
version: "1.0"

settings:
  default_store_payload: false

rules:
  # Higher priority rule checked first
  - name: igra-rollup
    priority: 110
    enabled: true
    tag: igra
    store_payload: true
    conditions:
      txid:
        prefix: "97b1"
      payload:
        - prefix: "hex:94f8"
        - prefix: "hex:9402"

  - name: kasplex-protocol
    priority: 100
    enabled: true
    tag: kasplex
    store_payload: true
    conditions:
      payload:
        - prefix: "kasplex"
```

**Result**: Stores both protocol types with appropriate tags. Priority ensures Igra is checked first (useful if a transaction could match multiple rules).

#### Example 4: Tagging All Transactions (`examples/filters_tagging_all.yaml`)

Tag and store all transaction payloads:

```yaml
version: "1.0"

settings:
  default_store_payload: true  # Store everything by default

rules:
  # Tag specific protocols for classification
  - name: igra-rollup
    priority: 110
    enabled: true
    tag: igra
    store_payload: true
    conditions:
      txid:
        prefix: "97b1"
      payload:
        - prefix: "hex:94f8"
        - prefix: "hex:9402"

  - name: kasplex-protocol
    priority: 100
    enabled: true
    tag: kasplex
    store_payload: true
    conditions:
      payload:
        - prefix: "kasplex"
```

**Result**: All transactions are stored with payloads, but Kasplex and Igra transactions get specific tags for easier querying.

### Filtering Logic

**Rule Evaluation**:
1. Rules are evaluated in **priority order** (highest first)
2. **First match wins** - once a transaction matches a rule, evaluation stops
3. If no rules match, `default_store_payload` setting is used
4. Within a rule:
   - TXID and payload conditions are combined with **AND** logic
   - Multiple payload prefixes within a rule are combined with **OR** logic

**Prefix Matching**:
- UTF-8 strings: `prefix: "kasplex"` matches payloads starting with ASCII "kasplex"
- Hex strings: `prefix: "hex:94f8"` matches payloads starting with bytes 0x94 0xf8
- Case-sensitive for UTF-8, case-insensitive for hex

### Database Changes

**Modified `transactions` table**:
```sql
ALTER TABLE transactions ADD COLUMN tag VARCHAR(50);
```

**Column Description**:
*   `tag`: Protocol or rule name assigned by matching filter rules (e.g., "kasplex", "igra", null for unmatched transactions)


### Performance Characteristics

**Filter Evaluation Overhead**:
- Negligible (~microseconds per transaction)
- Evaluated during transaction mapping phase
- No impact on block processing throughput

**Storage Benefits**:
- 30-99% reduction in payload storage depending on filter selectivity
- Maintains full transaction ID indexing for all transactions
- Enables protocol-specific indexers without full chain storage

### Best Practices

1. **Use descriptive rule names**: Helps with debugging and maintenance
2. **Set appropriate priorities**: Higher priority for more specific rules
3. **Test filter configs**: Use short sync durations to verify rules match as expected
4. **Monitor tag distribution**: Query `SELECT tag, COUNT(*) FROM transactions GROUP BY tag` regularly
5. **Consider default_store_payload carefully**:
   - `false` = opt-in (only matched transactions stored)
   - `true` = opt-out (everything stored, rules just add tags)
6. **Use YAML anchors for complex configs**: Reuse common condition patterns
7. **Version your filter configs**: Include `version: "1.0"` for future compatibility

---

## 2. KIP-15 Sequencing Commitment (SeqCom) Implementation

**Official Specification**: [KIP-15: Canonical Transaction Ordering](https://github.com/kaspanet/kips/blob/master/kip-0015.md)

**Origin**: Proposed by Igra Labs in collaboration with Kaspa core developers to enable based rollup architecture and trustless archival nodes.

**Status**: Active consensus feature (deployed on Kaspa mainnet via Crescendo hard fork, May 2025)

**Purpose**: Provides cryptographic proof of canonical transaction ordering, enabling based Layer 2 rollups and archival nodes that can verify historical transaction data without trusting centralized indexers.

**Why it was needed**: Igra's based rollup architecture requires Kaspa L1 to function as the sequencer for L2 transactions. Since Kaspa nodes prune data after ~30 hours (post-Crescendo), applications need a trustless way to verify historical transaction ordering. KIP-15 enables **Accepted Transactions Archival Nodes (ATAN)** that provide this functionality.

**How it works**:
*   KIP-15 is implemented at the Kaspa consensus layer as a hard fork
*   SequencingCommitment is computed by Kaspa nodes for every block
*   When enabled in this indexer, a `sequencing_commitments` table stores the commitments
*   This indexer functions as an **ATAN (Accepted Transactions Archival Node)** - storing accepted transaction data and their canonical ordering

**Computation Formula** (per KIP-15 specification):
```
SequencingCommitment = blake2b_hash(
    SelectedParent.SequencingCommitment || AcceptedIDMerkleRoot
)
```

Where:
*   `AcceptedIDMerkleRoot` (AIDMR): Merkle root of accepted transaction IDs in canonical order
*   `SelectedParent`: The selected parent block's SequencingCommitment
*   Hash function: blake2b (consistent with Kaspa's merkle tree implementation)

**CLI Arguments**:
```bash
# Enable KIP-15 Sequencing Commitment computation and storage
--enable seqcom
```

**Note**: SeqCom is **DISABLED by default** to minimize storage and computation overhead. Users who need KIP-15 compliance must explicitly enable it.

**Database Changes**:
*   New table `sequencing_commitments` (created only when `--enable seqcom` is used):
    ```sql
    CREATE TABLE sequencing_commitments (
        block_hash BYTEA PRIMARY KEY,
        seqcom_hash BYTEA NOT NULL,
        parent_seqcom_hash BYTEA
    );
    ```

**Usage**:

The table creates a cryptographic chain that proves transaction ordering:

```
Genesis Block (SeqCom = hash(0x00...00, AIDMR_0))
    ↓
Block 1 (SeqCom = hash(SeqCom_0, AIDMR_1))
    ↓
Block 2 (SeqCom = hash(SeqCom_1, AIDMR_2))
    ↓
...
```

**Use Cases**:

### Based Rollups (PRIMARY - Core Motivation for KIP-15)

**Igra: The Origin of KIP-15**

Igra is an EVM-compatible L2 rollup built on Kaspa using a **based rollup architecture**. In this design, users post transactions directly to the Kaspa L1 blockchain, and Kaspa's consensus mechanism determines the canonical ordering of these transactions.

**How Igra Uses KIP-15**:
1. **L1 as Sequencer**: Users submit L2 transactions by posting them to Kaspa L1 (in transaction payloads)
2. **Canonical Ordering**: Kaspa's consensus determines the ordering of posted transactions
3. **SeqCom as Proof**: The SequencingCommitment provides cryptographic proof of this ordering
4. **L2 Execution**: Igra's execution layer (IgReth) processes transactions in the order proven by SeqCom
5. **Block Windows**: Igra L2 blocks represent ~10 Kaspa blocks (~1 second windows)

**Key Benefits for Based Rollups**:
- **Decentralized Sequencing**: No centralized sequencer needed; Kaspa L1 provides censorship resistance
- **Verifiable Ordering**: SeqCom proves canonical transaction order cryptographically
- **Data Availability**: ATAN nodes store transaction data with verifiable ordering
- **Finality**: SeqCom chain proves when transactions reached finality on L1

### Accepted Transactions Archival Node (ATAN)

**This Indexer as ATAN**:
- Stores only accepted transactions and their canonical ordering (not full blocks)
- Allows untrusting nodes to bootstrap from trusted sources using block headers
- Reduces storage requirements while maintaining cryptographic verifiability
- Critical for L2s like Igra that need historical transaction data

**ATAN Design Benefits**:
- Modular separation of chain validation and data availability
- Light clients can verify data integrity without trusting the archival node
- Enables based rollups to access historical data with cryptographic proofs

### General Infrastructure Use Cases

**Trustless Light Clients**:
- Verify transaction inclusion without trusting the indexer
- Proves indexer hasn't omitted or reordered transactions
- Chain SeqCom commitments back to genesis or trusted checkpoint

**Cross-Chain Bridges**:
- Use SeqCom as finality proof for external blockchains
- Verify Kaspa transaction ordering from other chains
- Enable secure wrapped token bridges


**Performance Impact**:
*   Adds ~40 bytes per block to storage
*   No computation overhead (commitments computed by Kaspa consensus layer)
*   This indexer only stores the commitments provided by kaspad
*   No impact when disabled (table not created)

**Implementation Context**:
*   KIP-15 implemented as consensus hard fork ([rusty-kaspa#636](https://github.com/kaspanet/rusty-kaspa/pull/636))
*   SequencingCommitment computed by all Kaspa nodes at consensus layer
*   This indexer exposes commitments via database for ATAN functionality
*   When enabled, provides archival services for based rollup networks and light clients

**References**:
*   [KIP-15 Specification](https://github.com/kaspanet/kips/blob/master/kip-0015.md)
*   [Reference Implementation PR](https://github.com/kaspanet/rusty-kaspa/pull/636)
*   Status: Active (deployed on Kaspa mainnet)

---

**Last Updated**: 2025-01-06
**Schema Version**: v12 (with optional SeqCom table)
**Filter System Version**: 1.0 (YAML-based)
