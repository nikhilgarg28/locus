"""Exercise the real shell gate with deterministic substitute build tools."""
import json
import os
import pathlib
import shutil
import subprocess
import tempfile
import unittest

ROOT = pathlib.Path(__file__).resolve().parent.parent
BANNER = "LOCUS GATE COMPLETE"


class GateReceipt(unittest.TestCase):
    def run_gate(self, failure=None, extended=False):
        with tempfile.TemporaryDirectory(prefix="locus-gate-") as directory:
            root = pathlib.Path(directory)
            (root / "tools").mkdir()
            (root / "bin").mkdir()
            for name in ("check.sh", "gate.py"):
                shutil.copy(ROOT / "tools" / name, root / "tools" / name)
            for tool in ("highlight", "spec", "bench", "metrics", "site", "test_site"):
                (root / f"tools/{tool}.py").write_text("pass\n")
            if failure == "website":
                (root / "tools/site.py").write_text("raise SystemExit(42)\n")
            (root / "target/release").mkdir(parents=True)
            binary=root / "target/release/locus"
            binary.write_text("#!/bin/bash\nexit 0\n")
            binary.chmod(0o755)
            for name in ("node", "cargo"):
                body = "#!/bin/bash\n"
                if name == "cargo" and failure and failure != "website":
                    condition = '*"--release"*' if failure.startswith("extended_") else '"test --locked --offline"'
                    body += f'if [[ "$*" == {condition} ]]; then\n'
                    body += ('kill -TERM "$PPID"; exit 143\n' if failure.endswith("killed")
                             else 'echo "test result: ok. 999 passed"; exit 42\n')
                    body += "fi\n"
                body += "exit 0\n"
                path = root / "bin" / name
                path.write_text(body)
                path.chmod(0o755)
            env = dict(os.environ, PATH=str(root / "bin") + os.pathsep + os.environ["PATH"])
            args = ["bash", str(root / "tools/check.sh")]
            if extended:
                args.append("--extended")
            result = subprocess.run(args, env=env, text=True, capture_output=True, timeout=15)
            history = root / "target/gate-history.jsonl"
            records = [json.loads(line) for line in history.read_text().splitlines()] if history.exists() else []
            return result, records

    def test_success_has_one_final_receipt_and_measurement(self):
        for extended in (False, True):
            result, records = self.run_gate(extended=extended)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(result.stdout.count(BANNER), 1)
            self.assertTrue(result.stdout.strip().splitlines()[-1].startswith(BANNER))
            self.assertEqual(len(records), 1)
            self.assertEqual(records[0]["mode"], "extended" if extended else "fast")
            self.assertIsInstance(records[0]["fast_seconds"], int)

    def test_failed_or_killed_suite_cannot_claim_completion(self):
        for failure in ("website", "failed", "killed", "extended_failed", "extended_killed"):
            result, records = self.run_gate(failure=failure, extended=failure.startswith("extended_"))
            self.assertNotEqual(result.returncode, 0)
            self.assertNotIn(BANNER, result.stdout)
            self.assertEqual(records, [])


if __name__ == "__main__":
    unittest.main()
