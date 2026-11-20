"""S1/public checker: actual files, package inventory and evidence directory.

The contract says a declared asset must be read, not just named. An extra
shipped file or a missing declared file could be concealed by a checker that
only trusts the generator ledger. Compare the command's observable verdicts
against the complete delivery and faults in an owned copy, never the generator.
"""

import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

BOT = Path(__file__).resolve().parents[1]


class DeliveryCheck(unittest.TestCase):
    def test_actual_closed_package_and_missing_or_unlisted_assets(self):
        with tempfile.TemporaryDirectory(prefix="pstack-package-test-") as temp:
            root = Path(temp)
            toolkit = root / "toolkit with space"
            source = Path(sys.argv[1]).resolve()
            shutil.copytree(source / "pstack-suite", toolkit / "pstack-suite")
            for path in ("adapters/omp/index.ts", "extensions/pstack/definition.ts"):
                (toolkit / path).parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(source / path, toolkit / path)
            package = toolkit / "pstack-suite"
            output = "skills/show-me-your-work/references/decision-log-template.tsv"
            resource = package / "generated" / output
            expected = hashlib.sha256(resource.read_bytes()).hexdigest()

            def run(name):
                evidence = root / name
                proc = subprocess.run([
                    sys.executable, str(BOT / "scripts/check-pstack-delivery.py"),
                    "--herdr-bot-worktree", str(BOT), "--toolkit-worktree", str(toolkit),
                    "--evidence-dir", str(evidence),
                ], cwd=BOT, capture_output=True, text=True)
                return proc, evidence

            good, evidence = run("good")
            self.assertEqual(good.returncode, 0, good.stderr)
            report = json.loads((evidence / "summary.json").read_text(encoding="utf-8"))
            self.assertEqual(report["assets"][output]["sha256"], expected)
            self.assertTrue(any(row["target"] == "generated/" + output for row in report["references"]))
            self.assertEqual(report["human"], {"H1": "unverified", "H2": "unverified"})
            occupied, _ = run("good")
            self.assertNotEqual(occupied.returncode, 0)
            self.assertEqual(json.loads((evidence / "summary.json").read_text(encoding="utf-8")), report)

            resource.unlink()
            extra = package / "generated/skills/recall/SKILL.md"
            extra.parent.mkdir(parents=True)
            extra.write_text("---\nname: recall\n---\nUnapproved placeholder.\n", encoding="utf-8")
            bad, evidence = run("bad")
            self.assertNotEqual(bad.returncode, 0)
            report = json.loads((evidence / "summary.json").read_text(encoding="utf-8"))
            self.assertTrue(any("missing generated asset" in error and output in error for error in report["errors"]))
            self.assertTrue(any("undeclared generated asset" in error and "recall/SKILL.md" in error for error in report["errors"]))
            self.assertTrue(any("unreadable reference" in error and output in error for error in report["errors"]))

            # A mapped upstream source can drift without changing its generated file
            # or ownership ledger. The release archive fixes the expected source.
            shutil.copyfile(source / "pstack-suite/generated" / output, resource)
            shutil.rmtree(extra.parent)
            pinned = package / "upstream/pstack/skills/in-your-own-words/SKILL.md"
            drifted = pinned.read_bytes() + b"\nDifferent content under the same release identity.\n"
            pinned.write_bytes(drifted)
            drift, evidence = run("drift")
            self.assertNotEqual(drift.returncode, 0)
            report = json.loads((evidence / "summary.json").read_text(encoding="utf-8"))
            self.assertIn("selected snapshot differs from reviewed upstream: pstack", report["errors"])
            self.assertEqual(pinned.read_bytes(), drifted)


if __name__ == "__main__":
    if len(sys.argv) != 2:
        raise SystemExit("usage: python scripts/check-pstack-delivery.test.py TOOLKIT_WORKTREE")
    unittest.main(argv=sys.argv[:1], exit=True)
