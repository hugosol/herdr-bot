"""Transparent acceptance traffic observation; no business decisions."""
import errno
import http.server
import json
import secrets
import threading
import time
import traceback
import urllib.error
import urllib.request

from .processes import require


class WireProxy:
    """Transparent loopback observer. Never chooses/replaces a model response.

    Keep bodies, timing and status, not Authorization headers. Each collect gets
    its own thread, so a blocked collect cannot prevent the report that wakes it.
    """
    def __init__(self, upstream, directory, *, serialize_completions=False):
        self.upstream = upstream.rstrip('/')
        self.directory = directory
        directory.mkdir()
        self.records = []
        self.lock = threading.Lock()
        completion_slot = threading.Lock() if serialize_completions else None
        closing = threading.Event()
        self.closing = closing
        owner = self

        class Handler(http.server.BaseHTTPRequestHandler):
            def log_message(self, *_):
                return

            def do_POST(self):
                self.forward()

            def do_GET(self):
                self.forward()

            def do_PUT(self):
                self.forward()

            def forward(self):
                raw = self.rfile.read(int(self.headers.get('Content-Length', '0')))
                with owner.lock:
                    index = len(owner.records)
                    record = {'index': index, 'method': self.command, 'path': self.path,
                              'started': time.monotonic(), 'request': raw.decode('utf-8')}
                    owner.records.append(record)
                slot = completion_slot if (self.command == 'POST' and
                    self.path.split('?', 1)[0].rstrip('/').endswith('/chat/completions')) else None
                if slot is not None:
                    slot.acquire()
                response_body = bytearray()
                try:
                    if slot is not None and closing.is_set():
                        raise RuntimeError('Cell stopped before queued completion reached provider')
                    record['forwarded'] = time.monotonic()
                    request = urllib.request.Request(owner.upstream + self.path, data=raw if raw else None,
                        method=self.command, headers={key: value for key, value in self.headers.items()
                        if key.lower() not in ['host', 'connection', 'content-length', 'accept-encoding']})
                    try:
                        # Manager collect may wait for the backend's 90-minute default.
                        # The observer must not truncate its body before the native caller's budget.
                        collect = self.command == 'POST' and self.path == '/api/workflow/manager/collect'
                        response = urllib.request.urlopen(request, timeout=5460 if collect else 330)
                    except urllib.error.HTTPError as error:
                        response = error
                    with response:
                        record['status'] = response.status
                        self.send_response(response.status)
                        for key in ['Content-Type', 'Cache-Control']:
                            if response.headers.get(key):
                                self.send_header(key, response.headers[key])
                        self.send_header('Connection', 'close')
                        self.end_headers()
                        while True:
                            chunk = response.read1(65536)
                            if not chunk:
                                break
                            response_body.extend(chunk)
                            self.wfile.write(chunk)
                            self.wfile.flush()
                except BaseException:
                    record['error'] = traceback.format_exc()
                    if 'status' not in record:
                        self.send_error(502, 'Actual upstream request failed; see isolated wire evidence')
                finally:
                    if slot is not None:
                        slot.release()
                    record['response'] = response_body.decode('utf-8', errors='replace')
                    record['ended'] = time.monotonic()
                    (owner.directory / f'{index:04d}.json').write_text(
                        json.dumps(record, ensure_ascii=False, indent=2), encoding='utf-8')
                    self.close_connection = True

        # Match the owned completion transport: only actual bind collisions
        # reallocate, before any business request exists.
        for attempt in range(32):
            try:
                self.server = http.server.ThreadingHTTPServer(
                    ('127.0.0.1', 49152 + secrets.randbelow(16384)), Handler)
                break
            except OSError as error:
                if error.errno != errno.EADDRINUSE or attempt == 31:
                    raise
        if serialize_completions:
            self.server.daemon_threads = False  # server_close waits for admitted model calls
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        self.thread.start()
        self.base = f'http://127.0.0.1:{self.server.server_port}'

    def close(self):
        self.closing.set()
        self.server.shutdown()
        self.server.server_close()
        self.thread.join(timeout=15)
        require(not self.thread.is_alive(), 'Wire observer did not stop')
        (self.directory / 'index.json').write_text(json.dumps(self.records, ensure_ascii=False, indent=2), encoding='utf-8')



def decode(record, field):
    try:
        return json.loads(record.get(field, ''))
    except ValueError:
        return None



def messages(record):
    body = decode(record, 'request') or {}
    return body.get('messages', [])



def text(value):
    return json.dumps(value, ensure_ascii=False)

