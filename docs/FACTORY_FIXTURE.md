# Factory WASM fixture provenance

`tests/fixtures/stub_contract.wasm` is the minimal Soroban contract used by
the factory tests to exercise WASM upload and deployment without building the
full split contract. The factory embeds it with `include_bytes!`.

The file is an unchanged copy of
[`soroban-sdk/doctest_fixtures/contract.wasm`](https://github.com/stellar/rs-soroban-sdk/blob/27ae4ffd9fce7cb93dcd704582f7540cc88f0701/soroban-sdk/doctest_fixtures/contract.wasm)
in `stellar/rs-soroban-sdk` at commit
`27ae4ffd9fce7cb93dcd704582f7540cc88f0701` (Apache-2.0;
[upstream license](https://github.com/stellar/rs-soroban-sdk/blob/27ae4ffd9fce7cb93dcd704582f7540cc88f0701/LICENSE)).
This identifies a retrievable, byte-identical upstream copy; it does not claim
that revision was the original source used when the fixture was first committed.
The binary's embedded metadata records Rust `1.74.0` and SDK
`21.0.1-preview.1` at `116c35bc9e03f4b1b5e65b5ee831ae0f86aa92fd`.

Expected size: **639 bytes**. Expected SHA-256:

```text
33d12fec8f6f3ddf2eb0ec76ee9a75a9e37d1fa20af35908d90d278af8264311
```

## Retrieve and verify

Run from the repository root with `curl`, `sha256sum`, and Rust installed.
Retrieving this pinned artifact avoids relying on a modern compiler to produce
identical bytes to the original compiler.

```bash
set -euo pipefail
fixture=$(mktemp)
trap 'rm -f "$fixture"' EXIT
curl --fail --location --proto '=https' --tlsv1.2 \
  https://raw.githubusercontent.com/stellar/rs-soroban-sdk/27ae4ffd9fce7cb93dcd704582f7540cc88f0701/soroban-sdk/doctest_fixtures/contract.wasm \
  --output "$fixture"
echo "33d12fec8f6f3ddf2eb0ec76ee9a75a9e37d1fa20af35908d90d278af8264311  $fixture" | sha256sum --check
mkdir -p tests/fixtures
cp "$fixture" tests/fixtures/stub_contract.wasm
cargo test -p factory
```

When intentionally updating the fixture, record the new immutable source,
license, size, and digest here, and run the factory tests. The fixture is already
tracked despite the repository's `*.wasm` ignore rule; include its binary diff
in the update for review.
