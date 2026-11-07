"""Native Codex CLI auth rejection: no refresh, credential output or fallback.

Approved seam: installed-provider completion CLI, before a live model request.
Only the auth file is a test-owned external resource. The real installed SDK is
used; this is not proof of successful OAuth or model/business behavior.
"""
import base64
import json
import os
import time
from pathlib import Path
import subprocess
import tempfile
import unittest

REPO = Path(__file__).resolve().parents[1]


class CodexAuthSafety(unittest.TestCase):
    def test_native_tool_error_history_and_function_call_survive_real_sdk(self):
        package = Path(os.environ['WORKFLOW_PI_PACKAGE'])
        schema = {'type': 'object', 'properties': {'can_complete': {'type': 'boolean'},
                  'note': {'type': 'string'}}, 'required': ['can_complete', 'note']}
        request = {'model': 'gpt-6-luna', 'store': False, 'stream': True,
            'stream_options': {'include_usage': True}, 'max_completion_tokens': 8192,
            'tools': [{'type': 'function', 'function': {'name': 'report',
                'description': 'Report actual progress', 'parameters': schema, 'strict': False}}],
            'messages': [{'role': 'system', 'content': 'ROLE-SYSTEM-ORACLE'},
                {'role': 'user', 'content': '原始业务输入'},
                {'role': 'assistant', 'content': None, 'tool_calls': [{'id': 'call_prior',
                    'type': 'function', 'function': {'name': 'report',
                        'arguments': '{"can_complete":false,"note":"prior"}'}}]},
                {'role': 'tool', 'tool_call_id': 'call_prior', 'content': 'REJECTED: finish required'},
                {'role': 'user', 'content': '只报告进展，不声称完成'}]}
        with tempfile.TemporaryDirectory(prefix='codex-native-wire-') as directory:
            root = Path(directory)
            auth = root / 'auth.json'
            encoded = base64.urlsafe_b64encode(json.dumps({'https://api.openai.com/auth':
                {'chatgpt_account_id': 'test-owned-account'}}).encode()).decode().rstrip('=')
            original = json.dumps({'openai-codex': {'type': 'oauth', 'access': 'e30.' + encoded + '.test',
                'refresh': 'TEST-OWNED-NOT-A-REAL-REFRESH-TOKEN', 'expires': time.time() * 1000 + 3600000}}).encode()
            auth.write_bytes(original)
            capture = root / 'wire.json'
            preload = root / 'external-provider.mjs'
            preload.write_text('import ' + json.dumps((package / 'dist/core/http-dispatcher.js').as_uri()) + ';\n'
                + 'const capture = ' + json.dumps(str(capture)) + ';\n' + '''
import {writeFileSync} from 'node:fs';
import * as zlib from 'node:zlib';
let calls = 0;
globalThis.fetch = async (url, options) => {
  if (++calls !== 1) throw new Error('Unexpected additional model request');
  const headers = new Headers(options.headers);
  const raw = headers.get('content-encoding') === 'zstd'
    ? zlib.zstdDecompressSync(options.body).toString() : String(options.body);
  writeFileSync(capture, JSON.stringify({url:String(url), body:JSON.parse(raw), calls}));
  const item = {type:'function_call', id:'fc_owned', call_id:'call_owned', name:'report',
    arguments:'{"can_complete":false,"note":"按原文：未完成"}', status:'completed'};
  const response = {id:'resp_owned', object:'response', model:'gpt-6-luna', status:'completed',
    output:[item], usage:{input_tokens:17, output_tokens:9, total_tokens:26,
      input_tokens_details:{cached_tokens:3}, output_tokens_details:{reasoning_tokens:0}}};
  const events = [
    {type:'response.created', response:{...response, status:'in_progress', output:[]}},
    {type:'response.output_item.added', output_index:0, item:{...item, arguments:'', status:'in_progress'}},
    {type:'response.function_call_arguments.delta', output_index:0, item_id:item.id, delta:item.arguments},
    {type:'response.function_call_arguments.done', output_index:0, item_id:item.id, arguments:item.arguments},
    {type:'response.output_item.done', output_index:0, item},
    {type:'response.completed', response}
  ];
  return new Response(events.map(event=>'data: '+JSON.stringify(event)+'\\n\\n').join(''),
    {status:200, headers:{'content-type':'text/event-stream'}});
};
''', encoding='utf-8')
            result = subprocess.run(['node', str(REPO / 'scripts/workflow-codex-completion.mjs'),
                '--pi-package', str(package), '--auth-file', str(auth), '--model', 'gpt-6-luna', '--timeout', '10'],
                input=json.dumps({'request': request}, ensure_ascii=False), text=True, encoding='utf-8',
                capture_output=True, timeout=20, cwd=REPO,
                env=dict(os.environ, NODE_OPTIONS='--import=' + preload.as_uri()))
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            native = json.loads(result.stdout)
            self.assertEqual(native['tool_calls'], [{'id': 'call_owned|fc_owned', 'name': 'report',
                'arguments': {'can_complete': False, 'note': '按原文：未完成'}}])
            self.assertEqual(native['stopReason'], 'toolUse')
            wire = json.loads(capture.read_text(encoding='utf-8'))
            self.assertEqual(wire['calls'], 1)
            self.assertEqual(wire['body']['instructions'], 'ROLE-SYSTEM-ORACLE')
            declared = wire['body'].get('tools', []) + [tool for item in wire['body']['input']
                if item.get('type') == 'additional_tools' for tool in item['tools']]
            self.assertEqual([(tool['name'], tool['parameters'], tool.get('strict')) for tool in declared],
                [('report', schema, False)])
            prior = [item for item in wire['body']['input'] if item.get('type') == 'function_call']
            outputs = [item for item in wire['body']['input'] if item.get('type') == 'function_call_output']
            self.assertEqual([(item['call_id'], item['name'], json.loads(item['arguments'])) for item in prior],
                [('call_prior', 'report', {'can_complete': False, 'note': 'prior'})])
            self.assertEqual([(item['call_id'], item['output']) for item in outputs],
                [('call_prior', 'REJECTED: finish required')])
            self.assertEqual(auth.read_bytes(), original)

    def test_expired_oauth_is_rejected_without_mutating_or_exposing_credentials(self):
        package = os.environ['WORKFLOW_PI_PACKAGE']
        with tempfile.TemporaryDirectory(prefix='codex-expired-auth-') as directory:
            auth = Path(directory) / 'auth.json'
            original = json.dumps({'openai-codex': {'type': 'oauth', 'access': 'SECRET-ACCESS-DO-NOT-PRINT',
                'refresh': 'SECRET-REFRESH-DO-NOT-PRINT', 'expires': 1}}).encode()
            auth.write_bytes(original)
            result = subprocess.run(['node', str(REPO / 'scripts/workflow-codex-completion.mjs'),
                '--pi-package', package, '--auth-file', str(auth), '--model', 'gpt-6-luna', '--timeout', '5'],
                input=json.dumps({'request': {'model': 'gpt-6-luna', 'messages': [
                    {'role': 'user', 'content': 'Must never reach a live service.'}]}}),
                text=True, capture_output=True, timeout=15, cwd=REPO)
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(json.loads(result.stdout), {'error': 'oauth-expired'})
            self.assertNotIn('SECRET-ACCESS', result.stdout + result.stderr)
            self.assertNotIn('SECRET-REFRESH', result.stdout + result.stderr)
            self.assertEqual(auth.read_bytes(), original)


if __name__ == '__main__':
    unittest.main()
