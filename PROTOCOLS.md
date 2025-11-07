# Kaspa L1 Protocols

This document provides a comprehensive reference for all known protocols built on Kaspa's Layer 1 blockchain that use transaction payloads for protocol identification and data storage.

## Table of Contents

- [Mining Pools](#mining-pools)
- [L2 & Rollups](#l2--rollups)
- [Tokens & NFTs](#tokens--nfts)
- [Social Networks](#social-networks)
- [Messaging](#messaging)
- [File Storage](#file-storage)
- [Prefix Naming Patterns](#prefix-naming-patterns)

---

## Mining Pools

Mining pools use coinbase transaction payloads to identify their block rewards. These transactions contain binary timestamp data followed by the pool signature.

### F2Pool
**Website**: https://www.f2pool.com/
**Category**: Mining
**Identification**: Payload contains `/f2pool.com/`
**Volume**: ~21,347 blocks (mainnet analysis)

### ViaBTC
**Website**: https://www.viabtc.com/
**Category**: Mining
**Identification**: Payload contains `/viabtc.com/`
**Volume**: ~21,219 blocks

### HumPool
**Website**: https://www.humpool.com/
**Category**: Mining
**Identification**: Payload contains `/www.humpool.com/`
**Volume**: ~12,980 blocks

### HeroMiners
**Website**: https://herominers.com/
**Category**: Mining
**Identification**: Payload contains `/herominers.com/`
**Volume**: ~9,568 blocks

### Kryptex
**Website**: https://www.kryptex.com/
**Category**: Mining
**Identification**: Payload contains `/kryptex.com/`
**Volume**: ~5,177 blocks

### kaspa-pool.org
**Website**: https://kaspa-pool.org/
**Category**: Mining
**Identification**: Payload contains `/kaspa-pool.org/`
**Volume**: ~3,291 blocks

### Kasrate
**Website**: https://kasrate.io/
**Category**: Mining
**Identification**: Payload contains `/kasrate.io/`
**Volume**: ~1,914 blocks

### K1Pool
**Website**: https://k1pool.com/
**Category**: Mining
**Identification**: Payload contains `/k1pool.com/`
**Volume**: ~1,247 blocks

### WoolyPooly
**Website**: https://woolypooly.com/
**Category**: Mining
**Identification**: Payload contains `/woolypooly.com/`
**Volume**: ~1,145 blocks

### 2Miners
**Website**: https://2miners.com/
**Category**: Mining
**Identification**: Payload contains `/2miners.com/`
**Volume**: ~1,043 blocks

### IceRiver
**Website**: https://iceriver.io/
**Category**: Mining
**Identification**: Payload contains `/iceriver.io/`
**Volume**: ~337 blocks

### Miningcore
**Repository**: https://github.com/miningcore/miningcore
**Category**: Mining
**Identification**: Payload contains `/miningcore/`
**Volume**: ~14 blocks
**Note**: Generic mining pool software

### bpool
**Website**: https://bpool.io/
**Category**: Mining
**Identification**: Payload contains `/bpool.io/`
**Volume**: ~2 blocks

---

## L2 & Rollups

### Igra Rollup
**Website**: https://igralabs.com/
**Category**: L2 Gaming
**Description**: Gaming rollup protocol for Kaspa

**Identification**:
- TXID prefix: `97b1` (4 hex chars)
- Payload prefix: `hex:94` (1 byte, RLP encoding marker)

**Use Case**: Gaming state commitments and rollup transactions

---

## Tokens & NFTs

### Kasplex
**Website**: https://kasplex.org/
**Category**: Tokens
**Description**: Token and NFT protocol for Kaspa

**Identification**:
- Payload prefix: `kasplex` (UTF-8)

**Use Case**: Fungible tokens (KRC-20) and NFTs

---

## Social Networks

### K Social Network
**Repository**: https://github.com/thesheepcat/K
**Category**: Social
**Description**: Decentralized social network on Kaspa

**Identification**:
- Payload prefix: `k:1` (UTF-8)

**Use Case**: Social media posts, profiles, and interactions

---

## Messaging

### Kaspatalk
**Repository**: https://github.com/thesheepcat/kaspatalk-backend
**Category**: Messaging
**Description**: Messaging protocol with chat and talk modes

**Identification**:
- Chat mode: Payload prefix `kch:` (UTF-8)
- Talk mode: Payload prefix `ktk:` (UTF-8)

**Use Case**: On-chain messaging and communication

### Kasia
**Repository**: https://github.com/K-Kluster/Kasia
**Category**: Messaging
**Description**: Encrypted messaging protocol

**Identification**:
- Payload prefix: `ciph_msg` (UTF-8)

**Use Case**: End-to-end encrypted on-chain messages

---

## File Storage

### Kaspa File
**Repository**: https://github.com/RossKU/kaspa-file-storage-v2
**Category**: Storage
**Description**: Decentralized file storage protocol

**Identification**:
- File marker: Payload prefix `kaspa-file` (UTF-8)
- Directory marker: Payload prefix `kaspa-directory` (UTF-8)

**Use Case**: On-chain file metadata and directory structures

---

## Prefix Naming Patterns

Most Kaspa user-facing protocols follow a naming convention derived from "kaspa" or starting with "k":

- **k** prefix: K Social Network (`k:1`)
- **kas** prefix: Kaspa File (`kaspa-file`, `kaspa-directory`)
- **kasp** prefix: Kasplex (`kasplex`), Kaspatalk (`kch:`, `ktk:`)
- **kasia** prefix: Kasia (`ciph_msg` - exception to the naming pattern)

Mining pools use their domain names as signatures in coinbase transactions, typically in the format `/<domain>/`.

This shared prefix structure makes prefix-based identification and filtering particularly efficient for Kaspa protocols.
