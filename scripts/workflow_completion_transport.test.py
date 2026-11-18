"""HTTP transport contract, not model/role acceptance.

Approved seam: isolated model HTTP endpoint. An external completion API double
returns one literal decision; the real transport must preserve complete history,
tool definitions, arguments and raw evidence, without inventing tool choices.
"""
import json
from pathlib import Path
import tempfile
import threading
from concurrent.futures import ThreadPoolExecutor
import unittest
import urllib.error
import urllib.request

from acceptance.host.completion_transport import CompletionTransport


class TransportContract(unittest.TestCase):
    def test_scoped_codex_endpoints_keep_local_failure_local_but_share_credential_loss(self):
        from acceptance.host.completion_transport import business_transports, ModelAuthorizationLost
        spent = []

        def complete(request, key):
            spent.append((key, request['prompt']))
            if request['prompt'] == 'bad-case':
                raise ValueError('Rejected case data')
            if request['prompt'] == 'expired':
                raise ModelAuthorizationLost('Test credential expired')
            return {'content': 'OK', 'tool_calls': []}

        def submit(endpoint, prompt):
            payload = json.dumps({'model': 'local-test', 'prompt': prompt}).encode()
            try:
                with urllib.request.urlopen(endpoint.base + '/chat/completions', payload, timeout=3) as response:
                    return response.status
            except urllib.error.HTTPError as error:
                error.close()
                return error.code

        with tempfile.TemporaryDirectory() as directory, business_transports(complete, Path(directory),
                ['omp/v1', 'pi/v1', 'omp/outer'], 1) as cells:
            self.assertEqual(submit(cells['omp/v1'], 'bad-case'), 502)
            self.assertEqual(submit(cells['omp/v1'], 'no-retry'), 502)
            self.assertEqual(submit(cells['pi/v1'], 'independent'), 200)
            self.assertEqual(submit(cells['pi/v1'], 'expired'), 401)
            self.assertEqual(submit(cells['omp/outer'], 'must-not-spend'), 401)
        self.assertEqual(spent, [('omp/v1', 'bad-case'), ('pi/v1', 'independent'), ('pi/v1', 'expired')])

    def test_cell_budget_serializes_overlapping_model_requests_not_queries(self):
        from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
        from acceptance.host.wire import WireProxy
        from time import monotonic

        entered, release = threading.Event(), threading.Event()
        intervals = []

        class Upstream(BaseHTTPRequestHandler):
            def log_message(self, *_):
                pass

            def do_GET(self):
                self.send_response(204)
                self.end_headers()

            def do_POST(self):
                body = self.rfile.read(int(self.headers['Content-Length']))
                start = monotonic()
                if body == b'one':
                    entered.set()
                    release.wait(5)
                intervals.append((body, start, monotonic()))
                self.send_response(200)
                self.end_headers()
                self.wfile.write(body)

        def submit(proxy, body):
            with urllib.request.urlopen(proxy.base + '/v1/chat/completions', body, timeout=8) as response:
                return response.read()

        with tempfile.TemporaryDirectory() as directory, ThreadingHTTPServer(('127.0.0.1', 0), Upstream) as upstream:
            thread = threading.Thread(target=upstream.serve_forever)
            thread.start()
            try:
                proxy = WireProxy('http://127.0.0.1:' + str(upstream.server_port), Path(directory) / 'wire', serialize_completions=True)
                try:
                    with ThreadPoolExecutor(2) as workers:
                        first = workers.submit(submit, proxy, b'one')
                        self.assertTrue(entered.wait(3))
                        second = workers.submit(submit, proxy, b'two')
                        try:
                            # Wait until the proxy has actually admitted BOTH requests, not just until a thread started.
                            from time import sleep
                            deadline = monotonic() + 3
                            while len(proxy.records) < 2 and monotonic() < deadline:
                                sleep(.01)
                            self.assertEqual(len(proxy.records), 2)
                            with urllib.request.urlopen(proxy.base + '/models', timeout=2) as response:
                                self.assertEqual(response.status, 204)
                        finally:
                            release.set()
                        self.assertEqual((first.result(), second.result()), (b'one', b'two'))
                finally:
                    proxy.close()
            finally:
                release.set()
                upstream.shutdown()
                thread.join(5)
        by_body = {body: (start, end) for body, start, end in intervals}
        self.assertLessEqual(by_body[b'one'][1], by_body[b'two'][0])

    def test_context_exit_waits_for_owned_model_call_to_finish(self):
        entered = threading.Event()
        release = threading.Event()
        exited = threading.Event()

        def external_completion(_prompt):
            entered.set()
            if not release.wait(5):
                raise RuntimeError('Test-owned release was not delivered')
            return '{"content":"finished","tool_calls":[]}'

        with tempfile.TemporaryDirectory() as directory:
            server = CompletionTransport(external_completion, Path(directory))
            self.assertGreaterEqual(server.server.server_port, 49152)
            self.assertLessEqual(server.server.server_port, 65535)
            server.__enter__()
            def post():
                request = urllib.request.Request(server.base + '/chat/completions',
                    data=json.dumps({'model': 'actual-model', 'messages': []}).encode())
                with urllib.request.urlopen(request, timeout=10) as response:
                    return json.load(response)['choices'][0]['message']['content']
            def close():
                try:
                    server.__exit__(None, None, None)
                finally:
                    exited.set()
            with ThreadPoolExecutor(max_workers=2) as workers:
                client = workers.submit(post)
                try:
                    self.assertTrue(entered.wait(5), 'Actual model call never began')
                    closing = workers.submit(close)
                    premature = exited.wait(1.25)
                finally:
                    release.set()
                self.assertEqual(client.result(), 'finished')
                closing.result()
                self.assertFalse(premature, 'Transport cleanup returned while its model call was still held')

    def test_serialization_failure_prevents_queued_model_requests(self):
        entered = threading.Event()
        release = threading.Event()
        calls = []

        def external_completion(prompt):
            calls.append(prompt)
            entered.set()
            if not release.wait(5):
                raise RuntimeError('Test-owned release was not delivered')
            return 'not a JSON model decision'

        with tempfile.TemporaryDirectory() as directory:
            with CompletionTransport(external_completion, Path(directory)) as server:
                def post():
                    request = urllib.request.Request(server.base + '/chat/completions',
                        data=json.dumps({'model': 'actual-model', 'messages': []}).encode(),
                        headers={'Content-Type': 'application/json'})
                    try:
                        with urllib.request.urlopen(request, timeout=10) as response:
                            return response.status
                    except urllib.error.HTTPError as error:
                        return error.code

                with ThreadPoolExecutor(max_workers=2) as clients:
                    first = clients.submit(post)
                    try:
                        self.assertTrue(entered.wait(5), 'First actual model call never began')
                        second = clients.submit(post)
                    finally:
                        release.set()
                    self.assertEqual([first.result(), second.result()], [502, 502])
                self.assertEqual(len(calls), 1, 'A terminal model failure must not spend a queued model call')

    def test_full_request_and_model_decision_survive_streaming_transport(self):
        request = {'messages': [{'role': 'system', 'content': 'public guide'},
            {'role': 'user', 'content': 'independent business input'},
            {'role': 'tool', 'tool_call_id': 'prior', 'content': 'prior result'}],
            'tools': [{'type': 'function', 'function': {'name': 'report', 'parameters': {'type': 'object'}}}],
            'stream': True, 'model': 'public-default'}
        observed = []
        raw = '{"content":"partial work","tool_calls":[{"name":"report","arguments":{"can_complete":false,"body":"blocked"}}]}'
        def external_completion(prompt):
            observed.append(prompt)
            return raw
        with tempfile.TemporaryDirectory() as directory:
            with CompletionTransport(external_completion, Path(directory)) as server:
                http = urllib.request.Request(server.base + '/chat/completions',
                    data=json.dumps(request).encode(), headers={'Content-Type': 'application/json'})
                with urllib.request.urlopen(http) as response:
                    stream = response.read().decode()
                chunks = [json.loads(line[6:]) for line in stream.splitlines() if line.startswith('data: {')]
                delta = chunks[0]['choices'][0]['delta']
                self.assertEqual(delta['content'], 'partial work')
                call = delta['tool_calls'][0]
                self.assertEqual(call['function']['name'], 'report')
                self.assertEqual(json.loads(call['function']['arguments']), {'can_complete': False, 'body': 'blocked'})
                self.assertIn('data: [DONE]', stream)
            self.assertIn(json.dumps(request, ensure_ascii=False), observed[0])
            evidence = json.loads((Path(directory) / '0000.json').read_text())
            self.assertEqual(evidence['request'], request)
            self.assertEqual(evidence['completionOutput'], raw)
            self.assertEqual(evidence['mappedMessage']['tool_calls'][0]['function']['name'], 'report')


if __name__ == '__main__':
    unittest.main()
