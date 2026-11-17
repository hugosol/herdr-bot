"""Native Codex CLI auth rejection: no refresh, credential output or fallback.

Approved seam: installed-provider completion CLI, before a live model request.
CodexAuthSafety uses the real installed SDK with test-owned auth/provider data.
CodexContentDiagnostics supplies literal SDK returns through a test-only import
hook and forbids fetch; it is not provider, OAuth or model/business acceptance.
"""
import base64
import copy
import importlib.util
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


class CodexContentDiagnostics(unittest.TestCase):
    """Literal SDK-return fixtures only: no provider, HTTP or actual credentials."""

    def test_literal_sdk_rejections_and_unchanged_successes(self):
        package = Path(os.environ['WORKFLOW_PI_PACKAGE']).resolve()
        secret = 'SECRET-MUST-NOT-APPEAR-IN-DIAGNOSTICS'
        base = {'provider': 'openai-codex', 'model': 'gpt-6-luna', 'api': 'openai-codex-responses',
                'stopReason': 'stop', 'content': [{'type': 'text', 'text': 'Literal response'}],
                'usage': {'input': 2, 'output': 1, 'cacheRead': 0, 'cacheWrite': 0, 'totalTokens': 3}}
        tool = {'type': 'toolCall', 'id': 'call_owned', 'name': 'report', 'arguments': {}}
        cases = []

        def case(name, changes=None, constraint=None, expected=None, **flags):
            result = copy.deepcopy(base)
            result.update(copy.deepcopy(changes or {}))
            cases.append((name, {'result': result, **flags}, constraint, expected or {}))

        case('valid-text')
        case('valid-tool', {'stopReason': 'toolUse', 'content': [tool]})
        case('missing-completion', constraint='sdk-content', expected={'completed': False}, completed=False)
        case('unknown-stop-reason', {'stopReason': secret}, 'sdk-content', {'stopReason': 'other'})
        case('content-not-array', {'content': {'private': secret}}, 'sdk-content', {'contentArray': False})
        case('unknown-content-block', {'content': [{'type': secret}]}, 'sdk-content', {'invalidBlockCount': 1})
        case('invalid-text', {'content': [{'type': 'text', 'text': {'private': secret}}]},
             'sdk-content', {'invalidTextCount': 1})
        for name, changed, expected in [
            ('invalid-tool-id', {'id': 'invalid ' + secret}, {'invalidToolIds': 1}),
            ('invalid-tool-name', {'name': ''}, {'invalidToolNames': 1}),
            ('invalid-arguments', {'arguments': [secret]}, {'invalidArguments': 1}),
            ('arguments-error', {'argumentsError': secret}, {'argumentsErrorCount': 1}),
            ('partial-json', {'partialJson': secret}, {'partialJsonCount': 1}),
            ('namespace', {'namespace': secret}, {'namespaceCount': 1}),
        ]:
            case(name, {'stopReason': 'toolUse', 'content': [{**tool, **changed}]}, 'sdk-content', expected)
        case('tool-stop-mismatch', {'content': [tool]}, 'decision-consistency', {'toolUseMatches': False})
        case('duplicate-tool-id', {'stopReason': 'toolUse', 'content': [tool, tool]},
             'decision-consistency', {'duplicateToolIds': 1})
        case('empty-decision', {'content': []}, 'decision-consistency', {'hasText': False, 'toolCalls': 0})
        case('invalid-usage', {'usage': {**base['usage'], 'input': -1}}, 'usage', {'usageInvalidMask': 1})
        case('missing-usage', {'usage': None}, 'usage', {'usageInvalidMask': 31})
        case('nonfinite-usage', constraint='usage', expected={'usageInvalidMask': 2}, nonfinite=True)
        case('delivery-cap', {'usage': {**base['usage'], 'output': 9}},
             'delivery-cap', {'outputTokens': 9, 'deliveryCap': 8})
        case('serialization', constraint='serialization', circular=True)

        with tempfile.TemporaryDirectory(prefix='codex-literal-sdk-') as directory:
            root = Path(directory)
            auth = root / 'auth.json'
            original = json.dumps({'openai-codex': {'type': 'oauth', 'access': secret,
                'refresh': secret, 'expires': time.time() * 1000 + 3600000}}).encode()
            auth.write_bytes(original)
            fixture = root / 'sdk-result.json'
            preload = root / 'literal-sdk.mjs'
            adapter = REPO / 'scripts/workflow-codex-completion.mjs'
            # Intercept only this adapter's models import, not SDK dependencies.
            # The CLI's input/auth validation remains real; complete returns one
            # literal SDK result and never invokes a provider or its fetch hook.
            preload.write_text('import {registerHooks, findPackageJSON} from "node:module";\n'
                'import {dirname, join} from "node:path";\n'
                'import {pathToFileURL} from "node:url";\n'
                + 'const packageFile = ' + json.dumps((package / 'package.json').as_uri()) + ';\n'
                + 'const adapter = ' + json.dumps(adapter.as_uri()) + ';\n'
                + 'const fixture = ' + json.dumps(str(fixture)) + ';\n' + '''
const ai = dirname(findPackageJSON('@earendil-works/pi-ai', packageFile));
const models = pathToFileURL(join(ai, 'dist/models.js')).href;
const source = `import {readFileSync} from 'node:fs';
export function createModels() {
  return {
    setProvider() {},
    getModel(provider, id) { return {provider, id, api:'openai-codex-responses'}; },
    async complete(_model, _context, options) {
      if (options.maxRetries !== 0 || options.transport !== 'sse') throw new Error('Unsafe options');
      const data = JSON.parse(readFileSync(${JSON.stringify(fixture)}, 'utf8'));
      options.onProviderStreamEvent({type:data.completed === false ? 'response.created' : 'response.completed',
        response:{model:'gpt-6-luna'}});
      if (data.nonfinite) data.result.usage.output = Infinity;
      if (data.circular) data.result.self = data.result;
      return data.result;
    },
  };
}`;
registerHooks({resolve(specifier, context, nextResolve) {
  if (specifier === models && context.parentURL === adapter) {
    return {url:'data:text/javascript;base64,' + Buffer.from(source).toString('base64'), shortCircuit:true};
  }
  return nextResolve(specifier, context);
}});
globalThis.fetch = async () => { throw new Error('Offline literal fixture forbids network'); };
''', encoding='utf-8')
            for name, data, constraint, expected in cases:
                with self.subTest(name=name):
                    fixture.write_text(json.dumps(data), encoding='utf-8')
                    run = subprocess.run(['node', str(adapter), '--pi-package', str(package),
                        '--auth-file', str(auth), '--model', 'gpt-6-luna', '--timeout', '10'],
                        input=json.dumps({'request': {'model': 'gpt-6-luna', 'max_completion_tokens': 8,
                            'messages': [{'role': 'user', 'content': 'Literal offline protocol fixture'}]}}),
                        capture_output=True, text=True, encoding='utf-8', timeout=20, cwd=REPO,
                        env=dict(os.environ, NODE_OPTIONS='--import=' + preload.as_uri()))
                    self.assertNotIn(secret, run.stdout + run.stderr)
                    output = json.loads(run.stdout)
                    if constraint is None:
                        self.assertEqual(run.returncode, 0, run.stdout + run.stderr)
                        self.assertNotIn('constraint', output)
                        self.assertEqual(output['stopReason'], data['result']['stopReason'])
                    else:
                        self.assertNotEqual(run.returncode, 0)
                        self.assertEqual(output['error'], 'content-invalid')
                        self.assertEqual(output['constraint'], constraint)
                        self.assertNotIn('upstreamResponse', output)
                        for key, value in expected.items():
                            self.assertEqual(output[key], value)
                    self.assertEqual(auth.read_bytes(), original)
            print(f'Offline literal SDK vectors: {len(cases)}; live model requests: 0')

    def test_wrapper_keeps_only_bounded_structural_facts(self):
        spec = importlib.util.spec_from_file_location('codex_wrapper', REPO / 'scripts/accept-workflow-codex.py')
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        safe = module.safe_content_diagnostics
        value = {'error': 'content-invalid', 'constraint': 'sdk-content', 'completed': False,
                 'stopReason': 'other', 'contentBlocks': 2, 'usageInvalidMask': 3,
                 'outputTokens': 9, 'deliveryCap': 8}
        self.assertEqual(safe({**value, 'content': 'SECRET', 'arguments': 'SECRET',
                               'exception': 'SECRET', 'headers': {'Authorization': 'SECRET'}}),
                         {key: item for key, item in value.items() if key != 'error'})
        for malformed in [None, [], {'error': 'provider-error'},
                          {'error': 'content-invalid', 'constraint': {'secret': 'SECRET'}},
                          {'error': 'content-invalid', 'constraint': 'SECRET'}]:
            self.assertEqual(safe(malformed), {})
        self.assertEqual(safe({'error': 'content-invalid', 'constraint': 'usage',
                              'stopReason': {'secret': 'SECRET'}, 'completed': 1, 'hasText': 'true',
                              'toolCalls': True, 'contentBlocks': -1, 'usageInvalidMask': 2**40,
                              'outputTokens': float('inf'), 'deliveryCap': True}), {'constraint': 'usage'})
        for constraint in ['sdk-content', 'decision-consistency', 'usage', 'delivery-cap', 'serialization']:
            self.assertEqual(safe({'error': 'content-invalid', 'constraint': constraint}), {'constraint': constraint})


if __name__ == '__main__':
    unittest.main()
