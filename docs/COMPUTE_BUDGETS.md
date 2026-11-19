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
3. **Accuracy Is a Measured Property**: `estimate_compute` does not carry an unconditional accuracy guarantee. Any accuracy claim is scoped to the recorded results of the calibration procedure below, and clients should reproduce that procedure against their own SDK version and fixtures before relying on a specific error bound.

---

## 6. Accuracy Measurement Procedure

The accuracy of `estimate_compute` is established by measurement, not assertion. To assess confidence in a fee quote, run the following procedure and record every field; the resulting table is the only basis for an accuracy claim.

### 6.1 Recorded Fields

For each supported benchmark, record:

1. **SDK version**: the exact `soroban-sdk` version (and Rust toolchain version) used to build and run the benchmark.
2. **Fixture inputs**: the `operation` symbol and the full `params` map passed to `estimate_compute`, including recipient/payer counts and any configuration flags.
3. **Observed host costs**: the actual `cpu_insns` and `mem_bytes` consumed by the corresponding real contract operation, as reported by the host execution budget during the benchmark.
4. **Estimate error**: the estimated values returned by `estimate_compute` for the same fixture, and the error for each dimension:
   - `cpu_error_pct = |estimated_cpu_insns - observed_cpu_insns| / observed_cpu_insns * 100`
   - `mem_error_pct = |estimated_mem_bytes - observed_mem_bytes| / observed_mem_bytes * 100`
   - `fee_error_pct = |estimated_fee_stroops - observed_fee_stroops| / observed_fee_stroops * 100`

### 6.2 Benchmark Matrix

Run at least one fixture per supported operation, and cover the range boundaries used in Section 3:

| Operation | Fixture inputs to record |
|---|---|
| `create_invoice` | 1 recipient; 20 recipients; with and without optional config flags |
| `pay` | 1 recipient; 20 recipients; partial and fully-funded invoice (auto-release path) |
| `release` | 1 recipient; 20 recipients |
| `refund` | 1 payer; 10 payers |
| `open_dispute` | single `invoice_id` |
| `approve_release` | single `invoice_id` |

### 6.3 Procedure

1. Build the contract and benchmark harness with the recorded SDK version.
2. For each fixture, execute the real operation and capture the observed host `cpu_insns` and `mem_bytes` from the execution budget.
3. Call `estimate_compute` with the identical `operation` and `params` and capture the returned `ComputeEstimate`.
4. Compute the error percentages from Section 6.1 and record them alongside the SDK version and fixture inputs.
5. Re-run the procedure whenever the SDK version, contract code, or fixture set changes; prior accuracy claims do not carry over.

### 6.4 Scope of Accuracy Claims

- An accuracy claim applies only to the SDK version, fixtures, and operations recorded in the measurement table.
- Results outside the recorded fixture set (different recipient counts, different flags, different SDK versions) are unmeasured and carry no stated error bound.
- Clients should treat the recorded maximum error per operation as the confidence bound for their quotes, and re-measure when upgrading the SDK or changing inputs materially.
