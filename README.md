<div align="center">
  <img src="https://raw.githubusercontent.com/splitvault/.github/main/assets/splitvault-mark.svg" alt="SplitVault" width="80" />
  <h1>split-contracts</h1>
</div>

![Rust](https://img.shields.io/badge/Rust-1.93+-orange?logo=rust)
![Soroban SDK](https://img.shields.io/badge/soroban--sdk-28-blueviolet)
![License](https://img.shields.io/badge/license-MIT-green)
![CI](https://github.com/splitvault/split-contracts/actions/workflows/test.yml/badge.svg)

Soroban smart contracts powering **SplitVault** — on-chain invoice & payment splitting on Stellar.

## What It Does

SplitVault lets users create on-chain invoices where multiple payers each owe a share. When all shares are paid, the contract automatically routes USDC to each recipient. If the deadline passes unfunded, contributors are refunded.

**Use cases:** group bills, freelancer team payments, remittances across LATAM and Africa.

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Language | Rust 1.93+ |
| Smart Contract SDK | soroban-sdk 28 |
| Test Runner | `cargo test` |
| Deploy Tool | stellar-cli |
| Network | Stellar Testnet / Mainnet |

## Local Setup

### Prerequisites

- Stable Rust with the `wasm32v1-none` target
- stellar-cli 28.1.0

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install stable Rust and the production WASM target
rustup toolchain install stable --profile minimal --target wasm32v1-none

# Install stellar-cli
cargo install --locked stellar-cli --version 28.1.0
```

### Clone & Build

```bash
git clone https://github.com/splitvault/split-contracts.git
cd split-contracts
stellar contract build --package split
```

The production WASM artifact is written to
`target/wasm32v1-none/release/split.wasm`, matching the deployment workflow.
Soroban SDK 28 contracts must use `wasm32v1-none` and `stellar contract build`.

### Run Tests

```bash
cargo test --locked --workspace
```

### Fuzzing

Run these commands from the repository root with the nightly in
`rust-toolchain.toml`. Keep the standalone `fuzz/Cargo.lock` in sync with
dependency changes. CI uses it for the cache key, checks dependency resolution
with `--locked`, and verifies that fuzzing leaves the file unchanged.

```bash
cargo install --locked --version 0.13.2 cargo-fuzz
cargo check --locked --manifest-path fuzz/Cargo.toml
cargo fuzz run fuzz_create_invoice -- -max_total_time=60
```

### Testnet Deployment

The manually triggered **Deploy to Testnet** workflow uses Stellar CLI
`28.1.0` and logs the installed version. After a successful deployment,
download `testnet-deployment.json` from the run's **testnet-deployment**
artifact. The fields are `network`, `contract_id`, `commit_sha`, and
`wasm_sha256` (the SHA-256 digest of the deployed WASM file).

With `jq` installed, run the offline manifest fixture with
`bash scripts/test-testnet-deployment-manifest.sh`.

## Contract Function Reference

### `create_invoice`

```rust
create_invoice(
    env: Env,
    creator: Address,
    recipients: Vec<Address>,
    amounts: Vec<i128>,
    token: Address,
    deadline: u64,
) -> u64
```

Creates a new invoice. Returns the invoice ID. `amounts[i]` is owed to `recipients[i]`. `deadline` is a Unix timestamp.

### `pay`

```rust
pay(env: Env, payer: Address, invoice_id: u64, amount: i128)
```

Transfers `amount` of the invoice token from `payer` to the contract. Auto-releases if fully funded.

### `release`

```rust
release(env: Env, invoice_id: u64)
```

Routes funds to all recipients. Callable by anyone once fully funded.

### `refund`

```rust
refund(env: Env, invoice_id: u64)
```

Refunds all payers. Callable by anyone after the deadline if not fully funded.

### `get_invoice`

```rust
get_invoice(env: Env, invoice_id: u64) -> Invoice
```

Returns the full invoice struct.

## Testnet Contract Address

```
PLACEHOLDER — update after deployment
```

## Storage Key Registry

Every storage key the contract uses is serialised to XDR and compared against a committed baseline in `tests/snapshots/storage_keys.json`. The snapshot runs as `cargo test -p split storage_snapshot`.

**Policy:**
- Adding, removing, or changing any storage key will intentionally fail the snapshot test.
- If the change is intentional (e.g. a new feature adds a key, or a migration renames an existing one), update the baseline file by running the test locally, copying the generated XDR into `tests/snapshots/storage_keys.json`, and **including a migration note in the PR description**.
- The test also asserts that **no two keys produce the same XDR** (collision-free).
- Each key entry in the snapshot is labelled by its Rust function name (e.g. `invoice_key`, `rep_key`).

To update the baseline:
```bash
cargo test -p split storage_snapshot 2>&1 | grep -A 999 "EXPECTED (generated)" | tail -n +2 | head -n -1 > tests/snapshots/storage_keys.json
```

## Contributing via Drips Wave

This project participates in the [Drips Wave Program](https://drips.network/wave) by the Stellar Development Foundation. Contributors can earn rewards by completing open issues.

See [CONTRIBUTING.md](./CONTRIBUTING.md) for the full contribution guide.

**Do not start coding until assigned to an issue by a maintainer.**

## Handsoff notes

<!-- handsoff-issue-5 -->
- #5: Missing Authorization for Dynamic Payout Address Modification in `rotate_recipient_address`

<!-- handsoff-issue-6 -->
- #6: Pool Pay Cross-Invoice Limit Bypass for Blocked and Velocity-Capped Payers

<!-- handsoff-issue-63 -->
- #63: Incomplete State Clean-up During Emergency Circuit Breaker Rollbacks

<!-- handsoff-issue-64 -->
- #64: Silent Payout Loss in `reveal_confidential_payment` Due to Omission of Shard Storage

<!-- handsoff-issue-255 -->
- #255: Match fee bracket coverage to the advertised payment domain
