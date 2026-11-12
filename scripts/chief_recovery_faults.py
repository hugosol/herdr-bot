"""Acceptance-only HTTP observer: hold/drop the real post-commit response.

No replacement backend result and no product retry/wait policy. A gate has a
bounded driver budget; cleanup releases every waiter before awaiting workers.
"""
import http.server
import json
import threading
import time
import traceback
import urllib.error
import urllib.request


class Gate:
    def __init__(self, drop):
        self.drop = drop
        self.committed = threading.Event()
        self.release = threading.Event()
        self.record = None


class RecoveryFaultProxy:
    def __init__(self, upstream, directory):
        self.upstream = upstream
        self.directory = directory
        directory.mkdir(exist_ok=True)
        self.records = []
        self.lock = threading.Lock()
        self.gates = {}
        self.all_gates = []
        self.pending = []
        owner = self

        class Handler(http.server.BaseHTTPRequestHandler):
            def log_message(self, *_):
                pass
            def do_POST(self):
                self.forward()
            def do_GET(self):
                self.forward()
            def do_PUT(self):
                self.forward()
            def forward(self):
                raw = self.rfile.read(int(self.headers.get('Content-Length', '0')))
                with owner.lock:
                    record = {'index': len(owner.records), 'method': self.command, 'path': self.path,
                              'request': raw.decode(), 'started': time.monotonic()}
                    owner.records.append(record)
                    completed = threading.Event()
                    owner.pending.append(completed)
                    gate = owner.gates.pop(self.path, None)
                try:
                    request = urllib.request.Request(owner.upstream + self.path, data=raw if raw else None,
                        method=self.command, headers={key: value for key, value in self.headers.items()
                            if key.lower() not in ['host', 'connection', 'content-length', 'accept-encoding']})
                    try:
                        response = urllib.request.urlopen(request, timeout=100)
                    except urllib.error.HTTPError as error:
                        response = error
                    with response:
                        body = response.read()
                        record.update(status=response.status, response=body.decode(), upstreamEnded=time.monotonic())
                        if gate:
                            gate.record = record
                            gate.committed.set()
                            if not gate.release.wait(60):
                                raise TimeoutError('Acceptance driver did not release postcommit gate within 60s')
                            if gate.drop:
                                record['injectedFault'] = 'drop-real-postcommit-response'
                                return
                        self.send_response(response.status)
                        self.send_header('Content-Type', response.headers.get('Content-Type', 'application/json'))
                        self.send_header('Connection', 'close')
                        self.end_headers()
                        self.wfile.write(body)
                except (BrokenPipeError, ConnectionResetError):
                    record['callerDisconnected'] = True
                except BaseException:
                    record['error'] = traceback.format_exc()
                    self.send_error(502)
                finally:
                    record['ended'] = time.monotonic()
                    try:
                        (owner.directory / f"{record['index']:04d}.json").write_text(json.dumps(record, ensure_ascii=False, indent=2), encoding='utf-8')
                    finally:
                        self.close_connection = True
                        completed.set()
        self.server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Handler)
        self.thread = threading.Thread(target=self.server.serve_forever)
        self.thread.start()
        self.base = f'http://127.0.0.1:{self.server.server_port}'

    def arm(self, path, drop=False):
        with self.lock:
            if path in self.gates:
                raise ValueError('Gate already armed: ' + path)
            gate = Gate(drop)
            self.gates[path] = gate
            self.all_gates.append(gate)
            return gate

    def close(self):
        for gate in self.all_gates:
            gate.release.set()
        self.server.shutdown()
        deadline = time.monotonic() + 15
        settled = all(event.wait(max(0, deadline - time.monotonic())) for event in self.pending)
        self.server.server_close()
        self.thread.join(max(0, deadline - time.monotonic()))
        if self.thread.is_alive() or not settled:
            raise RuntimeError('Recovery fault server or owned HTTP worker did not exit')
        (self.directory / 'index.json').write_text(json.dumps(self.records, ensure_ascii=False, indent=2), encoding='utf-8')
