# kaspa-db-filler-ng
A high performance Kaspa DB Filler implemented in Rust.  

## About
The filler has been implemented from scratch by deriving the functional spec of [kaspa-db-filler](https://github.com/lAmeR1/kaspa-db-filler).  
As part of this process the database schema was reworked to better support concurrency.  
This means that databases populated by the lAmeR1/kaspa-db-filler must be migrated to be compatible.  
A schema migration script has been developed and is available [here](https://github.com/supertypo/kaspa-db-filler-migration).  
A compatible version of the kaspa-rest-server is available [here](https://github.com/kaspa-ng/kaspa-rest-server).

# License
Restricted, not for redistribution or use on other cryptocurrency networks. See LICENSE.

# Getting started

## Run using precompiled Docker image
docker run --rm -it supertypo/kaspa-db-filler-ng:latest -s ws://<kaspad_host>:17110 -d postgres://postgres:postgres@<postgres_host>:5432/postgres

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

### 5. Build workspace
```shell
cargo build
```

### 6. Run filler
```shell
cargo run -- -s ws://<kaspad_host>:17110 -d postgres://postgres:postgres@<postgres_host>:5432
```
