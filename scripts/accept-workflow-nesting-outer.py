"""P45 outer Task pairing and P46 real declared-host outer/legacy acceptance.

Only explicit isolated dependencies and an authorized real model are accepted.
Configuration, controlled-provider success and final status are not acceptance.
"""
import argparse
import json
import os
from pathlib import Path
import runpy
import sys
import traceback

from developer_paths import REPO, backend_suffix

identity = runpy.run_path(str(REPO / 'scripts/accept-workflow-nesting-planning.py'))
source_identity, digest, require = (identity[name] for name in ['source_identity', 'digest', 'require'])
COVERAGE = ['P45/creator', 'P45/optimizer', 'P46/outer', 'P46/legacy', 'P46/scope']


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ['backend', 'toolkit', 'paths-file', 'model-config', 'evidence-dir']:
        parser.add_argument('--' + name, type=lambda value: Path(value).resolve(), required=True)
    parser.add_argument('--hosts', choices=['declared', 'omp,pi'], required=True)
    parser.add_argument('--timeout', type=int, default=300)
    args = parser.parse_args()
    args.evidence_dir.mkdir(parents=True, exist_ok=False)
    record = {'passed': False, 'requestedHosts': args.hosts, 'processes': [], 'scenarios': [], 'cleanupErrors': [],
              'coverage': {name: {'verified': False} for name in COVERAGE}, 'sources': {}, 'commands': []}
    try:
        require(Path.cwd().resolve() == REPO, 'Run from this herdr-bot worktree root')
        require(1 <= args.timeout <= 900, 'timeout must be 1..900 seconds; no business/model retries')
        for name, root, lock in [('herdr-bot', REPO, 'backend/Cargo.lock'), ('toolkit', args.toolkit, 'bun.lock')]:
            record['sources'][name] = source_identity(root, lock, args.evidence_dir)
        require(args.backend.is_file(), 'Missing current worktree backend: ' + str(args.backend))
        target = Path(os.environ.get('CARGO_TARGET_DIR', ''))
        require(target.is_absolute() and not REPO.is_relative_to(target), 'Set an independent absolute CARGO_TARGET_DIR')
        require(args.backend == (REPO / backend_suffix()).resolve(), 'Backend must match this worktree CARGO_TARGET_DIR')
        record['backend'] = {'path': str(args.backend), 'sha256': digest(args.backend)}
        from workflow_nesting_outer_acceptance import run_scenarios
        run_scenarios(args, record)
        for name, root, lock in [('herdr-bot', REPO, 'backend/Cargo.lock'), ('toolkit', args.toolkit, 'bun.lock')]:
            after = source_identity(root, lock, args.evidence_dir)
            require(after['fileSha256'] == record['sources'][name]['fileSha256'], 'Participating source changed: ' + name)
        require(all(item['verified'] for item in record['coverage'].values()), 'Owned coverage remains unverified')
        require(not record['cleanupErrors'], 'Owned process cleanup failed')
        record['passed'] = True
    except BaseException:
        record['verificationError'] = traceback.format_exc()
    output = args.evidence_dir / 'evidence.json'
    output.write_text(json.dumps(record, ensure_ascii=False, indent=2), encoding='utf-8')
    print(json.dumps({'passed': record['passed'], 'evidence': str(output)}, ensure_ascii=False))
    return 0 if record['passed'] else 1


if __name__ == '__main__':
    sys.exit(main())
