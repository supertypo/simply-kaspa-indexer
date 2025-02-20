# Simply Kaspa Indexer
A high performance Kaspa PostgreSQL indexer implemented in Rust.  

## About
The indexer has been implemented from scratch by deriving the functional spec of [kaspa-db-filler](https://github.com/lAmeR1/kaspa-db-filler).  
As part of this process the database schema was reworked to better support concurrency.  
This means that databases populated by the lAmeR1/kaspa-db-filler must be migrated to be compatible.  
A schema migration script has been developed and is available [here](https://github.com/supertypo/kaspa-db-filler-migration).  
A compatible version of the kaspa-rest-server is available [here](https://github.com/kaspa-ng/kaspa-rest-server).

## Important notes

### Optional tables
To maximize the performance for your specific needs you should take care to disable any table you don't need through command line flags.  
See --help for a list of optional fields.

### Optional fields
In addition to optional tables, many fields can be left empty if they are not required for your use case.  
Use exclude-fields arguments to fine tune. See --help for a list of optional fields.

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

### Tn11 (10bps) note
The indexer is able to keep up with the 10bps testnet (TN11) under full load (2000+tps) as long as Postgres is running on a sufficiently high-end NVMe.  
By disabling optional tables and fields you can bring the requirements down if running on lesser hardware.

### Historical data
The indexer will begin collecting data from the point in time when it's started.  
If you have an archival node, you can specify the start-block using the --ignore_checkpoint argument and specify an older start block.  
Please make contact with us on the [Kaspa Discord](https://kaspa.org) if you need a pg_dump-file of historical records.

# License
ISC, which means this software can be freely modified to any specific need and redistributed (under certain terms).  
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

## Help
```
Usage: simply-kaspa-indexer [OPTIONS]

Options:
  -s, --rpc-url <RPC_URL>
          The url to a kaspad instance, e.g 'ws://localhost:17110'. Leave empty to use the Kaspa PNN

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

  -i, --ignore-checkpoint <IGNORE_CHECKPOINT>
          Ignore checkpoint and start from a specified block, 'p' for pruning point or 'v' for virtual

  -u, --upgrade-db
          Auto-upgrades older db schemas. Use with care

  -c, --initialize-db
          (Re-)initializes the database schema. Use with care

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
          - addresses_transactions_table: Disables the addresses_transactions table
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
          - block_blue_score:                 NB! Used for sorting blocks
          - block_daa_score
          - block_hash_merkle_root
          - block_nonce
          - block_pruning_point
          - block_timestamp
          - block_utxo_commitment
          - block_version
          - tx_subnetwork_id:                 NB! Used for identifying tx type (coinbase/regular)
          - tx_hash
          - tx_mass
          - tx_payload
          - tx_block_time:                    NB! Used for sorting transactions
          - tx_in_previous_outpoint:          NB! Used for identifying wallet address of sender
          - tx_in_signature_script
          - tx_in_sig_op_count
          - tx_in_block_time
          - tx_out_amount
          - tx_out_script_public_key:         NB! Used for identifying wallet addresses
          - tx_out_script_public_key_address: NB! Used for identifying wallet addresses
          - tx_out_block_time
```
