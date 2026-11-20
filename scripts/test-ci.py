#!/usr/bin/env python3
"""Regression checks for fuzz failure preservation and WASM size comparisons."""

import importlib.util
import json
import os
import subprocess
import tarfile
import tempfile
import unittest
import unittest.mock
from pathlib import Path

SCRIPTS = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location(
    "wasm_sizes", SCRIPTS / "wasm-size-report.py"
)
WASM_SIZES = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(WASM_SIZES)


class SizeReportTests(unittest.TestCase):
    def test_reads_raw_bytes_using_cargo_artifact_name(self):
        metadata = {
            "workspace_members": ["invoice-escrow"],
            "packages": [
                {
                    "id": "invoice-escrow",
                    "name": "invoice-escrow",
                    "targets": [
                        {"name": "invoice_escrow", "crate_types": ["cdylib", "rlib"]},
                        {"name": "integration_tests", "crate_types": ["bin"]},
                    ],
                },
                {"id": "dependency", "name": "dependency", "targets": []},
            ],
        }
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            output = root / "target/wasm32-unknown-unknown/release"
            output.mkdir(parents=True)
            (output / "invoice_escrow.wasm").write_bytes(bytes(range(256)))
            with unittest.mock.patch.object(
                WASM_SIZES.subprocess, "check_output", return_value=json.dumps(metadata)
            ):
                self.assertEqual(
                    WASM_SIZES.sizes(root, root / "target", "wasm32-unknown-unknown"),
                    {
                        (
                            "invoice-escrow",
                            "invoice_escrow",
                            "wasm32-unknown-unknown",
                        ): 256
                    },
                )

    def test_changes_are_keyed_by_package_artifact_and_target(self):
        split = ("split", "split", "wasm32-unknown-unknown")
        factory = ("factory", "factory", "wasm32-unknown-unknown")
        removed = ("old", "old", "wasm32-unknown-unknown")
        added = ("new", "new", "wasm32-unknown-unknown")
        text = WASM_SIZES.report(
            {split: 100, factory: 200, removed: 50},
            {split: 125, factory: 200, added: 60},
            "base-commit",
        )
        self.assertIn("`base-commit`", text)
        self.assertIn("| 100 | 125 | +25 | changed |", text)
        self.assertIn("| 200 | 200 | +0 | unchanged |", text)
        self.assertIn("| 50 | — | — | removed |", text)
        self.assertIn("| — | 60 | — | added |", text)

    def test_missing_wasm_is_an_error(self):
        with (
            tempfile.TemporaryDirectory() as directory,
            unittest.mock.patch.object(
                WASM_SIZES.subprocess,
                "check_output",
                return_value=b'{"workspace_members":["split"],"packages":[{"id":"split",'
                b'"name":"split","targets":[{"name":"split","crate_types":["cdylib"]}]}]}',
            ),
            self.assertRaises(FileNotFoundError),
        ):
            WASM_SIZES.sizes(Path(directory), Path(directory), "wasm32-unknown-unknown")


class FuzzReplayTests(unittest.TestCase):
    def run_target(self, exit_code, seed="12345"):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        root = Path(directory.name)
        binaries = root / "bin"
        binaries.mkdir()
        # A synthetic crashing invocation also simulates failed minimization:
        # neither may discard the reproducer or turn the job green.
        commands = {
            "cargo": """#!/usr/bin/env bash
set -eu
if [[ $* == 'fuzz --version' ]]; then echo cargo-fuzz-test; exit; fi
printf '%s\\n' "$*" >> calls
if [[ $2 == tmin ]]; then
  printf 't' > fuzz/artifacts/fuzz_synthetic/minimized-from-input
  exit 1
fi
if [[ $SYNTHETIC_EXIT != 0 ]]; then
  printf 'trigger' > fuzz/artifacts/fuzz_synthetic/crash-input
fi
exit "$SYNTHETIC_EXIT"
""",
            "rustc": "#!/bin/sh\necho rustc-test\n",
            "git": "#!/bin/sh\necho test-commit\n",
        }
        for name, contents in commands.items():
            path = binaries / name
            path.write_text(contents)
            path.chmod(0o755)
        corpus = root / "fuzz/corpus/fuzz_synthetic"
        corpus.mkdir(parents=True)
        (corpus / "seed_zeros").write_bytes(b"\0\0")
        (root / "Cargo.lock").write_text("workspace lock")
        (root / "fuzz/Cargo.lock").write_text("fuzz lock")
        env = dict(
            os.environ,
            PATH=f"{binaries}:{os.environ['PATH']}",
            FUZZ_SEED=seed,
            SYNTHETIC_EXIT=str(exit_code),
            GITHUB_STEP_SUMMARY=str(root / "summary"),
        )
        result = subprocess.run(
            ["bash", str(SCRIPTS / "run-fuzz.sh"), "fuzz_synthetic"],
            cwd=root,
            env=env,
            capture_output=True,
            text=True,
            check=False,
        )
        return root, result

    def test_failure_preserves_input_and_replay_command(self):
        root, result = self.run_target(77)
        self.assertEqual(result.returncode, 77, result.stderr)
        artifacts = root / "fuzz/artifacts/fuzz_synthetic"
        self.assertEqual((artifacts / "crash-input").read_bytes(), b"trigger")
        self.assertIn(
            "cargo fuzz run fuzz_synthetic fuzz/artifacts/fuzz_synthetic/crash-input -- -runs=1",
            (artifacts / "REPLAY.md").read_text(),
        )
        self.assertIn("fuzz tmin", (root / "calls").read_text())
        self.assertEqual((artifacts / "minimized-from-input").read_bytes(), b"t")
        self.assertIn(
            "fuzz/artifacts/fuzz_synthetic/minimized-from-input -- -runs=1",
            (artifacts / "REPLAY.md").read_text(),
        )
        with tarfile.open(artifacts / "initial-corpus.tar") as archive:
            self.assertEqual(
                archive.extractfile("fuzz/corpus/fuzz_synthetic/seed_zeros").read(),
                b"\0\0",
            )

    def test_success_logs_effective_seed_and_invocation(self):
        root, result = self.run_target(0)
        self.assertEqual(result.returncode, 0, result.stderr)
        summary = (root / "summary").read_text()
        self.assertIn("test-commit", summary)
        self.assertIn("-seed=12345 -max_total_time=60", summary)
        self.assertIn("fuzz run fuzz_synthetic fuzz/corpus/fuzz_synthetic", summary)
        self.assertNotIn("tmin", (root / "calls").read_text())

    def test_invalid_seeds_never_invoke_the_fuzzer(self):
        for seed in ("0", "0123", "-1", "2147483648", "999999999999999999999"):
            with self.subTest(seed=seed):
                root, result = self.run_target(0, seed)
                self.assertEqual(result.returncode, 2)
                self.assertFalse((root / "calls").exists())


if __name__ == "__main__":
    unittest.main()
