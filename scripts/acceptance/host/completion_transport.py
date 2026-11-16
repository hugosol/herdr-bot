"""Acceptance HTTP transports for actual text or native model decisions.

CompletionTransport accepts completion(prompt) -> actual model text;
NativeCompletionTransport accepts completion(request) -> SDK-native decision.
Neither chooses business actions or executes tools: the real host owns those
and its history. Request serialization is not direct-host wire compliance.
"""
import http.server
import json
from pathlib import Path
import threading
import time
import traceback
import uuid


PROTOCOL = '''You are the assistant in the native conversation serialized below.
Continue that conversation, following its system instructions and user request.
The complete actual conversation and available tools are supplied verbatim as a
JSON request. Tool results are observations from tools already executed by the
host. Decide your next response yourself. Do not simulate tool results.
Return only one JSON object with keys content (string, possibly empty) and
 tool_calls (array, possibly empty). Each tool call has name (exact available
function name) and arguments (JSON object matching its schema). When calling
tools the host will actually execute them and send their results next turn.
Do not wrap JSON in Markdown. This instruction only defines serialization;
it adds no business decisions. Actual native request follows:\n'''


class CompletionTransport:
    def decision(self, request, record):
        prompt = PROTOCOL + json.dumps(request, ensure_ascii=False)
        record['completionPrompt'] = prompt
        raw = self.complete(prompt)
        record['completionOutput'] = raw
        return json.loads(raw)

    def __init__(self, complete, directory):
        self.complete = complete
        self.directory = Path(directory)
        self.directory.mkdir(parents=True, exist_ok=True)
        self.records = []
        self.lock = threading.Lock()
        self.failed = False
        owner = self

        class Handler(http.server.BaseHTTPRequestHandler):
            def log_message(self, *_):
                pass

            def do_POST(self):
                # A queued host retry must observe failure only after the prior
                # model decision and its serialization have both settled.
                with owner.lock:
                    self.respond()

            def respond(self):
                record = {'index': len(owner.records), 'started': time.monotonic(), 'path': self.path}
                owner.records.append(record)
                try:
                    if owner.failed:
                        raise RuntimeError('Earlier completion failure is terminal; no additional model call')
                    if self.path != '/chat/completions':
                        raise ValueError('Only /chat/completions is supported')
                    request = json.loads(self.rfile.read(int(self.headers['Content-Length'])))
                    record['request'] = request
                    decision = owner.decision(request, record)
                    if not isinstance(decision.get('content'), str) or not isinstance(decision.get('tool_calls'), list):
                        raise ValueError('Completion must return content string and tool_calls array')
                    calls = []
                    for call in decision['tool_calls']:
                        if not isinstance(call.get('name'), str) or not isinstance(call.get('arguments'), dict):
                            raise ValueError('Completion tool call requires name string and arguments object')
                        # Do not filter/coerce/validate tool arguments here: native host owns it.
                        call_id = call.get('id')
                        if call_id is not None and (not isinstance(call_id, str) or not call_id):
                            raise ValueError('Native tool call identity must be a nonempty string')
                        calls.append({'id': call_id or 'call_' + uuid.uuid4().hex, 'type': 'function',
                            'function': {'name': call['name'], 'arguments': json.dumps(call['arguments'], ensure_ascii=False)}})
                    message = {'role': 'assistant', 'content': decision['content']}
                    if calls:
                        message['tool_calls'] = calls
                    record['mappedMessage'] = message
                    common = {'id': 'chatcmpl-' + uuid.uuid4().hex, 'created': int(time.time()), 'model': request['model']}
                    reason = 'tool_calls' if calls else 'stop'
                    if request.get('stream'):
                        delta = dict(message)
                        if calls:
                            delta['tool_calls'] = [dict(call, index=index) for index, call in enumerate(calls)]
                        frames = [dict(common, object='chat.completion.chunk', choices=[{'index': 0, 'delta': delta, 'finish_reason': None}]),
                            dict(common, object='chat.completion.chunk', choices=[{'index': 0, 'delta': {}, 'finish_reason': reason}])]
                        if isinstance(decision.get('usage'), dict):
                            frames[-1]['usage'] = decision['usage']
                        payload = ''.join('data: ' + json.dumps(frame, ensure_ascii=False) + '\n\n' for frame in frames) + 'data: [DONE]\n\n'
                        content_type = 'text/event-stream'
                    else:
                        response = dict(common, object='chat.completion', choices=[{'index': 0, 'message': message, 'finish_reason': reason}])
                        if isinstance(decision.get('usage'), dict):
                            response['usage'] = decision['usage']
                        payload = json.dumps(response)
                        content_type = 'application/json'
                    encoded = payload.encode('utf-8')
                    self.send_response(200)
                    self.send_header('Content-Type', content_type)
                    self.send_header('Content-Length', str(len(encoded)))
                    self.end_headers()
                    self.wfile.write(encoded)
                except BaseException:
                    owner.failed = True
                    record['error'] = traceback.format_exc()
                    self.send_error(502, 'Actual completion or transport failed; see raw evidence')
                finally:
                    record['ended'] = time.monotonic()
                    (owner.directory / f"{record['index']:04d}.json").write_text(
                        json.dumps(record, ensure_ascii=False, indent=2), encoding='utf-8')

        self.server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Handler)
        # server_close joins owned handlers; daemon handlers could leave the
        # provider subprocess running after evidence/context cleanup returned.
        self.server.daemon_threads = False
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        self.base = f'http://127.0.0.1:{self.server.server_port}'

    def __enter__(self):
        self.thread.start()
        return self

    def __exit__(self, *_):
        self.server.shutdown()
        self.server.server_close()
        self.thread.join(timeout=15)
        if self.thread.is_alive():
            raise RuntimeError('Completion transport did not stop')


class NativeCompletionTransport(CompletionTransport):
    """The provider returns actual tool calls; no model-written JSON envelope."""
    def decision(self, request, record):
        decision = self.complete(request)
        record['nativeCompletion'] = decision
        return decision
