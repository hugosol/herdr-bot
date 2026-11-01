"""HTTP transport contract, not model/role acceptance.

Approved seam: isolated model HTTP endpoint. An external completion API double
returns one literal decision; the real transport must preserve complete history,
tool definitions, arguments and raw evidence, without inventing tool choices.
"""
import json
from pathlib import Path
import tempfile
import unittest
import urllib.error
import urllib.request

from workflow_completion_transport import CompletionTransport


class TransportContract(unittest.TestCase):
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
