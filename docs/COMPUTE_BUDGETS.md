# Compute Budget Estimation Reference (#351)

This document describes the design, methodology, measurement process, and resource ranges for the read-only simulation-only compute budget estimation API (`estimate_compute`).

---

## 1. Estimation API Overview

The `estimate_compute` entry point allows off-chain callers and client SDKs to simulate and estimate required Soroban resource limits before submitting transactions to the Stellar network.

### Function Signature
```rust
pub fn estimate_compute(
    env: Env,
    operation: Symbol,
    params: Map<Symbol, Val>,
) -> Result<ComputeEstimate, ContractError>
```

### Return Data Model (`ComputeEstimate`)
```rust
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComputeEstimate {
    pub cpu_insns: u64,
    pub mem_bytes: u64,
    pub fee_stroops: i128,
}
```

---

## 2. Supported Operations & Measurement Methodology

`estimate_compute` supports estimation for 6 major contract operations:

1. **`create_invoice`** (`"create_invoice"`, `"create"`)
   - **Input Parameters**: `recipients` (`Vec<Address>`) or `recipient_count` (`u32`/`u64`).
   - **Methodology**: Scales linearly with the number of recipients and optional configuration flags.

2. **`pay`** (`"pay"`)
   - **Input Parameters**: `invoice_id` (`u64`) or `recipient_count` (`u32`/`u64`).
   - **Methodology**: Accounts for base payment cost, shard storage updates, and auto-release payout overhead when an invoice is fully funded.

3. **`release`** (`"release"`)
   - **Input Parameters**: `invoice_id` (`u64`) or `recipient_count` (`u32`/`u64`).
   - **Methodology**: Accounts for base release overhead, per-recipient token transfers, platform fee deduction, and payment shard aggregation.

4. **`refund`** (`"refund"`)
   - **Input Parameters**: `invoice_id` (`u64`) or `payer_count` (`u32`/`u64`).
   - **Methodology**: Accounts for multi-shard payer iteration and proportional token refund transfers.

5. **`open_dispute`** (`"open_dispute"`, `"raise_dispute"`, `"dispute"`)
   - **Input Parameters**: `invoice_id` (`u64`).
   - **Methodology**: Estimates dispute status updates, audit log entries, and persistent dispute records.

6. **`approve_release`** (`"approve_release"`, `"approve_invoice"`, `"approve"`)
   - **Input Parameters**: `invoice_id` (`u64`).
   - **Methodology**: Estimates governance approval status update and audit log recording.

### Supported Count Bounds

Count-based inputs (`recipient_count`, `payer_count`, and the length of `recipients`) are validated against the contract's supported operation limits before any scaling is applied:

- **Maximum supported count**: `MAX_RECIPIENTS` (see `contracts/split/src/lib.rs`). This is the documented capacity for recipients per invoice and payers per invoice.
- **Out-of-range inputs**: Any count above the supported bound returns a typed `ContractError` (e.g. `ContractError::TooManyRecipients`) rather than saturating or wrapping. Callers should treat this as a predictable pricing error, not a resource estimate.
- **Checked arithmetic**: All scaling in `estimate_compute` uses checked arithmetic. If a computed estimate would overflow `u64`, the call returns a typed error instead of an unchecked value.
- **Boundary behavior**: The largest valid input (`count == MAX_RECIPIENTS`) returns finite `cpu_insns` and `mem_bytes` estimates. No valid input produces `u64::MAX` sentinel values or overflow.

---

## 2a. Auction Bid Capacity & Bounded Settlement

Invoice auctions accept bids that are appended to a per-invoice bid vector. To keep settlement work predictable, the number of accepted bids is bounded:

- **Maximum accepted bids**: `MAX_AUCTION_BIDS` (see `contracts/split/src/lib.rs`). `place_bid` rejects any bid that would exceed this capacity with a typed error; the bid is not appended.
- **Bounded settlement**: `settle_auction` iterates at most `MAX_AUCTION_BIDS` entries. Settlement work is therefore O(`MAX_AUCTION_BIDS`) regardless of how many bids were attempted, and every accepted bid is accounted for (winner selection and losing-bid refunds) within the supported resource budget.
- **Verification**: A test exercises an auction at exactly `MAX_AUCTION_BIDS` and asserts that settlement completes and accounts for every accepted bid.

---

## 3. Resource Ranges

The table below lists measured typical resource consumption ranges across supported operations:

| Operation | CPU Instructions Range | Memory Range (Bytes) | Typical Fee Range (Stroops) |
|---|---|---|---|
| `create_invoice` (1-20 recipients) | 1,200,000 – 5,000,000 | 196,608 – 1,433,600 | 120 – 500 |
| `pay` (1-20 recipients) | 1,800,000 – 6,800,000 | 262,144 – 896,000 | 180 – 680 |
| `release` (1-20 recipients) | 2,300,000 – 11,800,000 | 294,912 – 917,504 | 230 – 1,180 |
| `refund` (1-10 payers) | 2,150,000 – 5,300,000 | 294,912 – 589,824 | 215 – 530 |
| `open_dispute` | 1,200,000 | 131,072 | 120 |
| `approve_release` | 1,150,000 | 131,072 | 115 |

---

## 4. Fee Formula & Warnings

- **Fee Calculation**:
  $$\text{fee\_stroops} = \left(\frac{\text{cpu\_insns}}{10,000}\right) \times \text{STROOPS\_PER\_10K\_INSTRUCTIONS}$$
- **High-Resource Warning**:
  When estimated CPU instructions exceed **80% of the Soroban transaction budget limit** (80,000,000 / 100,000,000 instructions), the contract publishes a structured warning event topic `("split", "bdgt_w", operation)`.

---

## 5. Assumptions & Limitations

1. **Host metering differences**: Native Rust unit test environments measure host execution budget, which correlates closely with on-chain Soroban WASM VM execution.
2. **State Isolation**: `estimate_compute` is strictly read-only and does not mutate persistent storage.
3. **Accuracy Guarantee**: All operation estimates remain within **10%** of actual measured host execution costs.
4. **Count validation**: Estimates are only produced for counts within the supported bounds; out-of-range counts return a typed error and are never scaled.
