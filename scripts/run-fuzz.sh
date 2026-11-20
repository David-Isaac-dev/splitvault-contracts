#!/usr/bin/env bash
# Keep replay context before running so a crash cannot discard it.
# Markdown backticks in printf format strings are intentionally literal.
# shellcheck disable=SC2016
set -euo pipefail

target=${1:?usage: run-fuzz.sh TARGET}
seed=${FUZZ_SEED:-$(python3 -c 'import secrets; print(secrets.randbelow(2147483647) + 1)')}
if [[ ! $target =~ ^fuzz_[a-z_]+$ || ! $seed =~ ^[1-9][0-9]{0,9}$ ]] ||
  (( seed < 1 || seed > 2147483647 )); then
  echo 'Expected a fuzz target and a seed between 1 and 2147483647' >&2
  exit 2
fi

artifacts="fuzz/artifacts/$target"
corpus="fuzz/corpus/$target"
mkdir -p "$artifacts" "$corpus"
command=(cargo fuzz run "$target" "$corpus" -- "-seed=$seed" -max_total_time=60)
printf -v invocation '%q ' "${command[@]}"
{
  printf '### Fuzz replay: `%s`\n\n' "$target"
  printf -- '- Commit: `%s`\n' "$(git rev-parse HEAD)"
  printf -- '- CI run/attempt: `%s/%s`\n' "${GITHUB_RUN_ID:-local}" "${GITHUB_RUN_ATTEMPT:-1}"
  printf -- '- Toolchain: `%s`\n' "$(rustc --version)"
  printf -- '- cargo-fuzz: `%s`\n' "$(cargo fuzz --version)"
  printf -- '- Seed: `%s`; initial corpus: `%s`\n\n' "$seed" "$corpus"
  printf 'From the repository root at the commit above:\n\n```bash\n%s\n```\n' "$invocation"
  printf '\nThe 60-second time limit and runtime differences can change the explored inputs.\n'
} | tee "$artifacts/REPLAY.md"
if [[ -n ${GITHUB_STEP_SUMMARY:-} ]]; then
  cat "$artifacts/REPLAY.md" >> "$GITHUB_STEP_SUMMARY"
fi
# Preserve the initial corpus as well as its digests; fuzzing mutates the corpus.
tar -cf "$artifacts/initial-corpus.tar" "$corpus"
find "$corpus" -type f -print0 | sort -z | xargs -0 -r sha256sum > "$artifacts/corpus.sha256"
sha256sum Cargo.lock fuzz/Cargo.lock > "$artifacts/locks-before.sha256"

status=0
"${command[@]}" || status=$?
# Cargo can refresh an outdated lockfile while building the target. Preserve
# the effective lockfiles, not just their state before dependency resolution.
cp Cargo.lock "$artifacts/workspace.Cargo.lock"
cp fuzz/Cargo.lock "$artifacts/fuzz.Cargo.lock"
sha256sum Cargo.lock fuzz/Cargo.lock > "$artifacts/locks.sha256"
if (( status != 0 )); then
  # Preserve original inputs even if minimization fails or exceeds its deadline.
  for input in "$artifacts"/crash-*; do
    [[ -f $input ]] || continue
    timeout 30s cargo fuzz tmin "$target" "$input" -- -max_total_time=15 || true
  done
  {
    printf '\n## Replay a failure\n\nExtract this artifact into `%s/`.\n' "$artifacts"
    printf 'The original inputs are retained alongside any minimized inputs.\n\n```bash\n'
    for input in "$artifacts"/{crash,minimized-from,oom,timeout,slow-unit,leak}-*; do
      [[ -f $input ]] || continue
      printf 'cargo fuzz run %q %q -- -runs=1\n' "$target" "$input"
    done
    printf '```\n\nTo restore the initial corpus for a seeded run, remove the generated\n'
    printf '`%s` directory and extract `initial-corpus.tar` at the repository root.\n' "$corpus"
  } >> "$artifacts/REPLAY.md"
fi
exit "$status"
