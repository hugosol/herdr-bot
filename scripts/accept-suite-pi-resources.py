"""S2/S3/S4 Pi resources and native MCP acceptance, using two explicit worktrees.

The controlled model and file-backed MCP endpoints supply external conditions only.
Actual Task HTTP -> Herdr -> installed agent -> native tools/resources remains real.
Each scenario owns its deployment, processes, dynamic ports and retained evidence.
"""
import argparse
import json
from pathlib import Path
import sys
import traceback

from acceptance.host.isolation import require
from acceptance.host.provenance import preflight
from acceptance.suite_resources import Acceptance, SCENARIOS


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ['herdr-bot-worktree', 'toolkit-worktree', 'backend', 'paths-file', 'evidence-dir']:
        parser.add_argument('--' + name, type=Path, required=True)
    parser.add_argument('--scenario', choices=['all', *SCENARIOS], default='all')
    parser.add_argument('--timeout', type=int, choices=range(1, 301), default=90)
    args = parser.parse_args()
    args.repo, args.toolkit = args.herdr_bot_worktree.resolve(), args.toolkit_worktree.resolve()
    args.backend, args.paths_file, args.evidence_dir = args.backend.resolve(), args.paths_file.resolve(), args.evidence_dir.resolve()
    args.evidence_dir.mkdir(parents=True, exist_ok=False)
    selected = list(SCENARIOS) if args.scenario == 'all' else [args.scenario]
    evidence = {'passed': False, 'coverage': {item: {'verified': False, 'scenario': scenario} for scenario in selected for item in SCENARIOS[scenario]},
                'startedProcesses': [], 'scenarios': [], 'cleanupErrors': [], 'argv': sys.argv[1:]}
    try:
        preflight(args, evidence)
        for scenario in selected:
            run = Acceptance(args, scenario)
            result = run.run()
            evidence['startedProcesses'].extend({'pid': p.child.pid, 'exit': p.child.poll()} for p in run.processes)
            evidence['scenarios'].append({'scenario': scenario, 'exit': result, 'evidence': str(run.root / 'evidence.json')})
            evidence['cleanupErrors'].extend(run.evidence['cleanupErrors'])
            for item in SCENARIOS[scenario]:
                evidence['coverage'][item].update(verified=result == 0, evidence=str(run.root / 'evidence.json'))
        evidence['passed'] = all(item['verified'] for item in evidence['coverage'].values()) and not evidence['cleanupErrors']
    except BaseException:
        evidence['error'] = traceback.format_exc()
    (args.evidence_dir / 'evidence.json').write_text(json.dumps(evidence, ensure_ascii=False, indent=2), encoding='utf-8')
    print(json.dumps({'passed': evidence['passed'], 'evidence': str(args.evidence_dir / 'evidence.json')}, indent=2))
    return 0 if evidence['passed'] else 1


if __name__ == '__main__':
    sys.exit(main())
