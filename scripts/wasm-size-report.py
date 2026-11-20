#!/usr/bin/env python3
"""Compare release WASM byte counts against a separately built PR base."""

import argparse
import json
import subprocess
from pathlib import Path


def sizes(root, target_dir, target):
    metadata = json.loads(
        subprocess.check_output(
            ["cargo", "metadata", "--no-deps", "--format-version", "1"], cwd=root
        )
    )
    members = set(metadata["workspace_members"])
    result = {}
    for package in metadata["packages"]:
        if package["id"] not in members:
            continue
        for artifact in package["targets"]:
            if "cdylib" in artifact["crate_types"]:
                path = target_dir / target / "release" / (artifact["name"] + ".wasm")
                # A missing build must fail rather than silently report zero bytes.
                result[(package["name"], artifact["name"], target)] = (
                    path.stat().st_size
                )
    return result


def report(baseline, current, baseline_sha):
    lines = [
        "## Release WASM size report",
        "",
        f"Baseline: `{baseline_sha}` (PR base), built with the same toolchain and release profile.",
        "",
        "| Package | Artifact | Build target | Base bytes | PR bytes | Delta bytes | Status |",
        "|---|---|---|---:|---:|---:|---|",
    ]
    for key in sorted(baseline.keys() | current.keys()):
        before, after = baseline.get(key), current.get(key)
        if before is None:
            delta, status = "—", "added"
        elif after is None:
            delta, status = "—", "removed"
        else:
            delta = f"{after - before:+d}"
            status = "unchanged" if before == after else "changed"
        package, artifact, target = key
        lines.append(
            f"| `{package}` | `{artifact}.wasm` | `{target}` | "
            f"{before if before is not None else '—'} | "
            f"{after if after is not None else '—'} | {delta} | {status} |"
        )
    return "\n".join(lines) + "\n"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline-root", type=Path, required=True)
    parser.add_argument("--baseline-target-dir", type=Path, required=True)
    parser.add_argument("--baseline-sha", required=True)
    parser.add_argument("--target", default="wasm32-unknown-unknown")
    parser.add_argument("--target-dir", type=Path, default=Path("target"))
    args = parser.parse_args()
    baseline = sizes(args.baseline_root, args.baseline_target_dir, args.target)
    current = sizes(Path.cwd(), args.target_dir, args.target)
    print(report(baseline, current, args.baseline_sha), end="")


if __name__ == "__main__":
    main()
