"""P12/P13/P28-P32/P35 at S2 HTTP, S3 deployment and S4 real Herdr hosts.

Only the external model is controlled. Native tools, extension factories, auth
stores, session managers, launch/resume and workflow association remain real.
The observation extension uses documented native events/read-only APIs; it does
not replace a product extension, modify host methods, or create a product API.
"""
import argparse
import json
import os
from pathlib import Path
import sys
import traceback

from developer_paths import REPO
from acceptance.host.isolation import require
from acceptance.host.provenance import digest, preflight
from acceptance.suite_lifecycle import Acceptance, PROMISES, SCENARIOS


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for flag in ['herdr-bot-worktree', 'toolkit-worktree', 'backend', 'paths-file', 'evidence-dir']:
        parser.add_argument('--' + flag, type=lambda value: Path(value).resolve(), required=True)
    parser.add_argument('--scenario', choices=['all', *SCENARIOS], default='all')
    parser.add_argument('--host', choices=['all', 'pi', 'omp'], default='all')
    parser.add_argument('--timeout', type=int, choices=range(1, 301), default=90)
    args = parser.parse_args()
    args.repo, args.toolkit, args.node = args.herdr_bot_worktree, args.toolkit_worktree, 'node'
    args.evidence_dir.mkdir(parents=True, exist_ok=False)
    scenarios = list(SCENARIOS) if args.scenario == 'all' else [args.scenario]
    hosts = ['pi', 'omp'] if args.host == 'all' else [args.host]
    evidence = {'passed': False, 'argv': sys.argv[1:], 'coverage': {p: {'expected': expected, 'verified': False, 'items': []} for p, expected in PROMISES.items()},
        'startedProcesses': [], 'cleanupErrors': [], 'runs': []}
    try:
        evidence['entrypoint'] = {'path': str(Path(__file__).resolve()), 'sha256': digest(Path(__file__))}
        require(args.backend.is_file(), 'Missing current worktree backend: ' + str(args.backend))
        require('CARGO_TARGET_DIR' in os.environ and Path(os.environ['CARGO_TARGET_DIR']).resolve().is_relative_to(REPO),
                'Use a private CARGO_TARGET_DIR within the explicit herdr-bot worktree')
        require(args.backend.is_relative_to(Path(os.environ['CARGO_TARGET_DIR']).resolve()), 'Backend is outside worktree-private CARGO_TARGET_DIR')
        preflight(args, evidence)
        evidence['stateIntegrations'] = {}
        for host in ['pi', 'omp']:
            key = host + '_state_extension'
            require(key in evidence['paths']['roots'], 'Isolated paths must name readonly roots.' + key)
            path = Path(evidence['paths']['roots'][key])
            require(path.is_file(), ('Missing installed Herdr state extension', str(path)))
            setattr(args, key, path)
            evidence['stateIntegrations'][host] = {'path': str(path), 'sha256': digest(path)}
        for host in hosts:
            for scenario in scenarios:
                run = Acceptance(args, host, scenario)
                result = run.run()
                evidence['runs'].append({'host': host, 'scenario': scenario, 'exit': result, 'evidence': str(run.root / 'evidence.json')})
                evidence['startedProcesses'].extend({'pid': p.child.pid, 'exit': p.child.poll()} for p in run.processes)
                evidence['cleanupErrors'].extend(run.evidence['cleanupErrors'])
                for promise in SCENARIOS[scenario]:
                    evidence['coverage'][promise]['items'].append({'host': host, 'scenario': scenario, 'verified': result == 0, 'evidence': str(run.root / 'evidence.json')})
        for promise, item in evidence['coverage'].items():
            expected_scenarios = [s for s, promises in SCENARIOS.items() if promise in promises]
            item['verified'] = all(any(i['host'] == h and i['scenario'] == s and i['verified'] for i in item['items']) for h in ['pi', 'omp'] for s in expected_scenarios)
        evidence['selectedScenarioSucceeded'] = all(r['exit'] == 0 for r in evidence['runs']) and not evidence['cleanupErrors']
        evidence['passed'] = all(item['verified'] for item in evidence['coverage'].values()) and not evidence['cleanupErrors']
    except BaseException:
        evidence['error'] = traceback.format_exc()
    (args.evidence_dir / 'evidence.json').write_text(json.dumps(evidence, ensure_ascii=False, indent=2), encoding='utf-8')
    print(json.dumps({'passed': evidence['passed'], 'evidence': str(args.evidence_dir / 'evidence.json')}, indent=2))
    return 0 if evidence['passed'] else 1


if __name__ == '__main__':
    sys.exit(main())
