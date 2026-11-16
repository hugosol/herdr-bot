"""P17/P19/P20/P21/P22/P46: installed OMP native MCP through real Herdr.

Approved seams S2/S3/S4. The native host, discovery and manager remain real;
only the external MCP server and loopback model are controlled. Literal file
bytes and independent wire/start counts distinguish a tool-only bridge, lost
native sources, forced Suite precedence, and a Chat-success/readiness conflation.
"""
import argparse
import ctypes
import hashlib
import json
from pathlib import Path
import shutil
import socket
import sys
import threading
import traceback

from developer_paths import REPO, backend_suffix

from acceptance.suites import skills
from acceptance.host.processes import LiveProcess, require, until


class OwnedProcess(LiveProcess):
    def __init__(self, pid):
        self.pid = pid
        self.kernel = ctypes.WinDLL('kernel32',use_last_error=True)
        self.kernel.OpenProcess.argtypes = [ctypes.c_ulong,ctypes.c_int,ctypes.c_ulong]
        self.kernel.OpenProcess.restype = ctypes.c_void_p
        self.kernel.WaitForSingleObject.argtypes = [ctypes.c_void_p,ctypes.c_ulong]
        self.kernel.CloseHandle.argtypes = [ctypes.c_void_p]
        self.handle = self.kernel.OpenProcess(0x00100000,False,pid)
        self.open_error = 0 if self.handle else ctypes.get_last_error()
        # ERROR_INVALID_PARAMETER for a nonzero owned PID means the process
        # already exited. Access denied and every other failure remain errors.
        require(self.handle or self.open_error == 87, ('Cannot observe owned process',pid,self.open_error))

    def alive(self):
        return bool(self.handle) and super().alive()

    def close(self):
        if self.handle:
            super().close()


class Model(skills.Model):
    def do_POST(self):
        request = json.loads(self.rfile.read(int(self.headers['Content-Length'])))
        case = self.server.case
        case['requests'].append(request)
        delta, finish = {'role':'assistant', 'content':'MCP-VERIFIED-728'}, 'stop'
        try:
            messages = request['messages']
            system = '\n'.join(str(m.get('content', '')) for m in messages if m['role'] in ['system','developer'])
            if not request.get('tools'):
                delta['content'] = '<title>Native MCP acceptance</title>'
            elif not any(m['role'] == 'tool' for m in messages):
                calls = []
                for name in case['expected']:
                    calls.append({'id':name,'type':'function','function':{'name':'write','arguments':json.dumps({
                        'i':'Reading owned MCP file','path':f'xd://mcp__{name}_probe','content':'{}'})}})
                delta = {'role':'assistant','tool_calls':[{'index':i,**call} for i,call in enumerate(calls)]}
                finish = 'tool_calls'
                case['instructionsObserved'] = 'SERVER-INSTRUCTIONS-916' in system
            else:
                outputs = '\n'.join(str(m.get('content','')) for m in messages if m['role']=='tool')
                for literal in case['expected'].values():
                    require(literal + ':FILE-BYTES-473' in outputs, ('Native MCP file result missing',literal,outputs))
                resource_outputs = [m for m in messages if m.get('tool_call_id') == 'owned-resource']
                if case['scenario'] == 'full' and not resource_outputs:
                    delta = {'role':'assistant','tool_calls':[{'index':0,'id':'owned-resource','type':'function',
                        'function':{'name':'read','arguments':json.dumps({'i':'Reading native MCP resource','path':'mcp://owned://refreshed'})}},
                        {'index':1,'id':'owned-refreshed','type':'function','function':{'name':'write',
                         'arguments':json.dumps({'i':'Calling refreshed native tool','path':'xd://mcp__central_refreshed','content':'{}'})}}]}
                    finish = 'tool_calls'
                else:
                    if case['scenario'] == 'full':
                        require('FILE-BYTES-473' in str(resource_outputs), ('MCP resource bytes missing',resource_outputs))
                        refreshed_outputs = [m for m in messages if m.get('tool_call_id') == 'owned-refreshed']
                        require('CENTRAL:FILE-BYTES-473' in str(refreshed_outputs), ('Refreshed native tool failed',refreshed_outputs))
                    case['passed'] = True
        except BaseException:
            case['error'] = traceback.format_exc()
            delta, finish = {'role':'assistant','content':'MCP-ASSERTION-FAILED'}, 'stop'
        self.send_response(200)
        self.send_header('Content-Type','text/event-stream')
        self.end_headers()
        for item, reason in [(delta,None),({},finish)]:
            chunk = {'id':'owned','object':'chat.completion.chunk','created':1,'model':'suite-probe',
                'choices':[{'index':0,'delta':item,'finish_reason':reason}]}
            self.wfile.write(('data: '+json.dumps(chunk)+'\n\n').encode())
        self.wfile.write(b'data: [DONE]\n\n')


class Acceptance(skills.Acceptance):
    def start(self, argv, label):
        if label == 'backend':
            paths = self.root / 'paths.toml'
            paths.write_text(paths.read_text() + 'runtime = ' + json.dumps(str(self.args.node.parent)) + '\n')
        return super().start(argv,label)

    def prepare(self):
        # Reuse the established exclusive HOME/config/server/process lifecycle,
        # not its skill assertions. Empty skills require no directory links.
        scenario = self.args.scenario
        self.args.scenario = 'empty'
        super().prepare()
        self.args.scenario = scenario
        self.evidence['scenario'] = scenario
        self.evidence['promise'] = {'central':['P17'], 'precedence':['P19','P20'],
            'empty':['P21'], 'full':['P22'], 'unreachable':['P46']}[scenario]
        for name in ['backend/src/suites/mcp.rs','scripts/test-real-herdr-suite-mcp.py', 'scripts/acceptance/suites/mcp.py']:
            self.evidence['source']['sha256'][name] = hashlib.sha256((REPO / name).read_bytes()).hexdigest()
        self.model.RequestHandlerClass = Model
        data = self.authority / 'payload.txt'
        data.write_text('FILE-BYTES-473', encoding='utf-8')
        self.fixture = REPO / 'scripts/fixtures/suite-native-mcp.cjs'
        self.evidence['source']['sha256']['scripts/fixtures/suite-native-mcp.cjs'] = hashlib.sha256(self.fixture.read_bytes()).hexdigest()
        def server(identity):
            return {'command':str(self.args.node),'args':[str(self.fixture),str(self.root / f'mcp-{identity}.jsonl'),str(data),identity]}
        central = {'central':server('CENTRAL')}
        central['central'].update(command='${roots.runtime}/' + self.args.node.name, cwd='.')
        central['central']['args'][2] = 'payload.txt'
        project, user = {}, {}
        expected = {'central':'CENTRAL'}
        if scenario in ['precedence','empty','unreachable']:
            project['project'] = server('PROJECT')
            user['user'] = server('USER')
            expected.update(project='PROJECT',user='USER')
        if scenario == 'precedence':
            central.update(shared=server('SHADOW'), alias=server('EQUIVALENT'), distinct=server('DISTINCT'),
                           disabled=server('DISABLED'), denylisted=server('DENYLISTED'))
            project.update(shared=server('WINNER'), equivalent=server('EQUIVALENT'), disabled={'enabled':False,**server('PROJECT-DISABLED')})
            expected.update(shared='WINNER', equivalent='EQUIVALENT', distinct='DISTINCT')
        if scenario == 'empty':
            central = {}
            expected.pop('central')
        if scenario == 'unreachable':
            # Reserve an exclusive dynamic loopback port without listening.
            # Connection refusal is real, no external daemon or racing free port.
            self.unreachable = socket.socket()
            self.unreachable.bind(('127.0.0.1',0))
            central['unreachable'] = {'type':'http','url':f'http://127.0.0.1:{self.unreachable.getsockname()[1]}/mcp'}
        (self.authority / 'mcp.json').write_text(json.dumps({'mcpServers':central}))
        (self.authority / 'suites.json').write_text(json.dumps({'suites':[{
            'id':'selected','name':'MCP acceptance','agents':['omp'],'mcp':list(central)}]}))
        (self.root / 'agent/mcp.json').write_text(json.dumps({'mcpServers':user,
            'disabledServers':['denylisted'] if scenario == 'precedence' else []}))
        zone = self.zones[0]
        shutil.rmtree(zone / '.omp')
        if project:
            (zone / '.omp').mkdir()
            (zone / '.omp/mcp.json').write_text(json.dumps({'mcpServers':project}))
        self.before = {str(p.relative_to(zone)):p.read_bytes() for p in zone.rglob('*') if p.is_file()}
        self.case = {'scenario':scenario,'requests':[],'passed':False,'expected':expected}
        self.evidence['cases'] = [self.case]
        self.model.case = self.case
        self.observer_lock = threading.Lock()
        self.observer_stop = threading.Event()
        self.observer_errors = []
        def watch():
            while not self.observer_stop.wait(0.01):
                try:
                    self.observe_mcp()
                except BaseException:
                    self.observer_errors.append(traceback.format_exc())
                    return
        self.observer = threading.Thread(target=watch,daemon=True)
        self.observer.start()

    def observe_mcp(self):
        if not hasattr(self,'observer_lock'):
            return
        with self.observer_lock:
            for path in self.root.glob('mcp-*.jsonl'):
                for line in path.read_text().splitlines():
                    if not line.endswith('}'):
                        continue  # Writer has not finished this record yet.
                    entry = json.loads(line)
                    if entry.get('event') == 'start' and entry['pid'] not in self.mcp_pids:
                        handle = OwnedProcess(entry['pid'])
                        self.mcp_handles.append(handle)
                        self.mcp_pids.add(entry['pid'])
                        self.evidence.setdefault('processObservations',[]).append({'pid':entry['pid'],
                            'heldHandle':bool(handle.handle),'openError':handle.open_error})
                        if handle.handle:
                            Path(str(path) + '.' + str(entry['pid']) + '.ack').touch()

    def wire(self):
        self.observe_mcp()
        return {p.stem[4:]:[json.loads(line) for line in p.read_text().splitlines()]
                for p in self.root.glob('mcp-*.jsonl')}

    def terminal(self, pane):
        return self.command([self.args.herdr,'pane','read',pane,'--lines','200','--format','text'])

    def exercise(self):
        saved = self.http('/api/tasks', {'name':'Native MCP','chatName':'{task.name}-{chat.id}',
            'agent':'omp','taskType':'normal','model':'probe','thinking':'off','skill':None,
            'prompt':'','suite':'selected'})
        task = saved['tasks'][0]['id']
        self.http('/api/zones', {'id':'owned','path':str(self.zones[0])})
        preview = self.http('/api/chats/preview', {'zone':'owned','taskId':task})
        launch = self.http('/api/chats', {'zone':'owned','taskId':task,'chatId':preview['chatId'],'input':'/mcp help'}, status=202)
        if self.args.scenario != 'unreachable':
            def native_ready():
                current = self.http('/api/chats/' + launch['id'])
                self.case['creation'] = current
                require(current['stage'] != 'failed',current)
                if current['stage'] != 'succeeded':
                    return None
                wire = self.wire()
                identities = set(self.case['expected'].values())
                if not identities.issubset(wire) or not all(any(e.get('method')=='prompts/list' for e in wire[name]) for name in identities):
                    return None
                text = self.terminal(current['paneId'])
                return current if 'Connected to MCP server' in text and 'Suite probe' in text else None
            ready = until('native initialized management Chat',native_ready,seconds=self.args.timeout)
            # Dismiss help/autocomplete and clear any unsubmitted management
            # command before the one actual S4 exercise. Never retry a prompt
            # or synthesize Enter to hide the external startup paste race.
            self.command([self.args.herdr,'agent','send-keys',ready['agentName'],'esc'])
            self.command([self.args.herdr,'agent','send-keys',ready['agentName'],'ctrl+u'])
            self.command([self.args.herdr,'agent','prompt',ready['agentName'],'Read all owned MCP files.'])
        def result():
            self.wire()
            current = self.http('/api/chats/' + launch['id'])
            self.case['creation'] = current
            require(current['stage'] != 'failed',current)
            require(not self.case.get('error'),self.case.get('error'))
            if self.args.scenario == 'unreachable' and current['stage'] == 'succeeded':
                text = self.terminal(current['paneId'])
                if 'Failed: unreachable' in text and 'ECONNREFUSED' in text:
                    self.case['nativeFailure'] = text
                    return current
                return None
            return current if current['stage']=='succeeded' and self.case['passed'] else None
        try:
            current = until('real native MCP file calls',result,seconds=self.args.timeout)
        finally:
            pane = self.case.get('creation',{}).get('paneId')
            if pane:
                self.case['terminal'] = self.terminal(pane)
        if self.args.scenario in ['precedence','empty']:
            self.command([self.args.herdr,'agent','prompt',current['agentName'],'/mcp list'])
            def sources():
                text = self.terminal(current['paneId'])
                if not all(literal in text for literal in ['User level','Project level']):
                    return None
                if self.args.scenario != 'empty' and 'OMP Extension Packages' not in text:
                    return None
                return text
            self.case['nativeSources'] = until('native source and connection status report',sources,seconds=self.args.timeout)
            self.command([self.args.herdr,'agent','send-keys',current['agentName'],'esc'])
        if self.args.scenario == 'full':
            require(self.case.get('instructionsObserved'), 'Native server instructions were not delivered to model')
            def refreshed():
                events = self.wire().get('CENTRAL',[])
                return all(sum(e.get('method') == method for e in events) >= 2
                           for method in ['tools/list','resources/list','prompts/list'])
            until('native notifications refresh all catalogs',refreshed,seconds=self.args.timeout)
            for command, title, literal in [('/mcp list','Configured MCP Servers','OMP Extension Packages'),
                    ('/mcp resources','MCP Resources','owned://refreshed'),
                    ('/mcp prompts','MCP Prompts','Refreshed prompt'),
                    ('/mcp notifications','MCP Notifications','tools/list_changed')]:
                self.command([self.args.herdr,'agent','prompt',current['agentName'],command])
                def report():
                    text = self.terminal(current['paneId'])
                    return text if title in text and literal in text else None
                self.case.setdefault('management',{})[command] = until('native management '+command,report)
                self.command([self.args.herdr,'agent','send-keys',current['agentName'],'esc'])
            self.command([self.args.herdr,'agent','prompt',current['agentName'],'/central:owned-prompt'])
            until('native prompt retrieval',lambda: any(e.get('method')=='prompts/get'
                  for e in self.wire().get('CENTRAL',[])),seconds=self.args.timeout)
            until('native prompt text delivered to model',lambda: any('PROMPT:FILE-BYTES-473' in json.dumps(r['messages'])
                  for r in self.case['requests']),seconds=self.args.timeout)
        wire = self.wire()
        self.case['wire'] = wire
        expected_ids = set(self.case['expected'].values())
        require(set(wire) == expected_ids, ('Native deduplication/disable winners',set(wire),expected_ids))
        for identity, events in wire.items():
            require(sum(e.get('event')=='start' for e in events)==1, ('Extra native connection',identity,events))
            method = 'initialize' if self.args.scenario == 'unreachable' else 'tools/call'
            require(any(e.get('method')==method for e in events), ('Missing native interaction',identity,method))
        require(self.before == {str(p.relative_to(self.zones[0])):p.read_bytes() for p in self.zones[0].rglob('*') if p.is_file()},
                'Launch copied or changed project configuration')
        self.evidence['passed'] = True

    def cleanup_extra(self):
        try:
            self.observe_mcp()
            if hasattr(self,'observer'):
                self.observer_stop.set()
                self.observer.join(timeout=5)
                require(not self.observer.is_alive(), 'MCP process observer did not stop')
            self.evidence['observedMcpPids'] = sorted(self.mcp_pids)
            for handle in self.mcp_handles:
                try:
                    if handle.alive():
                        # A native reconnect racing server teardown can outlive
                        # its parent tree snapshot. Kill only our held process.
                        result = __import__('subprocess').run(['taskkill','/PID',str(handle.pid),'/T','/F'],
                            capture_output=True,text=True,encoding='utf-8',errors='replace',timeout=60)
                        self.evidence.setdefault('mcpCleanup',[]).append({'pid':handle.pid,'exit':result.returncode,
                            'stdout':result.stdout,'stderr':result.stderr})
                    until('owned MCP process exit',lambda: not handle.alive(),seconds=15)
                finally:
                    handle.close()
            require(not getattr(self,'observer_errors',[]),getattr(self,'observer_errors',[]))
        finally:
            if hasattr(self,'unreachable'):
                self.unreachable.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--herdr',type=Path,required=True)
    parser.add_argument('--omp-package',type=Path,required=True)
    parser.add_argument('--backend',type=Path,default=REPO / backend_suffix())
    parser.add_argument('--node',type=Path,default=Path(shutil.which('node') or 'node'))
    parser.add_argument('--timeout',type=int,choices=range(1,301),default=120)
    parser.add_argument('--scenario',choices=['central','precedence','empty','full','unreachable'],default='central')
    args = parser.parse_args()
    args.repo = REPO
    return Acceptance(args).run()

