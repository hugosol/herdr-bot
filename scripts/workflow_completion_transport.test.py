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
