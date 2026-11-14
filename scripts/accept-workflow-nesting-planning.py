"""P03 real native planner acceptance, using only explicit isolated dependencies.

Public seams: shipped planning Tasks through Chat HTTP, native tool/history,
real model transport, strict Map HTTP readback/preflight, read-only Run catalog.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import traceback

from developer_paths import REPO, backend_suffix


from acceptance.host.processes import require
from acceptance.host.provenance import digest, source_identity


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ['backend', 'toolkit', 'paths-file', 'model-config', 'evidence-dir']:
        parser.add_argument('--' + name, type=lambda value: Path(value).resolve(), required=True)
    parser.add_argument('--hosts', choices=['declared', 'omp,pi'], required=True)
    parser.add_argument('--timeout', type=int, default=300)
    args = parser.parse_args()
    args.evidence_dir.mkdir(parents=True, exist_ok=False)
    record = {'passed': False, 'requestedHosts': args.hosts, 'processes': [], 'scenarios': [], 'cleanupErrors': [],
              'coverage': {'P03': {'verified': False}}, 'sources': {}, 'commands': []}
    try:
        require(Path.cwd().resolve() == REPO, 'Run from this herdr-bot worktree root')
        require(1 <= args.timeout <= 900, 'timeout must be 1..900 seconds; no model/business retries')
        record['sources']['herdr-bot'] = source_identity(REPO, 'backend/Cargo.lock', args.evidence_dir)
        record['sources']['toolkit'] = source_identity(args.toolkit, 'bun.lock', args.evidence_dir)
        require(args.backend.is_file(), 'Missing current worktree backend: ' + str(args.backend))
        target = Path(os.environ.get('CARGO_TARGET_DIR', ''))
        require(target.is_absolute() and not REPO.is_relative_to(target),
                'Set an absolute independent CARGO_TARGET_DIR, not a source-root ancestor')
        require(args.backend == (REPO / backend_suffix()).resolve(), 'Backend must match this worktree independent CARGO_TARGET_DIR')
        record['backend'] = {'path': str(args.backend), 'sha256': digest(args.backend)}
        from workflow_nesting_planning_acceptance import run_scenarios
        run_scenarios(args, record)
        for name, root, lock in [('herdr-bot', REPO, 'backend/Cargo.lock'), ('toolkit', args.toolkit, 'bun.lock')]:
            after = source_identity(root, lock, args.evidence_dir)
            require(after['fileSha256'] == record['sources'][name]['fileSha256'],
                    'Participating source changed during native acceptance: ' + name)
        record['coverage']['P03']['verified'] = True
        record['passed'] = True
    except BaseException:
        record['verificationError'] = traceback.format_exc()
    output = args.evidence_dir / 'evidence.json'
    output.write_text(json.dumps(record, ensure_ascii=False, indent=2), encoding='utf-8')
    print(json.dumps({'passed': record['passed'], 'evidence': str(output)}, ensure_ascii=False))
    return 0 if record['passed'] else 1


if __name__ == '__main__':
    sys.exit(main())
