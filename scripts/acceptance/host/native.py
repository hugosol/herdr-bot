"""Shared real-host acceptance facilities; no workflow scenario or acceptance oracle.

Native history reads use installed SessionManager APIs; role authorization remains
in the shipped extensions and backend. All mutable resources are invocation-owned.
"""
import copy
import ctypes
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import time
import traceback

from developer_paths import REPO

from .isolation import Acceptance
from .processes import require, until, LiveProcess
from .wire import WireProxy, decode
from .model import configure_model, message_text
from .evidence import copy_evidence


def process_arguments(command_line):
    """Use Windows' argv parser; junction aliases are not different installations."""
    shell = ctypes.WinDLL('shell32', use_last_error=True)
    kernel = ctypes.WinDLL('kernel32', use_last_error=True)
    shell.CommandLineToArgvW.argtypes = [ctypes.c_wchar_p, ctypes.POINTER(ctypes.c_int)]
    shell.CommandLineToArgvW.restype = ctypes.POINTER(ctypes.c_wchar_p)
    kernel.LocalFree.argtypes = [ctypes.c_void_p]
    kernel.LocalFree.restype = ctypes.c_void_p
    count = ctypes.c_int()
    pointer = shell.CommandLineToArgvW(command_line, ctypes.byref(count))
    require(pointer, ('Cannot parse actual OS command line', ctypes.get_last_error()))
    try:
        return [pointer[index] for index in range(count.value)]
    finally:
        kernel.LocalFree(pointer)


class NativeAcceptance(Acceptance):
    def __init__(self, args):
        native_args = copy.copy(args)
        native_args.evidence_dir = None
        super().__init__(native_args, prefix='nested-' + args.host + '-', promise='P22', session_prefix='p22-')
        self.args = args
        self.evidence.update(nativeRoot=str(self.root), retainedEvidenceRoot=str(args.evidence_dir / args.host))
        self.env = {key: value for key, value in self.env.items() if not any(term in key.upper() for term in ['SECRET', 'PASSWORD', 'CREDENTIAL', 'PROXY'])}
        self.host = args.host
        self.hosts = ['omp', 'pi'] if self.host == 'declared' else [self.host]
        self.chat_hosts = {}
        self.backend_wire = self.model_wire = None
        self.handles = []
        self.native_cache = {}
        self.evidence.update(host=self.host, nativeProcesses=[], coverage={},
            scope='Real installed host, real model, native identity, public backend authorization; no controlled business provider')

    def run(self):
        result = super().run()
        target = self.args.evidence_dir / self.host
        try:
            # Herdr IPC needs a short native path. Base.run first finishes its
            # cleanup/exit checks and writes evidence; cleanup errors stay failed.
            copy_evidence(self.root, target)
        except BaseException:
            self.evidence['cleanupErrors'].append('Evidence retention failed: ' + traceback.format_exc())
            self.evidence['passed'] = False
            (self.root / 'evidence.json').write_text(json.dumps(self.evidence, ensure_ascii=False, indent=2), encoding='utf-8')
            return 1
        self.root = target
        return result

    def configure_model(self):
        configure_model(self)
        agent = self.root / 'agent'
        if 'omp' in self.hosts:
            shutil.copy2(agent / 'models.json', agent / 'models.yml')
            (agent / 'config.yml').write_text('autoResume: false\nstartup:\n  setupWizard: false\n  showSplash: false\n  checkUpdate: false\nmarketplace:\n  autoUpdate: off\n', encoding='utf-8')

    def role_catalog(self, toolkit):
        catalog = 'skills = []\n' + ''.join(f'[[agents]]\nid = "{host}"\nlabel = "{host}"\n' for host in self.hosts)
        catalog += '[[models]]\nid = "default"\nlabel = "Default"\n[[models]]\nid = "probe"\nlabel = "Real acceptance"\nprovider_model = ' + json.dumps(self.provider_model) + '\nthinking = { ' + ', '.join(host + ' = ["off"]' for host in self.hosts) + ' }\n'
        for host in self.hosts:
            roles = [('manager', toolkit / f'entrypoints/{host}-auto/manager-chat.ts'),
                     ('node', toolkit / f'entrypoints/{host}-auto/node-chat.ts')]
            roles.append(('herdr-state', getattr(self.args, host + '_state_extension')))
            for role, path in roles:
                entry_id = f'{host}-{role}-chat' if role in ['manager', 'node'] else f'{host}-herdr-agent-state'
                catalog += f'[[extensions]]\nid = "{entry_id}"\nlabel = "{role}"\nagent = "{host}"\nkind = "fixed"\nentrypoint = {json.dumps(str(path))}\n'
        return catalog

    def prepare(self):
        require(self.args.backend.is_file(), ('Missing current worktree backend', str(self.args.backend)))
        for name, root, lock in [('herdr-bot', REPO, 'backend/Cargo.lock'), ('toolkit', self.args.toolkit, 'bun.lock')]:
            source = {'root': str(root), 'lockSha256': hashlib.sha256((root / lock).read_bytes()).hexdigest()}
            if name == 'herdr-bot':
                source['frontendLockSha256'] = hashlib.sha256((root / 'frontend/bun.lock').read_bytes()).hexdigest()
            for label, argv in [('revision', ['rev-parse', 'HEAD']), ('status', ['status', '--porcelain']), ('diff', ['diff', '--binary', 'HEAD'])]:
                source[label] = subprocess.run(['git', *argv], cwd=root, capture_output=True, text=True, check=True, encoding='utf-8').stdout
            self.evidence.setdefault('sources', {})[name] = source
        for host in self.hosts:
            package = self.args.omp_package if host == 'omp' else self.args.pi_package
            entry = package / ('dist/cli.js' if host == 'omp' else 'dist/bundle/cli.js')
            require(entry.is_file(), ('Missing installed native host', str(entry)))
            state = getattr(self.args, host + '_state_extension')
            require(state.is_file(), ('Missing installed official native state extension', str(state)))
            self.evidence.setdefault('stateExtensions', {})[host] = {'path': str(state), 'sha256': hashlib.sha256(state.read_bytes()).hexdigest()}
            launcher = shutil.which(host)
            require(launcher, ('Missing installed native launcher on unchanged PATH', host))
            self.evidence.setdefault('installedHosts', {})[host] = {'entry': str(entry), 'entrySha256': hashlib.sha256(entry.read_bytes()).hexdigest(),
                'package': json.loads((package / 'package.json').read_text(encoding='utf-8'))['version'],
                'launcher': launcher, 'launcherSha256': hashlib.sha256(Path(launcher).read_bytes()).hexdigest()}
        self.evidence['runnerSha256'] = hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
        self.evidence['backend'] = {'path': str(self.args.backend), 'sha256': hashlib.sha256(self.args.backend.read_bytes()).hexdigest()}
        super().prepare()
        self.backend_wire = WireProxy(self.base, self.root / 'backend-wire')
        (self.root / 'toolkit/extensions/workflow-chat/config.yaml').write_text(
            f'endpoint: {self.backend_wire.base}\nrequestTimeoutMs: 30000\nreportConfirmationTimeoutMs: 20000\ncollectTimeoutMs: 5460000\n', encoding='utf-8')

    def detail(self, run):
        return self.http('/api/workflow/runs/' + run)

    def native(self, chat):
        files = list((self.root / 'agent').rglob('*.jsonl'))
        for file in files:
            try:
                with file.open(encoding='utf-8') as stream:
                    first = json.loads(stream.readline())
                    if first.get('type') == 'title':
                        first = json.loads(stream.readline())
            except (ValueError, OSError):
                continue
            if first.get('type') == 'session' and first.get('id') == chat['sessionId']:
                self.evidence.setdefault('nativeHeaders', {})[chat['sessionId']] = first
                digest = hashlib.sha256(file.read_bytes()).hexdigest()
                cached = self.native_cache.get(chat['sessionId'])
                if cached and cached[0] == digest:
                    return cached[1]
                host = self.chat_hosts[chat['sessionId']]
                package = self.args.omp_package if host == 'omp' else self.args.pi_package
                module = package / ('src/session/session-manager.ts' if host == 'omp' else 'dist/core/session-manager.js')
                code = 'import {SessionManager} from ' + json.dumps(module.as_uri()) + '; const s=await SessionManager.open(' + json.dumps(str(file)) + '); console.log(JSON.stringify({id:s.getSessionId(),branch:s.getBranch()}));'
                history = json.loads(self.command([self.args.bun if host == 'omp' else self.args.node, '--eval', code] if host == 'omp'
                    else [self.args.node, '--input-type=module', '--eval', code]))
                require(history['id'] == chat['sessionId'], 'Native history identity mismatch')
                target = self.root / 'histories'
                target.mkdir(exist_ok=True)
                shutil.copy2(file, target / file.name)
                self.native_cache[chat['sessionId']] = (digest, history)
                return history
        return None

    def await_note(self, chat, marker):
        def probe():
            history = self.native(chat)
            if not history:
                return None
            entries = [entry['message'] for entry in history['branch'] if entry.get('type') == 'message']
            assistant = [message for message in entries if message.get('role') == 'assistant']
            return history if assistant and marker in message_text(assistant[-1]) and not any(
                part.get('type') == 'toolCall' for part in assistant[-1].get('content', []) if isinstance(part, dict)) else None
        return until('native assistant approval boundary ' + marker, probe, self.args.timeout)

    def send(self, chat, message, kind):
        self.evidence.setdefault('userInputs', []).append({'kind': kind, 'sessionId': chat['sessionId'], 'input': message, 'at': time.monotonic()})
        return self.herdr('agent', 'prompt', chat['paneId'], message)

    def tabs(self, workspace):
        return self.herdr('tab', 'list', '--workspace', workspace)['tabs']

    def observe_processes(self):
        owned = str(self.root).replace("'", "''")
        code = "Get-CimInstance Win32_Process | Where-Object { $_.CommandLine -like '*" + owned + "*' } | Select-Object ProcessId,ParentProcessId,ExecutablePath,CommandLine | ConvertTo-Json -Compress"
        value = json.loads(self.command(['powershell.exe', '-NoProfile', '-Command', code]) or '[]')
        processes = value if isinstance(value, list) else [value]
        for process in processes:
            process['argv'] = process_arguments(process.get('CommandLine') or '')
        self.evidence['nativeProcesses'].extend(processes)
        for host in self.hosts:
            entry = Path(self.evidence['installedHosts'][host]['entry']).resolve()
            require(any(Path(argument).resolve() == entry for process in processes for argument in process['argv']
                if argument.lower().endswith(('.js', '.ts'))), ('No actual installed host OS process under this isolation root', host))
        for process in processes:
            if process.get('ProcessId') == os.getpid() or 'powershell' in str(process.get('ExecutablePath', '')).lower():
                continue
            if process['ProcessId'] in {pid for pid, _ in self.handles}:
                continue
            self.handles.append((process['ProcessId'], LiveProcess(process['ProcessId'])))
        return processes

    def task(self, name, kind, prompt, host):
        result = self.http('/api/tasks', {'name': name, 'taskType': kind, 'agent': host,
            'chatName': '{chat.id}', 'model': 'probe', 'thinking': 'default', 'skill': None, 'prompt': prompt})
        return next(task['id'] for task in result['tasks'] if task['name'] == name)

    def cleanup_extra(self):
        errors = []
        for pid, handle in self.handles:
            try:
                until('owned OS process exits: ' + str(pid), lambda: not handle.alive(), 15)
                self.evidence.setdefault('processExitConfirmations', []).append({'pid': pid, 'exited': True, 'observation': 'held Windows process handle'})
            except BaseException:
                errors.append(traceback.format_exc())
            finally:
                handle.close()
        for wire in [self.backend_wire, self.model_wire]:
            if wire:
                try:
                    wire.close()
                except BaseException:
                    errors.append(traceback.format_exc())
        for name in ['models.json', 'models.yml']:
            (self.root / 'agent' / name).unlink(missing_ok=True)
        require(not errors, errors)
