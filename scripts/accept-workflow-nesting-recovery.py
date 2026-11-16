"""P29/P32/P33: real new-process OMP/Pi recovery, retry and backend restart.

Approved seams: public CLI/HTTP, installed native hosts and SessionManager,
read-only native/Run facts, OS process handles and actual file bytes. No private
API, observer reconnect, controlled provider or role-configuration assertion can
stand in for native recovery. The observer records identity only; actual native
tools and backend responses independently establish both role destinations.
"""
import argparse
import hashlib
import json
from pathlib import Path
import sys
import time
import traceback

from developer_paths import REPO, backend_suffix, resolve_arguments
from acceptance.host.native import NativeAcceptance
from acceptance.host.wire import WireProxy, decode
from acceptance.host.processes import require, until

# Actual business fixture declarations, retained verbatim in declared mode.
DECLARED = {'parent': 'omp', 'child': 'pi'}
EXPECTED = {
    'P29': 'Confirmed failure/retry creates new execution/child; old Chat, history and effect remain old',
    'P32': 'Each host resumes current and retired dual-role Chat in a genuinely new process without input replay or new Run',
    'P33': 'After actual backend process replacement historical links remain readable and native roles cannot take over',
}


class RecoveryAcceptance(NativeAcceptance):
    def __init__(self, args):
        super().__init__(args)
        self.roles = DECLARED if self.host == 'declared' else dict.fromkeys(DECLARED, self.host)
        self.observer = self.root / 'native-identity.ts'
        self.identity_log = self.root / 'native-identity.jsonl'
        self.evidence.update(entrypoint={'path': str(Path(__file__).resolve()), 'sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest()},
            promise=None, promises=list(EXPECTED), scenario='native-recovery',
            verified=[], expected=EXPECTED, declaredRoles=self.roles, retryPolicy='No runner business/model retries')

    def role_catalog(self, toolkit):
        source = '''import {appendFileSync} from "node:fs";
export default function(pi) {
 const observe=(event,ctx)=>appendFileSync(FILE,JSON.stringify({event:event.type,pid:process.pid,
  sessionId:ctx.sessionManager.getSessionId(),file:ctx.sessionManager.getSessionFile(),cwd:ctx.cwd})+"\\n");
 pi.on("session_start",observe); pi.on("before_agent_start",observe);
}
'''.replace('FILE', json.dumps(str(self.identity_log)))
        self.observer.write_text(source, encoding='utf-8')
        catalog = super().role_catalog(toolkit)
        for host in self.hosts:
            catalog += f'[[extensions]]\nid = "{host}-identity-observer"\nlabel = "Read-only identity observer"\nagent = "{host}"\nkind = "fixed"\nentrypoint = {json.dumps(str(self.observer))}\n'
        return catalog

    def events(self, session):
        return [value for line in self.identity_log.read_text(encoding='utf-8').splitlines()
            if (value := json.loads(line))['sessionId'] == session] if self.identity_log.exists() else []

    def settled(self):
        def quiet():
            records = self.backend_wire.records + self.model_wire.records
            return records and all('ended' in record for record in records) and time.monotonic() - max(record['ended'] for record in records) >= 1.25
        until('actual model/tool response settlement', quiet, self.args.timeout)
        require(all(record.get('status') == 200 and not record.get('error') for record in self.model_wire.records),
            'Actual model transport failed; subsequent decisions cannot hide that error')

    def facts(self):
        return {str(path.relative_to(self.root / 'data/runs')): hashlib.sha256(path.read_bytes()).hexdigest()
            for path in (self.root / 'data/runs').rglob('*') if path.is_file()}

    def start_event(self, chat):
        return until('actual native process identity event', lambda: next((e for e in reversed(self.events(chat['sessionId']))
            if e['event'] == 'session_start'), None), self.args.timeout)

    def close_chat(self, chat):
        event = self.start_event(chat)
        self.observe_processes()
        handle = next(handle for pid, handle in self.handles if pid == event['pid'])
        require(handle.alive(), 'Native process was not alive before deliberate close')
        self.herdr('tab', 'close', chat['tabId'])
        until('native process exit before new-process resume', lambda: not handle.alive(), 30)
        self.evidence.setdefault('retiredProcesses', []).append({'sessionId': chat['sessionId'], 'pid': event['pid'], 'exited': True})
        return event

    def resume_chat(self, execution, parent, readonly=False):
        chat = execution['chat']
        session = chat['sessionId']
        self.settled()
        live_history = self.native(chat)
        require(live_history is not None, 'Original native history unavailable')
        previous = self.close_chat(chat)
        # OMP records a real session_exit on SIGHUP. It belongs to closing the
        # old process, not Resume. Preserve it and baseline after confirmed exit;
        # do not filter messages or relax the no-replay equality below.
        before = self.native(chat)
        require(before['branch'][:len(live_history['branch'])] == live_history['branch'], 'Closing original process rewrote retained Chat history')
        event_count = len(self.events(session))
        model_count = len(self.model_wire.records)
        saved = self.facts()
        run_ids = sorted(row['id'] for row in self.http('/api/workflow/runs')['runs'])
        result = self.http('/api/chats/resume', {'sessionId': session})
        require(result['outcome'] == 'restored', result)
        restored = dict(chat, **result['target'])
        current = until('new OS process actually loads original native Session', lambda: next((e for e in self.events(session)[event_count:]
            if e['event'] == 'session_start'), None), self.args.timeout)
        require(current['pid'] != previous['pid'], 'Resume merely reconnected the old native process')
        self.observe_processes()
        time.sleep(1.25)
        after = self.native(chat)
        require(after['branch'] == before['branch'], 'Resume changed native conversation or replayed launch input')
        require(len(self.model_wire.records) == model_count, 'Resume submitted a model turn without actual user input')
        require(not any(e['event'] == 'before_agent_start' for e in self.events(session)[event_count:]), 'Resume replayed original business/trigger input')
        require(self.facts() == saved, 'Resume mutated execution facts or created a Run')
        require(sorted(r['id'] for r in self.http('/api/workflow/runs')['runs']) == run_ids, 'Resume created a new Run')
        source = next(row for row in json.loads((self.root / 'data/history.json').read_text(encoding='utf-8')) if row['sessionId'] == session)
        require(source['sourceTaskId'] == execution['taskId'], 'Recovered Chat has wrong trusted source Task')
        record = {'sessionId': session, 'sourceTaskId': source['sourceTaskId'], 'parentRunId': parent,
            'executionId': execution['id'], 'childRunId': execution['childRunId'], 'oldProcess': previous,
            'newProcess': current, 'resume': result, 'nativeBeforeExit': live_history, 'nativeBefore': before, 'nativeAfter': after,
            'runIds': run_ids, 'factsBefore': saved, 'factsAfter': self.facts(), 'readonly': readonly}
        self.evidence.setdefault('recoveries', []).append(record)
        execution['chat'] = restored
        if readonly:
            prompt = ('READONLY_DIAGNOSTIC: This is an explicitly authorized rejection diagnostic only, not business retry or takeover. '
                'Call run action=status exactly once, collect {} exactly once, run action=pause exactly once, '
                'run action=recover with nodeId="unused", confirmed=true and reason="historical takeover forbidden" exactly once, '
                'then report body="Historical rejection diagnostic" with can_complete=false exactly once. '
                'All must be refused by the historical backend. Never dispatch, retry or edit facts. Stop with HISTORICAL_REFUSED.')
            marker = 'HISTORICAL_REFUSED'
        else:
            prompt = ('RECOVERY_DIAGNOSTIC: Call run action=status once, collect {} once, and report '
                'body="Recovered original execution diagnostic" with can_complete=false once. '
                'Do not finish, dispatch, retry or write any business file. Stop with RECOVERY_CHECKED.')
            marker = 'RECOVERY_CHECKED'
        offset = len(self.backend_wire.records)
        self.send(restored, prompt, 'read-only-rejection-diagnostic' if readonly else 'recovery-role-diagnostic')
        history = self.await_note(restored, marker)
        self.settled()
        calls = [r for r in self.backend_wire.records[offset:] if (decode(r, 'request') or {}).get('sessionId') == session]
        require(history['branch'][:len(after['branch'])] == after['branch'], 'Diagnostic rewrote retained original Chat history')
        diagnostic_history = dict(history, branch=history['branch'][len(after['branch']):])
        self.verify_roles(calls, diagnostic_history, execution, parent, readonly)
        record['diagnosticCalls'] = calls
        record['nativeDiagnostic'] = history
        if readonly:
            require(self.facts() == saved, 'Native rejected historical operations mutated persisted facts')
        return record

    def verify_roles(self, calls, history, execution, parent, readonly):
        expected_paths = ['/api/workflow/manager/status', '/api/workflow/manager/collect', '/api/workflow/node/report']
        if readonly:
            expected_paths += ['/api/workflow/manager/pause', '/api/workflow/manager/recover']
        require(sorted(r['path'] for r in calls) == sorted(expected_paths), ('Missing/repeated actual native role calls', calls))
        for call in calls:
            request, response = decode(call, 'request') or {}, decode(call, 'response') or {}
            require('runId' not in request and 'executionId' not in request, 'Model supplied role identity instead of native Session injection')
            if readonly:
                require(call.get('status') == 403, ('Historical native tool gained authorization', call))
            elif '/manager/' in call['path']:
                require(response['snapshot']['id'] == execution['childRunId'], 'Recovered manager tool resolved wrong Run')
                require(response['snapshot']['parent'] == {'runId': parent, 'nodeId': 'call', 'executionId': execution['id']}, 'Recovered manager tool resolved wrong parent execution')
            else:
                report = response.get('report', {})
                require(response.get('saved') and report.get('runId') == parent and report.get('executionId') == execution['id']
                    and report.get('from', {}).get('sessionId') == execution['chat']['sessionId']
                    and report.get('to', {}).get('sessionId') == self.parent_session, 'Recovered report routed to wrong execution or recipient')
        native_calls = [part for entry in history['branch'] if entry.get('type') == 'message'
            for part in entry['message'].get('content', []) if isinstance(part, dict) and part.get('type') == 'toolCall']
        for name in ['run', 'collect', 'report']:
            # A guide or assistant mentioning a device is not a native invocation.
            require(any(part.get('name') == name or (part.get('name') == 'write'
                and part.get('arguments', {}).get('path') == 'xd://' + name) for part in native_calls),
                ('Actual native tool missing from persisted history', name))

    def exercise(self):
        zone = self.root / 'zone'
        self.http('/api/zones', {'id': 'recovery', 'path': str(zone)})
        parent_task = self.task('Recovery parent', 'manager',
            'This is an isolated recovery rehearsal. Do not act autonomously. Acknowledge with PARENT_READY and await the next explicit user instruction.', self.roles['parent'])
        child_task = self.task('Recovery child', 'manager',
            'For your initial business instruction only: call run action=status, collect {}, then write the exact bytes RECOVERY-SIDE-EFFECT-731 followed by one newline '
            f'to {zone.as_posix()}/effect-<your actual internal Run id>.txt (substitute the Run id from status). '
            'Report a concise progress body with can_complete=false. Do not finish, dispatch or retry. Stop with CHILD_READY. '
            'On later user turns follow only the new explicit diagnostic, never repeat this initial work.', self.roles['child'])
        self.http('/api/scheduler/bots/save', {'id': 'nested', 'revision': None, 'content': json.dumps({
            'name': 'Recovery nested', 'when': '', 'plan_guide': '', 'node_guide': '', 'map_file': 'inner.json'})})
        worker_task = self.task('Unused recovery worker', 'normal', 'Await explicit delegation only.', self.roles['child'])
        self.http('/api/scheduler/bots/save', {'id': 'unused-worker', 'revision': None, 'content': json.dumps({
            'name': 'Unused recovery worker', 'when': '', 'plan_guide': '', 'node_guide': '', 'task_id': worker_task})})
        for filename, nodes in [('inner.json', {'unused': {'title': 'Pending work, intentionally not dispatched',
            'bot_id': 'unused-worker', 'input': 'Never dispatched during recovery diagnostic', 'zone': 'recovery', 'blocked_by': []}}), ('outer.json', {'call': {'title': 'Recovery call', 'bot_id': 'nested',
            'manager_task_id': child_task, 'input': 'ORIGINAL-NATIVE-BUSINESS-829', 'zone': 'recovery', 'blocked_by': [], 'report_policy': 'auto', 'auto_close': False}})]:
            (self.root / 'maps' / filename).write_text(json.dumps({'title': filename, 'description': '', 'nodes': nodes}), encoding='utf-8')
        started = self.http('/api/scheduler/maps/outer.json/runs', {'managerTaskId': parent_task, 'zone': 'recovery',
            'userInput': 'ROOT-ORIGINAL-TRIGGER-905'}, 201)
        parent = started['id']
        parent_chat = started['run']['manager']
        self.parent_session = parent_chat['sessionId']
        self.chat_hosts[parent_chat['sessionId']] = self.roles['parent']
        self.await_note(parent_chat, 'PARENT_READY')
        self.send(parent_chat, 'START_CALL: Use run action=dispatch nodes=["call"] exactly once, then stop with CALL_STARTED. Do nothing else.', 'initial-parent-dispatch')
        self.await_note(parent_chat, 'CALL_STARTED')
        old = self.detail(parent)['run']['nodes']['call']['execution']
        self.chat_hosts[old['chat']['sessionId']] = self.roles['child']
        self.await_note(old['chat'], 'CHILD_READY')
        old_effect = zone / ('effect-' + old['childRunId'] + '.txt')
        require(old_effect.read_bytes() == b'RECOVERY-SIDE-EFFECT-731\n', 'Original actual side effect differs from literal')
        self.send(parent_chat, 'CONFIRM_FAILURE_AND_RETRY: Original execution has a retained partial file, but the business attempt is now explicitly confirmed failed. '
            'Call run action=fail nodeId="call" reason="Explicit original execution failure after partial side effect" once. '
            'I authorize a NEW attempt: run action=retry nodeId="call" confirmed=true reason="User explicitly authorizes independent new execution" once. '
            'Do not close/delete old Chat or files and do not complete anything. Stop with RETRIED.', 'confirmed-failure-and-retry')
        self.await_note(parent_chat, 'RETRIED')
        node = self.detail(parent)['run']['nodes']['call']
        current = node['execution']
        require(current['id'] != old['id'] and current['childRunId'] != old['childRunId'] and current['chat']['sessionId'] != old['chat']['sessionId'], 'Retry reused original execution/child/Chat')
        require(node['previousExecutions'][0]['execution']['id'] == old['id'], 'Retry discarded original execution')
        parent_calls = [record for record in self.backend_wire.records
            if (decode(record, 'request') or {}).get('sessionId') == self.parent_session]
        for action in ['fail', 'retry']:
            calls = [record for record in parent_calls if record['path'] == '/api/workflow/manager/' + action]
            require(len(calls) == 1 and calls[0].get('status') == 200, ('Failure/retry decision was missing, rejected or repeated', action, calls))
            request = decode(calls[0], 'request')
            require(request['nodeId'] == 'call' and request['reason'].strip(), 'Failure/retry lacked explicit target or evidence')
            if action == 'retry':
                require(request['confirmed'] is True, 'Original side effects were not explicitly confirmed before independent retry')
        self.chat_hosts[current['chat']['sessionId']] = self.roles['child']
        self.await_note(current['chat'], 'CHILD_READY')
        current_effect = zone / ('effect-' + current['childRunId'] + '.txt')
        require(current_effect.read_bytes() == b'RECOVERY-SIDE-EFFECT-731\n' and old_effect.read_bytes() == b'RECOVERY-SIDE-EFFECT-731\n', 'Retry lost actual old or new side effects')
        expected_ids = sorted([parent, old['childRunId'], current['childRunId']])
        require(sorted(row['id'] for row in self.http('/api/workflow/runs')['runs']) == expected_ids,
            'One root and two actual attempts must produce exactly three Runs')
        require(self.detail(parent)['run']['userInput'] == 'ROOT-ORIGINAL-TRIGGER-905', 'Root original input changed')
        for execution in [old, current]:
            child = self.detail(execution['childRunId'])['run']
            require(child['userInput'] == 'ORIGINAL-NATIVE-BUSINESS-829', 'Child original trigger input changed')
            require('ORIGINAL-NATIVE-BUSINESS-829' in json.dumps(self.native(execution['chat']), ensure_ascii=False),
                'Actual native history never received original business input')
        self.evidence['retry'] = {'parent': parent, 'old': json.loads(json.dumps(old)), 'current': json.loads(json.dumps(current)),
            'originalUserInput': 'ROOT-ORIGINAL-TRIGGER-905', 'originalChildInput': 'ORIGINAL-NATIVE-BUSINESS-829',
            'files': [{'path': str(p), 'bytesHex': p.read_bytes().hex()} for p in [old_effect, current_effect]]}
        # Both old and current Sessions really execute both roles after process replacement.
        for execution in [old, current]:
            self.resume_chat(execution, parent)
        self.evidence['verified'].extend(['P29', 'P32'])
        self.settled()
        saved = self.facts()
        backend = self.processes[-1]
        previous_pid = backend.child.pid
        backend.stop()
        require(backend.child.poll() is not None, 'Old backend did not exit')
        self.backend_wire.close()
        self.start_backend(self.root / 'toolkit/extensions/workflow-chat/config.yaml')
        require(self.processes[-1].child.pid != previous_pid, 'Backend restart did not replace process')
        self.backend_wire = WireProxy(self.base, self.root / 'backend-wire-restarted')
        (self.root / 'toolkit/extensions/workflow-chat/config.yaml').write_text(
            f'endpoint: {self.backend_wire.base}\nrequestTimeoutMs: 30000\nreportConfirmationTimeoutMs: 20000\ncollectTimeoutMs: 5460000\n', encoding='utf-8')
        self.evidence['backendRestart'] = {'oldPid': previous_pid, 'oldExit': backend.child.poll(), 'newPid': self.processes[-1].child.pid}
        for execution in [old, current]:
            detail = self.detail(execution['childRunId'])
            require(detail['managed'] is False, 'Restart automatically took over historical child')
            require(detail['run']['parent'] == {'runId': parent, 'nodeId': 'call', 'executionId': execution['id']}, 'Backend restart lost readable parent/child link')
            self.resume_chat(execution, parent, readonly=True)
        require(self.facts() == saved, 'Restart or rejected native takeover rewrote historical facts')
        require(self.detail(parent)['run']['nodes']['call']['execution']['id'] == current['id'], 'Historical takeover changed current execution')
        self.evidence['verified'].append('P33')
        self.evidence['passed'] = True


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ['backend', 'toolkit', 'paths-file', 'model-config', 'evidence-dir']:
        parser.add_argument('--' + name, type=lambda value: Path(value).resolve(), required=True)
    parser.add_argument('--hosts', choices=['omp,pi', 'omp', 'pi', 'declared'], default='omp,pi')
    for name in ['herdr', 'pi-package', 'omp-package', 'pi-state-extension', 'omp-state-extension', 'shell-path']:
        parser.add_argument('--' + name, type=Path)
    parser.add_argument('--node', default='node')
    parser.add_argument('--bun', default='bun')
    parser.add_argument('--timeout', type=int, default=300)
    args = parser.parse_args()
    args.repo = REPO
    require(1 <= args.timeout <= 900, '--timeout must be 1..900')
    args.evidence_dir.mkdir(parents=True, exist_ok=False)
    hosts = args.hosts.split(',')
    evidence = {'passed': False, 'hosts': hosts, 'runs': [], 'cleanupErrors': [],
        'coverage': {key: {'expected': value, 'verified': False} for key, value in EXPECTED.items()}}
    try:
        require(args.backend.is_file(), ('Missing current worktree backend', str(args.backend)))
        require(args.backend == (REPO / backend_suffix()).resolve(), 'Use current worktree binary and its independent CARGO_TARGET_DIR')
        resolve_arguments(parser, args, {'herdr': ('herdr', 'herdr.exe'), 'pi_package': ('pi_package', ''),
            'omp_package': ('omp_package', ''), 'pi_state_extension': ('pi_agent', 'extensions/herdr-agent-state.ts'),
            'omp_state_extension': ('omp_agent', 'extensions/herdr-omp-agent-state.ts')})
        for host in hosts:
            args.host = host
            run = RecoveryAcceptance(args)
            result = run.run()
            evidence['runs'].append({'host': host, 'exit': result, 'evidence': str(run.root / 'evidence.json')})
            evidence['cleanupErrors'].extend(run.evidence['cleanupErrors'])
            # A failing first host does not silently remove the second required host.
        for key in EXPECTED:
            items = []
            for result in evidence['runs']:
                observed = json.loads(Path(result['evidence']).read_text(encoding='utf-8'))
                items.append({'host': result['host'], 'verified': key in observed['verified'] and not observed['cleanupErrors'],
                    'evidence': result['evidence'], 'scenarioExit': result['exit']})
            evidence['coverage'][key]['items'] = items
            evidence['coverage'][key]['verified'] = len(items) == len(hosts) and all(item['verified'] for item in items)
        evidence['passed'] = bool(evidence['runs']) and all(c['verified'] for c in evidence['coverage'].values()) and not evidence['cleanupErrors']
    except BaseException:
        evidence['verificationError'] = traceback.format_exc()
    (args.evidence_dir / 'evidence.json').write_text(json.dumps(evidence, ensure_ascii=False, indent=2), encoding='utf-8')
    print(json.dumps({'passed': evidence['passed'], 'evidence': str(args.evidence_dir / 'evidence.json')}))
    return 0 if evidence['passed'] else 1


if __name__ == '__main__':
    sys.exit(main())
