"""S3 delivery acceptance: foreign binaries cannot certify native discovery.

Approved seams: S3 CLI/deployment, S2 Task/Chat HTTP and S4 installed hosts
through Herdr (the latter two are exercised by the real runner, not this test).
Oracle: ticket 14 requires the current worktree's delivered backend. A plausible
violation is accepting any executable, then reporting manifest-only success.
Observe nonzero, all four promises unverified, and no started host processes.
"""
import json
import runpy
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parent.parent


class DeliveryCli(unittest.TestCase):
    def test_unrelated_executable_cannot_certify_initial_native_suites(self):
        with tempfile.TemporaryDirectory(prefix='initial-suite-cli-') as directory:
            root = Path(directory)
            paths = root / 'dependencies.toml'
            paths.write_text('[roots]\n', encoding='utf-8')
            result = subprocess.run([sys.executable, str(ROOT / 'scripts/accept-initial-suites.py'),
                '--herdr-bot-worktree', str(ROOT), '--toolkit-worktree', str(ROOT),
                '--backend', sys.executable, '--paths-file', str(paths),
                '--herdr-bot-base', 'HEAD', '--toolkit-base', 'HEAD',
                '--evidence-dir', str(root / 'evidence')], capture_output=True, text=True, timeout=30)
            self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
            evidence_file = root / 'evidence/evidence.json'
            self.assertTrue(evidence_file.is_file(), result.stdout + result.stderr)
            evidence = json.loads(evidence_file.read_text(encoding='utf-8'))
            self.assertIn('backend provenance', evidence['error'])
            self.assertFalse(evidence['passed'])
            self.assertEqual(evidence['startedProcesses'], [])
            self.assertEqual(set(evidence['coverage']), {
                'P11/exploration-omp', 'P11/exploration-pi', 'P11/implementation-omp',
                'P42', 'P44'})
            self.assertTrue(all(not item['verified'] for item in evidence['coverage'].values()))


class SkillSources(unittest.TestCase):
    def test_omp_oracle_discovers_only_immediate_children_and_new_sources(self):
        discover = runpy.run_path(str(ROOT / 'scripts/accept-initial-suites.py'))['discovered_sources']
        with tempfile.TemporaryDirectory(prefix='initial-suite-sources-') as directory:
            root = Path(directory)
            wayfinder = root / 'wayfinder'
            exploration = wayfinder / 'dependent-skills/codebase-exploration'
            implementation = wayfinder / 'dependent-skills/code-implementation'
            for parent in [wayfinder / 'skills', exploration, implementation]:
                parent.mkdir(parents=True)
            for relative, header in [('wayfinder/skills/one', 'name: first\ndescription: First'),
                    ('wayfinder/skills/.hidden', 'name: hidden\ndescription: Hidden'),
                    ('wayfinder/skills/missing-description', 'name: absent'),
                    ('wayfinder/skills/disabled', 'name: disabled\ndescription: Disabled\ndisabled: false'),
                    ('wayfinder/skills/one/nested', 'name: nested\ndescription: Nested'),
                    ('wayfinder/dependent-skills/codebase-exploration/expert-translate', 'name: expert-translate\ndescription: Translate'),
                    ('wayfinder/dependent-skills/code-implementation/retro', 'name: retro\ndescription: Retro')]:
                child = root / relative
                child.mkdir(parents=True, exist_ok=True)
                (child / 'SKILL.md').write_text('---\n' + header + '\n---\nBody', encoding='utf-8')
            roots = {'wayfinder': str(wayfinder), 'wayfinder_skills': str(wayfinder / 'skills')}
            first = discover(roots, 'codebase-exploration')
            self.assertEqual(set(first), {'first', 'expert-translate'})
            self.assertEqual(set(discover(roots, 'code-implementation')), {'first', 'retro'})
            added = exploration / 'added'
            added.mkdir()
            (added / 'SKILL.md').write_text('---\nname: added\ndescription: Added\n---\nBody', encoding='utf-8')
            second = discover(roots, 'codebase-exploration')
            self.assertEqual(set(second), {'first', 'expert-translate', 'added'})
            self.assertEqual(second['added'], added.resolve())
            self.assertEqual(set(discover(roots, 'code-implementation')), {'first', 'retro'})


if __name__ == '__main__':
    unittest.main()
