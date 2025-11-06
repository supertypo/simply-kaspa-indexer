# Kaspa L1 Protocols

This document provides a comprehensive reference for all known protocols built on Kaspa's Layer 1 blockchain that use transaction payloads for protocol identification and data storage.

## Protocol Registry

### Igra Rollup
**Repository**: https://igralabs.com/
**Description**: Gaming rollup protocol for Kaspa

**Identification**:
- TXID prefix: `97b1` (4 hex chars)
- Payload prefix: `hex:94` (1 byte, RLP encoding marker)

**Use Case**: Gaming state commitments and rollup transactions

---

### Kasplex
**Repository**: https://kasplex.org/
**Description**: Token and NFT protocol for Kaspa

**Identification**:
- Payload prefix: `kasplex` (UTF-8)

**Use Case**: Fungible tokens (KRC-20) and NFTs

---

### K Social Network
**Repository**: https://github.com/thesheepcat/K
**Description**: Decentralized social network on Kaspa

**Identification**:
- Payload prefix: `k:1` (UTF-8)

**Use Case**: Social media posts, profiles, and interactions

---

### Kaspatalk
**Repository**: https://github.com/thesheepcat/kaspatalk-backend
**Description**: Messaging protocol with chat and talk modes

**Identification**:
- Chat mode: Payload prefix `kch` (UTF-8)
- Talk mode: Payload prefix `ktk` (UTF-8)

**Use Case**: On-chain messaging and communication

---

### Kasia
**Repository**: https://github.com/K-Kluster/Kasia
**Description**: Encrypted messaging protocol

**Identification**:
- Payload prefix: `ciph_msg` (UTF-8)

**Use Case**: End-to-end encrypted on-chain messages

---

### Kaspa File
**Repository**: https://github.com/RossKU/kaspa-file-storage-v2
**Description**: Decentralized file storage protocol

**Identification**:
- File marker: Payload prefix `kaspa-file` (UTF-8)
- Directory marker: Payload prefix `kaspa-directory` (UTF-8)

**Use Case**: On-chain file metadata and directory structures

---

## Prefix Naming Patterns

Most Kaspa protocols follow a naming convention derived from "kaspa" or starting with "k":

- **k** prefix: K Social Network (`k:1`)
- **kas** prefix: Kaspa File (`kaspa-file`, `kaspa-directory`)
- **kasp** prefix: Kasplex (`kasplex`), Kaspatalk (`kch`, `ktk`)
- **kasia** prefix: Kasia (`ciph_msg` - exception to the naming pattern)

This shared prefix structure makes prefix-based identification and filtering particularly efficient for Kaspa protocols.
