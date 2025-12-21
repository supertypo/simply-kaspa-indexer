# Simply Kaspa Indexer
A high performance Kaspa PostgreSQL indexer implemented in Rust.  

## About
The indexer has been implemented from scratch by deriving the functional spec of [kaspa-db-filler](https://github.com/lAmeR1/kaspa-db-filler).  
As part of this process the database schema was reworked to better support concurrency.  
This means that databases populated by the lAmeR1/kaspa-db-filler must be migrated to be compatible.  
A schema migration script has been developed and is available [here](https://github.com/supertypo/kaspa-db-filler-migration).  
A compatible version of the kaspa-rest-server is available [here](https://github.com/kaspa-ng/kaspa-rest-server).

## Binary releases
For now, binary releases are built for linux/amd64 and linux/arm64.
Docker images are available from https://hub.docker.com/r/supertypo/simply-kaspa-indexer

## Important notes

### Optional tables
To maximize the performance for your specific needs you should take care to disable any table you don't need through command line flags.  
See --help for a list of optional fields.

### Optional fields
In addition to optional tables, many fields can be left empty if they are not required for your use case.  
Use exclude-fields arguments to fine tune. See --help for a list of optional fields.

### Index by script instead of address
If addresses_transactions_table is NOT disabled and exclude-fields contains tx_out_script_public_key_address (and not tx_out_script_public_key),  
the indexer will use scripts_transactions instead of addresses_transactions for indexing addresses for > 25% space savings.

### Postgres tuning
Make sure to tune Postgres to your specific hardware, here is an example for a server with 12GB RAM and SSD storage:
```
shared_buffers = 2GB
work_mem = 128MB
effective_io_concurrency = 32
checkpoint_timeout = 5min
max_wal_size = 4GB
min_wal_size = 80MB
effective_cache_size = 8GB
```
In addition, I highly recommend running Postgres on ZFS with compression=lz4 (or zstd) for space savings as well as for improving performance. Make sure to also set recordsize=16k.

### 10bps note
The indexer is able to keep up with fully satured 10bps (2000+tps) as long as Postgres is running on a sufficiently high-end NVMe.  
By disabling optional tables and fields you can bring the requirements down if running on lesser hardware.

### Historical data
The indexer will begin collecting data from the point in time when it's started.  
If you have an archival node, you can specify the start-block using the --ignore_checkpoint argument and specify an older start block.  
Please make contact with us on the [Kaspa Discord](https://kaspa.org) if you need a pg_dump-file of historical records.

### Pruning
Pruning can be activated by supplying --prune-db, optionally a cron expression can be supplied, e.g. --prune-db=\"30 6 * * *\" (06:30Z every day).  
Additionally retention time must be supplied. E.g. --retention=14d, or by the more fine masked --retention-<...> arguments.

# License
MIT, which means this software can be freely modified to any specific need and redistributed (under certain terms).  
Please be so kind as to contribute back features you think could be beneficial to the general community.

### Contribute to development
kaspa:qrjtsnnpjyvlmkffdqyayrny3qyen9yjkpuw7xvhsz36n69wmrfdyf3nwv67t

# Getting started

## Run using precompiled Docker image
Please consult the [Docker Hub page](https://hub.docker.com/r/supertypo/simply-kaspa-indexer).

## Build and run from source
These instructions are for Ubuntu 24.04, adjustments might be needed for other distributions (or versions). 

### 1. Install dependencies
```shell
sudo apt update && sudo apt install -y git curl build-essential pkg-config libssl-dev
```

### 2. Install Rust
```shell
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
```

### 3. Update path
```shell
source ~/.bashrc
```

### 4. Clone this repository
```shell
git clone <repository-url>
```

### 5. Optionally, switch to a release version
```shell
git checkout <version>
```
E.g. git checkout v1.0.1

### 6. Build workspace
```shell
cargo build
```

### 7. Run indexer
```shell
cargo run -- -s ws://<kaspad_host>:17110 -d postgres://postgres:postgres@<postgres_host>:5432
```

## API
There is a simple api available at http://localhost:8500/api (by default), it currently provides the following endpoints:
- health
- metrics

## Configuration examples

### Minimal configuration
The default is well suited for a block explorers and the like, which is overkill for most other use cases.   
This is the recommended starting point for exchanges and other 'light' integrators:
```
disable:
    block_parent_table,
    blocks_transactions_table,
    addresses_transactions_table

exclude-fields:
    block_accepted_id_merkle_root,
    block_merge_set_blues_hashes,
    block_merge_set_reds_hashes,
    block_selected_parent_hash,
    block_bits,
    block_blue_work,
    block_daa_score,
    block_hash_merkle_root,
    block_nonce,
    block_pruning_point,
    block_utxo_commitment,
    block_version,
    tx_hash,
    tx_mass,
    tx_payload,
    tx_in_signature_script,
    tx_in_sig_op_count,
    tx_in_block_time,
    tx_out_script_public_key_address,
    tx_out_block_time
```
Example command arguments:
```
-u -s ws://your-kaspad:17110 -d postgres://postgres:postgres@your-db:5432 -l 0.0.0.0:8500 \
--prune-db --retention=7d \
--enable=transactions_inputs_resolve \
--disable=block_parent_table,blocks_transactions_table,addresses_transactions_table \
--exclude-fields=block_accepted_id_merkle_root,block_merge_set_blues_hashes,block_merge_set_reds_hashes,block_selected_parent_hash,block_bits,block_blue_work,block_daa_score,block_hash_merkle_root,block_nonce,block_pruning_point,block_utxo_commitment,block_version,tx_hash,tx_mass,tx_payload,tx_in_signature_script,tx_in_sig_op_count,tx_in_block_time,tx_out_script_public_key_address,tx_out_block_time
```

### Enable transactions_inputs_resolve
Having the indexer resolve inputs at index time allows avoiding the expensive join at query time. In essence this load is moved to the indexer, 
except if you also choose to use it to resolve addresses by dropping the addresses_transactions table and querying inputs and outputs directly,
in this case the added effort is zero or even negative. To enable add the argument: --enable=transactions_inputs_resolve.  
  
If you want to have the indexer pre-resolve inputs but already have existing data in the db, you have to manually pre-resolve existing inputs first. 
Make sure the indexer is stopped and apply the following SQL:
```sql
CREATE TABLE transactions_inputs_resolved AS
SELECT
    i.transaction_id,
    i.index,
    i.previous_outpoint_hash,
    i.previous_outpoint_index,
    i.signature_script,
    i.sig_op_count,
    i.block_time,
    o.script_public_key AS previous_outpoint_script,
    o.amount AS previous_outpoint_amount
FROM transactions_inputs i
LEFT JOIN transactions_outputs o
    ON i.previous_outpoint_hash = o.transaction_id
    AND i.previous_outpoint_index = o.index;

DROP TABLE transactions_inputs;
ALTER TABLE transactions_inputs_resolved RENAME TO transactions_inputs;
ALTER TABLE transactions_inputs ADD PRIMARY KEY (transaction_id, index);
ANALYZE transactions_inputs;
```
If you are using kaspa-rest-server you can apply the PREV_OUT_RESOLVED=true env var afterward to disable the expensive join for resolve_previous_outpoints=light queries.

### Address to transaction mapping without separate table
When --enable=transactions_inputs_resolve is specified (see above), you can look up transactions without a separate mapping table.  
  
First make sure the filler is running without exclude on tx_in_block_time and tx_out_block_time.  
If the db already contains inputs and/or outputs without block_time you will have to populate the column manually:
```sql
CREATE TABLE transactions_inputs_with_block_time AS
SELECT
    i.transaction_id,
    i.index,
    i.previous_outpoint_hash,
    i.previous_outpoint_index,
    i.signature_script,
    i.sig_op_count,
    t.block_time,
    i.previous_outpoint_script,
    i.previous_outpoint_amount
FROM transactions_inputs i
JOIN transactions t ON i.transaction_id = t.transaction_id;

DROP TABLE transactions_inputs;
ALTER TABLE transactions_inputs_with_block_time RENAME TO transactions_inputs;
ALTER TABLE transactions_inputs ADD PRIMARY KEY (transaction_id, index);
ANALYZE transactions_inputs;
```
Then use the same method to enrich transactions_outputs with block_time from transactions.  

Lastly the appropriate indexes for efficient querying must be created:
```sql
CREATE INDEX ON transactions_inputs (previous_outpoint_script, block_time DESC);
CREATE INDEX ON transactions_outputs (script_public_key, block_time DESC);
```
Afterward truncate the addresses_transactions/scripts_transactions table, apply --disable=addresses_transactions_table to the indexer and start it.

## Help
```
Usage: simply-kaspa-indexer [OPTIONS]

Options:
  -s, --rpc-url <RPC_URL>
          RPC url to a kaspad instance, e.g 'ws://localhost:17110'. Leave empty to use the Kaspa PNN

  -p, --p2p-url <P2P_URL>
          P2P socket address to a kaspad instance, e.g 'localhost:16111'.

  -n, --network <NETWORK>
          The network type and suffix, e.g. 'testnet-11'
          
          [default: mainnet]

  -d, --database-url <DATABASE_URL>
          PostgreSQL url
          
          [default: postgres://postgres:postgres@localhost:5432/postgres]

  -l, --listen <LISTEN>
          Web server socket address
          
          [default: localhost:8500]

      --base-path <BASE_PATH>
          Web server base path
          
          [default: /]

      --log-level <LOG_LEVEL>
          error, warn, info, debug, trace, off
          
          [default: info]

      --log-no-color
          Disable colored output

  -b, --batch-scale <BATCH_SCALE>
          Batch size factor [0.1-10]. Adjusts internal queues and database batch sizes
          
          [default: 1.0]

  -t, --cache-ttl <CACHE_TTL>
          Cache ttl (secs). Adjusts tx/block caches for in-memory de-duplication
          
          [default: 60]

      --vcp-window <VCP_WINDOW>
          Window size for automatic vcp tip distance adjustment (in seconds)
          
          [default: 600]

      --vcp-interval <VCP_INTERVAL>
          Poll interval for vcp (in seconds)
          
          [default: 4]

  -i, --ignore-checkpoint <IGNORE_CHECKPOINT>
          Ignore checkpoint and start from a specified block, 'p' for pruning point or 'v' for virtual

  -u, --upgrade-db
          Auto-upgrades older db schemas. Use with care

  -c, --initialize-db
          (Re-)initializes the database schema. Use with care

      --prune-db [<PRUNE_DB>]
          Enables db pruning. Optional cron expression. Default: '0 4 * * *' = daily 04:00 (UTC)

      --retention <RETENTION>
          Global data retention for db pruning. Ex: 60d, 24h, etc

      --retention-block-parent <RETENTION_BLOCK_PARENT>
          Retention for block_parent table

      --retention-blocks-transactions <RETENTION_BLOCKS_TRANSACTIONS>
          Retention for blocks_transactions table

      --retention-blocks <RETENTION_BLOCKS>
          Retention for blocks table

      --retention-transactions <RETENTION_TRANSACTIONS>
          Retention for transactions_* tables

      --retention-addresses-transactions <RETENTION_ADDRESSES_TRANSACTIONS>
          Retention for addresses_transactions, scripts_transactions tables

      --enable <ENABLE>
          Enable optional functionality

          Possible values:
          - none
          - dynamic_vcp_tip_distance:    Enables dynamic VCP tip distance, reduces write load due to reorgs
          - transactions_inputs_resolve: Enables resolving transactions_inputs previous_outpoint
          - force_utxo_import:           Forces (pruning point) utxo set import on startup (otherwise only on empty db)

      --disable <DISABLE>
          Disable specific functionality

          Possible values:
          - none
          - virtual_chain_processing:     Disables the virtual chain processor / the transactions_acceptances table
          - transaction_acceptance:       Disables transaction acceptance, marks chain blocks as long as VCP is not disabled
          - transaction_processing:       Disables transaction processing / all transaction related tables
          - blocks_table:                 Disables the blocks table
          - block_parent_table:           Disables the block_parent table
          - blocks_transactions_table:    Disables the blocks_transactions table
          - transactions_table:           Disables the transactions table
          - transactions_inputs_table:    Disables the transactions_inputs table
          - transactions_outputs_table:   Disables the transactions_outputs table
          - addresses_transactions_table: Disables the addresses_transactions (or scripts_transactions) table
          - initial_utxo_import:          Disables initial utxo set import
          - vcp_wait_for_sync:            Start VCP as soon as the filler has passed the previous run. Use with care

      --exclude-fields <EXCLUDE_FIELDS>
          Exclude specific fields. If include_fields is specified this argument is ignored.

          Possible values:
          - none
          - block_accepted_id_merkle_root
          - block_merge_set_blues_hashes
          - block_merge_set_reds_hashes
          - block_selected_parent_hash
          - block_bits
          - block_blue_work
          - block_blue_score:                 Used for sorting blocks
          - block_daa_score
          - block_hash_merkle_root
          - block_nonce
          - block_pruning_point
          - block_timestamp
          - block_utxo_commitment
          - block_version
          - tx_subnetwork_id:                 Used for identifying tx type (coinbase/regular)
          - tx_hash
          - tx_mass
          - tx_payload
          - tx_block_time:                    Used for sorting transactions
          - tx_in_previous_outpoint:          Used for identifying wallet address of sender
          - tx_in_signature_script
          - tx_in_sig_op_count
          - tx_in_block_time:                 Excluding this will increase load for populating adress-/scripts_transactions
          - tx_out_amount
          - tx_out_script_public_key:         Excluding both this and script_public_key_address will disable adress-/scripts_transactions
          - tx_out_script_public_key_address: Excluding this, scripts_transactions to be populated instead of adresses_transactions
          - tx_out_block_time
```
