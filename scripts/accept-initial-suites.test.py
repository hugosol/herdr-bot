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
    def test_delivered_and_release_suites_keep_host_source_contract(self):
        acceptance = runpy.run_path(str(ROOT / 'scripts/accept-initial-suites.py'))
        exploration = ['${roots.' + root + '}' + ('/' + suffix if suffix else '')
                       for root, suffix in acceptance['EXPLORATION'].values()]
        parents = {
            'codebase-exploration': ['${roots.matt_pocock_skills}/engineering',
                '${roots.matt_pocock_skills}/productivity', '${roots.wayfinder_skills}', '${roots.expert_translate}/..'],
            'code-implementation': ['${roots.wayfinder_skills}', '${roots.matt_pocock_skills}/engineering',
                '${roots.matt_pocock_skills}/productivity'],
        }
        for source in ['meta-data/suites.json', 'migrations/initial-suite-v1/suites.json']:
            suites = json.loads((ROOT / source).read_text(encoding='utf-8'))['suites']
            for suite in suites:
                with self.subTest(source=source, suite=suite['id']):
                    self.assertEqual(set(suite['skills']), {'omp', 'pi'})
                    self.assertEqual(suite['skills']['omp'], parents.get(suite['id'], []))
                    self.assertEqual(suite['skills']['pi'], exploration if suite['id'] == 'codebase-exploration' else [])

    def test_omp_oracle_discovers_only_immediate_children_and_new_sources(self):
        discover = runpy.run_path(str(ROOT / 'scripts/accept-initial-suites.py'))['discovered_sources']
        with tempfile.TemporaryDirectory(prefix='initial-suite-sources-') as directory:
            root = Path(directory)
            matt, wayfinder, translations = root / 'matt', root / 'wayfinder', root / 'translations'
            for parent in [matt / 'engineering', matt / 'productivity', wayfinder, translations]:
                parent.mkdir(parents=True)
            for relative, header in [('wayfinder/one', 'name: first\ndescription: First'),
                    ('wayfinder/.hidden', 'name: hidden\ndescription: Hidden'),
                    ('wayfinder/missing-description', 'name: absent'),
                    ('wayfinder/disabled', 'name: disabled\ndescription: Disabled\ndisabled: false'),
                    ('wayfinder/one/nested', 'name: nested\ndescription: Nested'),
                    ('translations/expert', 'name: expert-translate\ndescription: Translate')]:
                child = root / relative
                child.mkdir(parents=True, exist_ok=True)
                (child / 'SKILL.md').write_text('---\n' + header + '\n---\nBody', encoding='utf-8')
            roots = {'matt_pocock_skills': str(matt), 'wayfinder_skills': str(wayfinder),
                     'expert_translate': str(translations / 'expert')}
            first = discover(roots, 'codebase-exploration')
            self.assertEqual(set(first), {'first', 'expert-translate'})
            added = matt / 'engineering/added'
            added.mkdir()
            (added / 'SKILL.md').write_text('---\nname: added\ndescription: Added\n---\nBody', encoding='utf-8')
            second = discover(roots, 'codebase-exploration')
            self.assertEqual(set(second), {'first', 'expert-translate', 'added'})
            self.assertEqual(second['added'], added.resolve())


if __name__ == '__main__':
    unittest.main()
