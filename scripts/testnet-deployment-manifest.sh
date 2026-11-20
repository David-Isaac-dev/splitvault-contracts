#!/usr/bin/env bash
set -euo pipefail

contract_id=$1
commit_sha=$2
wasm=$3

if [[ ! $contract_id =~ ^C[A-Z2-7]{55}$ ]]; then
  echo "Expected a Stellar contract ID" >&2
  exit 1
fi

if [[ ! $commit_sha =~ ^[a-f0-9]{40}$ ]]; then
  echo "Expected a full Git commit SHA" >&2
  exit 1
fi

digest=$(sha256sum < "$wasm")
jq -n \
  --arg contract_id "$contract_id" \
  --arg commit_sha "$commit_sha" \
  --arg wasm_sha256 "${digest%% *}" \
  '{network: "testnet", contract_id: $contract_id, commit_sha: $commit_sha, wasm_sha256: $wasm_sha256}'
