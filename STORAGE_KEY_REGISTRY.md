# Storage Key Registry

This document enumerates every storage key used by the SplitVault contract, organized by storage tier and key type. Keys are defined in `contracts/split/src/storage_keys.rs` as `#[contracttype]` enums to prevent collisions and ensure XDR uniqueness.

## Storage Tiers

| Tier | Lifecycle | Keys | Typical TTL |
|------|-----------|------|------------|
| **instance** | Live during contract execution; wiped on upgrade unless explicitly preserved. | Singletons (config, counters, addresses) | Contract lifetime |
| **persistent** | Survives contract upgrades; archived after `ARCHIVE_AFTER_LEDGERS` (~100k ledgers) | Per-entity state | 100k ledgers or longer |
| **temporary** | Short-lived ephemeral state (not yet used) | None | – |

## Instance Storage: `StorageKey` enum

Keys in this tier are contract-level singletons. Each variant carries no data (unit type).

| Variant | Value | Introduced | TTL | Purpose |
|---------|-------|------------|-----|---------|
| `Admin` | unit | v1 | ∞ | Primary contract admin address |
| `Admins` | unit | v1 | ∞ | Map of admin roles → address lists |
| `Paused` | unit | v1 | ∞ | Global pause flag (bool) |
| `PausedFns` | unit | v1 | ∞ | Set of paused function names (Set<Symbol>) |
| `Treasury` | unit | v1 | ∞ | Platform treasury address |
| `UsdcToken` | unit | v1 | ∞ | USDC token contract address |
| `CreationFee` | unit | v1 | ∞ | Invoice creation fee (i128) in stroops |
| `PlatformFeeBps` | unit | v1 | ∞ | Platform fee rate (u32) in basis points |
| `PlatformFeeWaiverList` | unit | v1 | ∞ | Recipient-level platform fee waiver list (Vec<Address>) |
| `CreatorFeeWaiver` | unit | #296 | ∞ | Creator-level fee waiver list (Vec<Address>) |
| `Counter` | unit | v1 | ∞ | Invoice ID counter (u64) |
| `GlobalPayerLimit` | unit | v1 | ∞ | Global payer velocity limit (i128) |
| `GlobalPayerWindow` | unit | v1 | ∞ | Global payer velocity window (u32 ledgers) |
| `StreamContract` | unit | #1 | ∞ | Stellar payment streaming contract address |
| `CreatorWhitelist` | unit | #4 | ∞ | Creator whitelist enabled flag (bool) |
| `Compliance` | unit | v1 | ∞ | Compliance check contract address |
| `KycContract` | unit | v1 | ∞ | KYC verification contract address |
| `RateLimit` | unit | v1 | ∞ | Rate limit max invoices per window (u32) |
| `RateWindow` | unit | v1 | ∞ | Rate limit window duration (u32 seconds) |
| `MaxCancelBps` | unit | v1 | ∞ | Maximum cancellation rate (u32 basis points) |
| `ReceiptFactory` | unit | v1 | ∞ | Receipt token factory contract address |
| `DashboardContract` | unit | v1 | ∞ | Dashboard contract address |
| `NftGate` | unit | #192 | ∞ | NFT gate contract address |
| `TimelockSecs` | unit | #185 | ∞ | Timelock duration (u32 seconds) |
| `TimelockActionCounter` | unit | #185 | ∞ | Timelock action ID counter (u64) |
| `FeeTiers` | unit | #285 | ∞ | Volume-based fee tiers (Vec<FeeTier>) |
| `PendingAdmin` | unit | v1 | ∞ | Pending admin proposal address |
| `GovernanceContract` | unit | v1 | ∞ | External governance contract address |
| `Factories` | unit | #145 | ∞ | Authorized factory addresses (Vec<Address>) |
| `DexContract` | unit | v1 | ∞ | DEX contract address |
| `TotalInvoices` | unit | #28 | ∞ | Total invoices created (u64) |
| `TotalVolume` | unit | #28 | ∞ | Total funded volume (i128) |
| `TotalReleased` | unit | v1 | ∞ | Total released volume (i128) |
| `TotalRefunded` | unit | v1 | ∞ | Total refunded volume (i128) |
| `TreasuryGroupCounter` | unit | v1 | ∞ | Treasury group ID counter (u64) |
| `ContractVersion` | unit | #279 | ∞ | Current contract version (u32) |
| `ArchiveAfterLedgers` | unit | v1 | ∞ | Ledger threshold for archival (u64) |
| `CircuitBreaker` | unit | #297 | ∞ | Circuit breaker active flag (bool) |
| `CircuitBreakerReason` | unit | #297 | ∞ | Circuit breaker activation reason (String) |
| `PlatformVolThresh` | unit | #276 | ∞ | Platform volume milestone threshold (i128) |
| `PlatformVolMile` | unit | #276 | ∞ | Last platform milestone emitted (u32) |
| `CreatorVolThresh` | unit | #276 | ∞ | Creator volume milestone threshold (i128) |
| `UpgradeProposal` | unit | #310 | ∞ | Pending upgrade proposal (UpgradeProposal) |
| `ProtocolFee` | unit | #326 | ∞ | Protocol fee config (ProtocolFeeConfig) |
| `ReentrancyGuard` | unit | v1 | ∞ | Reentrancy guard flag (bool) |

## Factory Contract: `FactoryKey` enum

Keys used by the standalone factory contract (`contracts/factory/src/lib.rs`). The factory administrator is stored under `FactoryKey::Admin` and is written only by `initialize`; the confirmed handover flow (#212) updates it via `propose_admin` / `accept_admin` without touching the approved wasm hash allowlist.

| Variant | Value | Introduced | TTL | Purpose |
|---------|-------|------------|-----|---------|
| `Admin` | unit | v1 | ∞ | Current factory administrator (`factory_admin_key`); switched by `accept_admin` |
| `PendingAdmin` | unit | #212 | ∞ | Proposed successor administrator; set by `propose_admin`, cleared on `accept_admin` |
| `ApprovedWasmHashes` | unit | v1 | ∞ | Allowlist of approved wasm hashes (`approve_wasm_hash`); preserved across admin handover |

## Persistent Storage: `InvoiceKey` enum

Keys keyed by invoice ID or sharded. All carry a u64 invoice ID unless noted.

| Variant | Key Shape | Introduced | Type | Purpose |
|---------|-----------|------------|------|---------|
| `Invoice(id)` | `(Symbol, u64)` | v1 | InvoiceCore | Core invoice fields (recipients, amounts, deadlines) |
| `InvoiceExt(id)` | `(Symbol, u64)` | v1 | InvoiceExt | Extended fields (prerequisites, tranches, etc.) |
| `InvoiceExt2(id)` | `(Symbol, u64)` | v1 | InvoiceExt2 | Secondary extended fields (release stages, etc.) |
| `InvoiceCompact(id)` | `(Symbol, u64)` | #334 | Bytes | Compact XDR-encoded invoice data |
| `InvoiceHot(id)` | `(Symbol, u64)` | v1 | InvoiceHot | Hot (frequently-accessed) fields; also in instance storage |
| `AuditLog(id)` | `(Symbol, u64)` | v1 | Vec<AuditEntry> | Audit trail of invoice lifecycle events |
| `PaymentShard(id, shard)` | `(Symbol, u64, u64)` | v1 | Vec<Payment> | Sharded payment list (8 shards per invoice) |
| `ReleaseDelay(id)` | `(Symbol, u64)` | #327 | u64 | Release delay in ledgers |
| `FundedAtLedger(id)` | `(Symbol, u64)` | #327 | u32 | Ledger sequence when fully funded |
| `MetadataHash(id)` | `(Symbol, u64)` | #329 | BytesN<32> | Off-chain metadata hash (SHA-256) |
| `PaidRecipients(id)` | `(Symbol, u64)` | #330 | Set<Address> | Recipients already paid via release_to_recipient |
| `CompactStatus(id)` | `(Symbol, u64)` | #334 | u8 | Compact status byte (0=Pending, 1=Released, 2=Refunded, 3=Cancelled) |
| `CompactDeadlineLedger(id)` | `(Symbol, u64)` | #334 | u32 | Compact deadline as ledger sequence |
| `ConfidentialCount(id)` | `(Symbol, u64)` | #295 | u64 | Count of confidential payments |
| `InvoiceGroup(id)` | `(Symbol, u64)` | v1 | u64 | Reverse lookup: invoice → group ID |
| `InvoiceTreasury(id)` | `(Symbol, u64)` | v1 | TreasuryRecord | Invoice-level treasury allocation |
| `Delegate(id)` | `(Symbol, u64)` | #43 | Address | Primary delegate address for this invoice |
| `PaymentWindow(id)` | `(Symbol, u64)` | #168 | Vec<u64> | Sliding-window payment timestamps for rate limiting |
| `Cert(id)` | `(Symbol, u64)` | v1 | PaymentCertificate | Payment completion certificate |
| `DisputeRecord(id)` | `(Symbol, u64)` | #325 | DisputeRecord | Active dispute record |
| `DisputeRaisedAt(id)` | `(Symbol, u64)` | #325 | u32 | Ledger at which dispute was raised |
| `Refunded(id)` | `(Symbol, u64)` | #308 | Set<Address> | Set of addresses that received refunds |
| `RecipientsList(id)` | `(Symbol, u64)` | #332 | Vec<Address> | Contiguous list of all recipients |
| `AmountsList(id)` | `(Symbol, u64)` | #332 | Vec<i128> | Parallel amounts for each recipient |
| `PaidFlags(id)` | `(Symbol, u64)` | #332 | u32 | Bit-vector of paid flags (up to 32 recipients per word) |
| `MilestoneFlags(id)` | `(Symbol, u64)` | #333 | u8 | Milestone emission bitmask (Bit0=25%, Bit1=50%, etc.) |
| `ArchiveMarker(id)` | `(Symbol, u64)` | v1 | bool | Set when invoice is moved to instance storage |
| `CreatedLedger(id)` | `(Symbol, u64)` | v1 | u32 | Ledger sequence at invoice creation |
| `SubscriptionParams(id)` | `(Symbol, u64)` | v1 | SubscriptionParams | Subscription parameters |
| `SubscriptionSubscribers(id)` | `(Symbol, u64)` | v1 | Vec<Address> | Subscription subscriber list |
| `ExtVote(id)` | `(Symbol, u64)` | v1 | ExternalVote | External governance vote entry |
| `Group(id)` | `(Symbol, u64)` | v1 | InvoiceGroup | Invoice group definition |
| `GroupTreasury(id)` | `(Symbol, u64)` | v1 | TreasuryRecord | Group-level treasury allocation |
| `TimelockAction(id)` | `(Symbol, u64)` | #185 | TimelockAction | Timelock action entry (keyed by action_id) |

## Persi

/* … truncated 5632 chars — edit only what you need near the top … */
