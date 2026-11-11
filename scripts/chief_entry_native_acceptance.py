"""P7 real installed OMP/Pi, actual model, public launch/new-process resume.

Independent oracle: a literal Bot.when unavailable in the user input must appear
in a real chief_definitions result and the actual model's answer. A second Bot
is published only after the first native process exits, so retained history or
an unloaded resumed role cannot satisfy the resumed assertion. No tool decision
or result is supplied by this driver. S3/native histories and S2 remain real.
"""
import hashlib
import json
from pathlib import Path
import runpy
import tomllib
import time

shared = runpy.run_path(str(Path(__file__).with_name('workflow_native_acceptance.py')))
NativeAcceptance, require, until = shared['NativeAcceptance'], shared['require'], shared['until']


class ChiefEntryAcceptance(NativeAcceptance):
    def __init__(self, args):
        super().__init__(args)
        self.evidence.update(promise=None, promises=['P7'], scenario='chief-entry',
            coverage={'P7': {'verified': False}}, phases=[], modelIdentities=[],
            scenarioSource={'path': str(Path(__file__).resolve()), 'sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest()})
        self.env['OMP_CODING_AGENT_DIR'] = str(self.root / 'agent')
        self.env['XDG_DATA_HOME'] = str(self.root / 'state')
        self.evidence['isolation'].update(OMP_CODING_AGENT_DIR=self.env['OMP_CODING_AGENT_DIR'], XDG_DATA_HOME=self.env['XDG_DATA_HOME'])

    def role_catalog(self, toolkit):
        # Consume shipped fixed entries rather than inventing a parallel role list.
        source = self.args.repo / 'meta-data/task-options.toml'
        metadata = tomllib.loads(source.read_text(encoding='utf-8'))
        roots = dict(self.args.path_roots, toolkit=str(toolkit))
        catalog = 'skills = []\n'
        catalog += f'[[agents]]\nid = "{self.host}"\nlabel = "{self.host}"\n'
        catalog += '[[models]]\nid = "default"\nlabel = "Default"\n[[models]]\nid = "probe"\nlabel = "GPT-6-luna acceptance"\nprovider_model = ' + json.dumps(self.provider_model) + f'\nthinking = {{ {self.host} = ["off"] }}\n'
        entries = [entry for entry in metadata['extensions'] if entry['agent'] == self.host]
        require(any(entry['id'] == self.host + '-chief-chat' for entry in entries), 'Shipped metadata does not declare chief fixed entry')
        for entry in entries:
            path = entry['entrypoint']
            for name, value in roots.items():
                path = path.replace('${roots.' + name + '}', str(value))
            if entry['id'] == self.host + '-herdr-agent-state':
                path = str(getattr(self.args, self.host + '_state_extension'))
            require('${' not in path, ('Unresolved shipped fixed extension root', entry['id']))
            require(Path(path).is_file() or Path(path).is_dir(), ('Missing actual fixed extension', entry['id'], path))
            catalog += '\n[[extensions]]\n' + ''.join(key + ' = ' + json.dumps(value) + '\n' for key, value in {**entry, 'entrypoint': path}.items())
        self.evidence['fixedMetadata'] = {'source': str(source), 'sha256': hashlib.sha256(source.read_bytes()).hexdigest(), 'entries': entries}
        return catalog

    def configure_model(self):
        config = json.loads(self.args.model_config.read_text(encoding='utf-8'))
        require(config['model'] == 'gpt-6-luna', 'GPT-6-luna is required; no fallback')
        super().configure_model()

    def prepare(self):
        super().prepare()
        # Auto connect uses its normal safe configuration, only in the owned copy.
        config = self.root / 'toolkit/extensions/connect-bot/config.yaml'
        content = config.read_text(encoding='utf-8')
        lines = content.splitlines()
        require(any(line.startswith('endpoint:') for line in lines), 'Missing shipped connect-bot endpoint setting')
        config.write_text('\n'.join('endpoint: ' + self.base if line.startswith('endpoint:') else line for line in lines) + '\n', encoding='utf-8')
        cost = self.root / 'toolkit/extensions/model-cost/config.yaml'
        cost.write_text('\n'.join('storageDir: ' + json.dumps(str(self.root / 'cost-archive')) if line.startswith('storageDir:') else line
            for line in cost.read_text(encoding='utf-8').splitlines()) + '\n', encoding='utf-8')
        framework = self.root / 'toolkit/framework/config.yaml'
        framework.write_text(framework.read_text(encoding='utf-8').replace('      enabled: true', '      enabled: false'), encoding='utf-8')
        self.evidence['ownedConfigurationOverrides'] = {str(path.relative_to(self.root)): path.read_text(encoding='utf-8')
            for path in [config, cost, framework]}

    def publish_bot(self, name, literal):
        target = self.task('target-' + name, 'normal', 'Acceptance-only unstarted business target', self.host)
        body = {'name': name, 'when': literal, 'plan_guide': 'Read-only acceptance; do not execute.',
                'node_guide': 'No business execution authorized', 'task_id': target}
        result = self.http('/api/scheduler/bots/save', {'id': name, 'content': json.dumps(body), 'revision': None})
        self.evidence.setdefault('publishedBots', []).append(result)

    def native_pids(self):
        processes = self.observe_processes()
        entry = Path(self.evidence['installedHosts'][self.host]['entry']).resolve()
        return {p['ProcessId'] for p in processes if any(Path(arg).resolve() == entry for arg in p['argv'] if arg.lower().endswith(('.js', '.ts')))}

    def check_model_identity(self, start):
        records = self.model_wire.records[start:]
        require(records, 'No actual model request observed')
        identities = []
        for record in records:
            require(record.get('ended') and record.get('status') == 200 and not record.get('error'), ('Actual model request failed; no retry', record))
            body = json.loads(record['request'])
            require(body.get('model') == 'gpt-6-luna', ('Requested model mismatch', body.get('model')))
            raw = record['response']
            packets = []
            if raw.lstrip().startswith('{'):
                packets.append(json.loads(raw))
            else:
                for line in raw.splitlines():
                    if line.startswith('data:') and line[5:].strip() != '[DONE]':
                        packets.append(json.loads(line[5:].strip()))
            actual = {packet['model'] for packet in packets if packet.get('model')}
            require(actual == {'gpt-6-luna'}, ('Actual returned model unavailable or identity mismatch; no fallback', sorted(actual)))
            identities.append({'request': record['index'], 'requested': body['model'], 'returned': sorted(actual)})
        self.evidence['modelIdentities'].extend(identities)
        return records

    def cleanup_extra(self):
        # Preserve returned identities even when an earlier behavioral assertion
        # failed; cleanup failure must never erase that original failure.
        try:
            super().cleanup_extra()
        finally:
            if self.model_wire and self.model_wire.records:
                self.evidence['modelIdentities'] = []
                self.check_model_identity(0)

    def consume(self, chat, literal, label, wire_start, branch_start=0):
        def complete():
            history = self.native(chat)
            if not history:
                return None
            messages = [entry['message'] for entry in history['branch'][branch_start:] if entry.get('type') == 'message']
            replies = [message for message in messages if message.get('role') == 'assistant']
            if not replies:
                return None
            last = replies[-1]
            parts = last.get('content', [])
            if any(isinstance(part, dict) and part.get('type') == 'toolCall' for part in parts):
                return None
            return history if literal in json.dumps(last, ensure_ascii=False) else None
        history = until('actual chief capability answer ' + label, complete, self.args.timeout)
        messages = [entry['message'] for entry in history['branch'][branch_start:] if entry.get('type') == 'message']
        calls = [part for message in messages if message.get('role') == 'assistant' for part in message.get('content', [])
                 if isinstance(part, dict) and part.get('type') == 'toolCall']
        results = [message for message in messages if message.get('role') == 'toolResult']
        require(any('chief_definitions' in str(call) for call in calls), 'No real native chief definition call in this phase')
        require(any(literal in json.dumps(result, ensure_ascii=False) and not result.get('isError') for result in results), 'No actual literal definition tool result')
        records = self.check_model_identity(wire_start)
        requests = '\n'.join(record['request'] for record in records)
        require('chief-chat:tools' in requests and '不要亲自' in requests, 'Actual model request did not consume shipped chief role guidance')
        self.evidence['phases'].append({'phase': label, 'sessionId': chat['sessionId'], 'history': history,
            'nativeCalls': calls, 'nativeResults': results, 'modelRequestIndices': [r['index'] for r in records], 'verified': True})
        return history

    def exercise(self):
        self.http('/api/zones', {'id': 'chief/owned', 'path': str(self.root / 'zone')})
        self.publish_bot('entry-launch', 'CHIEF-ENTRY-LAUNCH-WHEN-731')
        task = self.task('entry-chief', 'chief', '', self.host)
        definition = self.http('/api/tasks/' + task)['task']
        require(definition.get('suite') is None, 'P7 requires no Suite')
        preview = self.http('/api/chats/preview', {'zone': 'chief/owned', 'taskId': task})
        wire_start = len(self.model_wire.records)
        user = '请查看当前可复用能力目录，逐字引用每项 when，并说明你在本次只读查询中的角色边界。不执行这些能力，不编辑文件，不创建能力；回答后等待用户。'
        started = self.http('/api/chats', {'zone': 'chief/owned', 'taskId': task, 'chatId': preview['chatId'], 'input': user}, 202)
        def ready():
            state = self.http('/api/chats/' + started['id'])
            require(state['stage'] != 'failed', state)
            if state['stage'] != 'succeeded':
                return None
            current = self.http('/api/chief')['current']
            return {**state, 'sessionId': current['sessionId']} if current and current.get('sessionId') else None
        created = until('public chief launch completion and native identity', ready, self.args.timeout)
        chat = {**created, 'sessionId': created['sessionId']}
        self.chat_hosts[chat['sessionId']] = self.host
        first_pids = self.native_pids()
        first = self.consume(chat, 'CHIEF-ENTRY-LAUNCH-WHEN-731', 'launch', wire_start)
        self.http('/api/chief/qualify', {'sessionId': chat['sessionId']})
        self.herdr('pane', 'close', chat['paneId'])
        for pid, handle in self.handles:
            if pid in first_pids:
                until('owned native process exited before new-process resume', lambda: not handle.alive(), 20)
                self.evidence.setdefault('preResumeExitConfirmations', []).append({'pid': pid, 'exited': True})
        # Explicit unregister concerns backend registration only; process exit was
        # independently established using its held OS process handle above.
        current = self.http('/api/chief')['current']
        if current is not None:
            require(current.get('sessionId') == chat['sessionId'], 'Unexpected replacement chief in owned instance')
            self.http('/api/sessions/events', {'sessionId': chat['sessionId'], 'kind': 'unregister'})
        self.publish_bot('entry-resume', 'CHIEF-ENTRY-RESUME-WHEN-926')
        resumed = self.http('/api/chats/resume', {'sessionId': chat['sessionId']})
        require(resumed['outcome'] == 'restored', ('Expected real new-process recovery, not focus', resumed))
        target = {**resumed['target'], 'sessionId': chat['sessionId']}
        second_pids = self.native_pids()
        require(second_pids and not first_pids.intersection(second_pids), 'Resume did not create a different native process')
        wire_start = len(self.model_wire.records)
        self.send(target, user, 'explicit-post-resume-readonly-user-input')
        second = self.consume(target, 'CHIEF-ENTRY-RESUME-WHEN-926', 'resume', wire_start, len(first['branch']))
        require(first['id'] == second['id'] == chat['sessionId'], 'Resume changed native conversation identity')
        self.http('/api/chief/qualify', {'sessionId': chat['sessionId']})
        self.evidence['coverage']['P7'] = {'verified': True, 'noSuite': definition.get('suite') is None,
            'firstPids': sorted(first_pids), 'resumedPids': sorted(second_pids), 'definition': definition}
        self.evidence['passed'] = True
