"""Ticket 14 cell selection, subprocess result contract and bounded dispatch."""
from concurrent.futures import FIRST_COMPLETED, ThreadPoolExecutor, wait
import hashlib
import json
from pathlib import Path
import re
import subprocess
from urllib.parse import urlsplit

HOSTS = ('omp', 'pi')
CASE_MAPS = {
    'v1': 'create-bot-wrapper.json',
    'outer': 'create-bot-task-workflow--bc-61ce48b80a4c.json',
    'revision': 'enhance-existing-bot--bc-301b25d9f8d6.json',
    'planning': 'plan_workflow.json',
    'investigation': 'tibo-monitor-then-c-drive-inspection.json',
}
WORKER_REPORT = 'worker.json'


def selected_cells(hosts, cases):
    return [(host, case) for host in hosts.split(',')
            for case in (CASE_MAPS if cases == 'all' else (cases,))]


def cell_keys(host, case, responsibilities):
    filename = CASE_MAPS[case]
    keys = {f'P34/{seam}/{host}/{filename}/{node}'
            for node in responsibilities[filename] for seam in ('S2', 'S3')}
    if case == 'v1':
        keys.update((f'P31/S2/{host}', f'P35/S2/{host}'))
    return keys


def concurrency(model, jobs, cell_count):
    capacity = model.get('maxConcurrentRequests')
    if 'maxConcurrentRequests' in model and (type(capacity) is not int or capacity < 1):
        raise ValueError('maxConcurrentRequests must be an explicit positive integer')
    return {'requestedJobs': jobs, 'modelCapacity': capacity,
            'effectiveJobs': min(jobs, capacity or 1, cell_count)}


def cell_model_config(model, host, case):
    config = dict(model)
    if 'cellBaseUrls' in config:
        if (config['provider'], config['apiKeyEnv']) != ('codex-completion-transport', 'WORKFLOW_CODEX_TRANSPORT_KEY'):
            raise ValueError('Scoped endpoints require the isolated Codex transport provider and credential marker')
        urls = config.pop('cellBaseUrls')
        key = f'{host}/{case}'
        url = urls.get(key) if isinstance(urls, dict) else None
        if not isinstance(url, str) or not url:
            raise ValueError(f'Missing exact scoped model endpoint for {key}')
        match = re.fullmatch(r'http://127\.0\.0\.1:([0-9]{5})', url)
        if not match or not 49152 <= int(match[1]) <= 65535:
            raise ValueError(f'Scoped model endpoint for {key} must be numeric loopback HTTP on a high port')
        config['baseUrl'] = url
    else:
        url = config.get('baseUrl')
        if not isinstance(url, str) or not url.strip():
            raise ValueError('Authorized model configuration requires baseUrl')
        parsed = urlsplit(url)
        if (parsed.scheme not in ('http', 'https') or not parsed.hostname or '@' in parsed.netloc
                or '?' in url or '#' in url):
            raise ValueError('Authorized baseUrl must be an HTTP(S) URL without inline credentials, query or fragment')
    return config


def scoped_models(model, cells):
    models = {cell: cell_model_config(model, *cell) for cell in cells}
    if 'cellBaseUrls' in model and len({config['baseUrl'] for config in models.values()}) != len(models):
        raise ValueError('Each selected cell requires a distinct scoped model endpoint')
    return models


def source_fingerprint(identity):
    manifest = json.dumps({key: identity[key] for key in ('revision', 'lockSha256', 'fileSha256')},
                          sort_keys=True, separators=(',', ':'))
    return hashlib.sha256(manifest.encode('utf-8')).hexdigest()


def cell_root(evidence_dir, host, case):
    return evidence_dir / 'cells' / host / case


def write_json(path, data):
    with path.open('x', encoding='utf-8') as stream:
        json.dump(data, stream, ensure_ascii=False, indent=2)


def cell_passed(report, keys):
    coverage = report['coverage']
    return (not report.get('error') and not report.get('nativeErrors')
            and not report['cleanupRisk'] and not report['cleanupErrors'] and not report['sharedFailures']
            and len(report['runs']) == 1 and type(report['runs'][0].get('exit')) is int
            and report['runs'][0]['exit'] == 0
            and (report['runs'][0].get('host'), report['runs'][0].get('case')) == (report['managerHost'], report['case'])
            and set(coverage) == keys and all(item.get('verified') is True for item in coverage.values()))


def run_cell_process(host, case, directory, argv, expected_keys, *, cwd, env, source_fingerprints):
    native_dir = directory / 'native'
    report_path = native_dir / WORKER_REPORT
    log_path = directory / 'worker.log'
    result = {'host': host, 'case': case, 'cellRoot': str(directory), 'workerEvidence': str(report_path),
              'log': str(log_path), 'state': 'failed'}
    try:
        with log_path.open('xb') as log, subprocess.Popen(argv, cwd=cwd, env=env, stdout=log,
                                                           stderr=subprocess.STDOUT) as process:
            result['pid'] = process.pid
            result['exit'] = process.wait()
        if result['exit'] not in (0, 1):
            raise ValueError(f'Worker exited without a confirmed cleanup outcome (exit {result["exit"]})')
        report = json.loads(report_path.read_text(encoding='utf-8'))
        if (report['managerHost'], report['case']) != (host, case):
            raise ValueError('Worker result names a different cell')
        for field in ('runs', 'cleanupErrors', 'sharedFailures'):
            if not isinstance(report[field], list):
                raise ValueError(f'Invalid worker field {field}')
        if (type(report['cleanupRisk']) is not bool or type(report['cellPassed']) is not bool
                or not isinstance(report['coverage'], dict) or not isinstance(report.get('nativeErrors', []), list)):
            raise ValueError('Missing typed cell result (diagnostic evidence is not a worker result)')
        expected_run = native_dir.resolve() / host / 'evidence.json'
        for run in report['runs']:
            path = Path(run['evidence'])
            if ((run['host'], run['case']) != (host, case) or type(run['exit']) is not int or not path.is_absolute()
                    or path.resolve() != expected_run or not path.is_file()):
                raise ValueError('Invalid or non-owned native run evidence')
        passed = cell_passed(report, expected_keys)
        verified = {key: item for key, item in report['coverage'].items() if item.get('verified') is True}
        if verified and (len(report['runs']) != 1 or any(item.get('sourceEvidence') != report['runs'][0]['evidence']
                                                         for item in verified.values())):
            raise ValueError('Coverage does not refer to this cell\'s native evidence')
        result.update({key: report[key] for key in ('runs', 'cleanupErrors', 'sharedFailures')})
        if report.get('error'):
            result['error'] = report['error']
        if report.get('nativeErrors'):
            result['nativeErrors'] = report['nativeErrors']
        if report['cleanupRisk'] or report['cleanupErrors']:
            result['haltReason'] = 'worker-cleanup-risk'
        elif report['sharedFailures']:
            result['haltReason'] = 'shared-failure'
        fingerprints = report.get('sourceFingerprints', {})
        result['sourceFingerprints'] = fingerprints
        if any(snapshot != source_fingerprints for snapshot in fingerprints.values()):
            result['sharedFailures'].append({'kind': 'source', 'reason': 'cell source differs from parent preflight'})
            if not result.get('haltReason'):
                result['haltReason'] = 'shared-failure'
        if (passed != report['cellPassed'] or (result['exit'] == 0) != passed
                or not set(report['coverage']) <= expected_keys
                or (verified and set(fingerprints) != {'before', 'after'})):
            result['protocolError'] = 'Worker exit, cell outcome, exact cell coverage, or source gate disagrees'
            if not result.get('haltReason'):
                result['haltReason'] = 'worker-protocol-unconfirmed'
        elif not result.get('haltReason'):
            if verified:
                result['coverage'] = verified
            if passed:
                result['state'] = 'passed'
    except (OSError, ValueError, KeyError, AttributeError, TypeError) as error:
        result.update(error=f'{type(error).__name__}: {error}', haltReason='worker-cleanup-unconfirmed')
    return result


def dispatch_cells(cells, effective_jobs, run_cell):
    """Drain every started cell and stop dispatch after an observed shared/cleanup failure."""
    results = {cell: {'host': cell[0], 'case': cell[1], 'state': 'not-run'} for cell in cells}
    next_cells = iter(results)
    halted = False
    with ThreadPoolExecutor(max_workers=effective_jobs, thread_name_prefix='v1-cell') as pool:
        active = {}
        while True:
            while not halted and len(active) < effective_jobs:
                cell = next(next_cells, None)
                if cell is None:
                    break
                active[pool.submit(run_cell, *cell)] = cell
            if not active:
                return list(results.values())
            done, _ = wait(active, return_when=FIRST_COMPLETED)
            # Observe a whole batch before launching replacements.
            for future in done:
                cell = active.pop(future)
                try:
                    result = future.result()
                except Exception as error:
                    result = {'host': cell[0], 'case': cell[1], 'state': 'failed',
                              'error': f'{type(error).__name__}: {error}', 'haltReason': 'worker-cleanup-unconfirmed'}
                results[cell] = result
                halted |= bool(result.get('haltReason'))
