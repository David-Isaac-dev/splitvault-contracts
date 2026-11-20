# CI replay and release size reports

## Fuzzing

Each CI fuzz target has its own job so a failure does not skip other targets.
Before invoking libFuzzer, `scripts/run-fuzz.sh` logs the target, commit, Rust
and cargo-fuzz versions, seed, initial corpus location, and full 60-second
command to the log and GitHub job summary. From the repository root at that
commit, install the recorded toolchain and cargo-fuzz version and copy the
command to start the same target locally. You can also set `FUZZ_SEED` to a
nonzero integer up to `2147483647` when running the wrapper:

```bash
FUZZ_SEED=12345 bash scripts/run-fuzz.sh fuzz_sequence
```

A seed starts the same random stream; a time limit, compiler differences,
and changes to the starting corpus can still change which inputs are explored.
Use a saved crash input for direct failure replay.

On failure, download the `fuzz-failure-<target>-<attempt>` artifact and extract
it into `fuzz/artifacts/<target>/`. It contains original crash inputs, any
inputs produced by best-effort minimization, and `REPLAY.md` with the exact
target and replay commands, for example:

```bash
cargo fuzz run fuzz_sequence fuzz/artifacts/fuzz_sequence/crash-<digest> -- -runs=1
```

Minimization is bounded to 30 seconds per crash and never replaces the original
input or masks the failing exit status. OOM and timeout reproducers are also
retained. Artifacts expire after 14 days. The artifact also preserves the
initial corpus archive, corpus checksums, and the effective lockfiles and their
checksums (Cargo may refresh a stale lockfile while building the target). To repeat
the seeded invocation, copy `workspace.Cargo.lock` and `fuzz.Cargo.lock` from
the extracted artifact to `Cargo.lock` and `fuzz/Cargo.lock`, respectively,
and restore the initial corpus as described in `REPLAY.md`.
A build failure may have replay context but no triggering input.

## Release WASM sizes

The compute budget job builds the PR and its exact base commit with the same
Rust toolchain, release profile, and `wasm32-unknown-unknown` target. The baseline
checkout and build directory are separate to avoid stale artifacts. The job
summary and `release-wasm-sizes` artifact include the base SHA and a table keyed
by package, artifact, and build target, with raw byte counts, signed differences,
and changed/unchanged/added/removed status. There is no size threshold.

The report measures the raw output of `cargo build --release`, before any
external WASM optimizer. A missing build artifact is an error. To reproduce it,
build both revisions separately, then run:

```bash
python3 scripts/wasm-size-report.py \
  --baseline-root /path/to/base-checkout \
  --baseline-target-dir /path/to/base-checkout/target \
  --baseline-sha <base-commit>
```

## CI checklist

The pull request workflows run these commands with Rust
`nightly-2024-08-01` as currently pinned:

- `python3 scripts/test-ci.py -v` (no Rust needed).
- `cargo test --workspace` with
  `SOROBAN_SDK_BUILD_SYSTEM_SUPPORTS_SPEC_SHAKING_V2=0`.
- `cargo test -p split storage_snapshot` with the same environment.
- `cargo install cargo-fuzz`, then `bash scripts/run-fuzz.sh <target>` for
  `fuzz_create_invoice`, `fuzz_pay`, `fuzz_release`, `fuzz_refund`, and
  `fuzz_sequence`. Each invokes `cargo fuzz run <target> fuzz/corpus/<target>
  -- -seed=<logged-seed> -max_total_time=60`.
- `cargo build --release --target wasm32-unknown-unknown` for both PR and base.
- `python3 scripts/wasm-size-report.py` with the baseline arguments above.
- `cargo test --package split -- compute_budget --nocapture` (piped through
  `tee budget_output.txt`).

The compute budget job also generates its existing static budget table, posts
the budget PR comment, and prints the existing budget threshold message.
GitHub's conflict workflow checks PR mergeability. Commenting, artifact uploads,
and GitHub mergeability checks require Actions/GitHub and cannot be fully
executed locally.

No lint, typecheck, formatting, or coverage gate is configured. Contributor
checks also include `cargo clippy`, `cargo check --workspace`, and
`cargo fmt --all -- --check`. The manually dispatched testnet deployment
workflow builds `cargo build --target wasm32-unknown-unknown --release
--package split` and then configures, funds, and deploys a testnet identity;
it does not run on pull requests.

The pinned nightly predates `wasm32v1-none` and cannot parse current dependencies
using Rust edition 2024. These are existing CI blockers; changing replay or
reporting scripts does not resolve them.
