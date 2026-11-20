#!/usr/bin/env bash
set -euo pipefail

wasm=$(mktemp)
trap 'rm -f "$wasm"' EXIT
printf 'fixture' > "$wasm"

contract_id=CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4
commit_sha=0123456789abcdef0123456789abcdef01234567
manifest=$(STELLAR_SECRET_KEY=fixture-secret bash scripts/testnet-deployment-manifest.sh "$contract_id" "$commit_sha" "$wasm")

jq -e --arg contract_id "$contract_id" --arg commit_sha "$commit_sha" '
  . == {
    network: "testnet",
    contract_id: $contract_id,
    commit_sha: $commit_sha,
    wasm_sha256: "f16d05ec6b29248d2c61adb1e9263f78e4f7bace1b955014a2d17872cfe4064d"
  }
' <<< "$manifest" >/dev/null

if bash scripts/testnet-deployment-manifest.sh fixture-secret "$commit_sha" "$wasm" >/dev/null 2>&1; then
  echo "Manifest accepted an invalid contract ID" >&2
  exit 1
fi
