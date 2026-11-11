#!/usr/bin/env node
// One stdin {request: <Chat Completions body>} -> one native Codex SSE request.
// No sessions, refresh, replay, text-wrapped conversations, or body-JSON decisions.
// Usage: node scripts/workflow-codex-completion.mjs --pi-package <absolute>
//   --auth-file <absolute-auth.json> --model gpt-6-luna --timeout 300
// Pipe exactly {"request":{...}}; stdout is one result or {"error":"safe-code"}.
// Supported messages: system/user/assistant/tool, text only; function tools only.
// Chat stream/usage flags are consumed by the HTTP host; this helper returns one
// complete result. store must be false. max_completion_tokens is delivery-only,
// not a server-side budget. Unknown fields/content or broken bindings fail closed.
// Installed pi-ai 1.4.0 public exports: models and providers/openai-codex.
// ReadOnlyAuthStorage is not exported at the coding-agent root; its installed
// core module rejects modify BEFORE invoking the refresh callback (resolve.js).
import { readFile } from 'node:fs/promises';
import { isAbsolute, join, dirname } from 'node:path';
import { findPackageJSON } from 'node:module';
import { pathToFileURL } from 'node:url';

let failure = 'invalid-arguments';
let finished = false;
const diagnostics = {};
function finish(value, status) {
  if (finished) return;
  finished = true;
  process.stdout.write(`${JSON.stringify(value)}\n`, () => process.exit(status));
}
function reject(code) { finish({ error: code, ...diagnostics }, 1); }
// Dependency diagnostics are not part of the protocol and may contain secrets.
for (const key of ['log', 'info', 'warn', 'error', 'debug', 'trace', 'dir']) console[key] = () => {};
process.on('uncaughtException', () => reject(failure));
process.on('unhandledRejection', () => reject(failure));

try {
  const args = new Map();
  const allowed = new Set(['--pi-package', '--auth-file', '--model', '--timeout']);
  for (let i = 2; i < process.argv.length; i += 2) {
    const key = process.argv[i], value = process.argv[i + 1];
    if (!allowed.has(key) || args.has(key) || !value) throw new Error();
    args.set(key, value);
  }
  const pi = args.get('--pi-package'), authFile = args.get('--auth-file');
  const requestedModel = args.get('--model');
  const seconds = Number(args.get('--timeout') ?? '300');
  if (!pi || !authFile || !isAbsolute(pi) || !isAbsolute(authFile) ||
      requestedModel !== 'gpt-6-luna' || !Number.isFinite(seconds) || seconds <= 0 ||
      seconds * 1000 > 2147483647) throw new Error();
  const timeoutMs = Math.ceil(seconds * 1000);
  const abort = new AbortController();
  setTimeout(() => { abort.abort(); reject('timeout'); }, timeoutMs);

  failure = 'invalid-input';
  let input = '';
  process.stdin.setEncoding('utf8');
  for await (const chunk of process.stdin) input += chunk;
  const object = value => value !== null && typeof value === 'object' && !Array.isArray(value);
  const fields = (value, allowed) => {
    if (!object(value) || Object.keys(value).some(key => !allowed.includes(key))) throw new Error();
  };
  const nonempty = value => typeof value === 'string' && value.length > 0;
  const text = value => {
    if (typeof value === 'string') return [{ type: 'text', text: value }];
    if (!Array.isArray(value) || value.length === 0) throw new Error();
    return value.map(part => {
      fields(part, ['type', 'text']);
      if (part.type !== 'text' || typeof part.text !== 'string') throw new Error();
      return { type: 'text', text: part.text };
    });
  };
  // The provider normalizes call IDs. Reject IDs it would rewrite rather than
  // silently changing the host's identity/binding (SDK IDs may include |fc_*).
  const callId = value => typeof value === 'string' && value.split('|').length <= 2 &&
    value.split('|').every((part, index) => /^[a-zA-Z0-9_-]{1,64}$/.test(part) &&
      !part.endsWith('_') && (index === 0 || part.startsWith('fc_')));
  const envelope = JSON.parse(input);
  fields(envelope, ['request']);
  const request = envelope.request;
  fields(request, ['model', 'messages', 'tools', 'tool_choice', 'parallel_tool_calls', 'temperature',
    'stream', 'store', 'stream_options', 'max_completion_tokens']);
  if (request.store !== undefined && request.store !== false) throw new Error();
  if (request.max_completion_tokens !== undefined &&
      (!Number.isSafeInteger(request.max_completion_tokens) || request.max_completion_tokens <= 0)) throw new Error();
  if (request.stream_options !== undefined) {
    fields(request.stream_options, ['include_usage']);
    if (request.stream_options.include_usage !== undefined && typeof request.stream_options.include_usage !== 'boolean') throw new Error();
  }
  if (request.model !== undefined && request.model !== requestedModel) throw new Error();
  if (!Array.isArray(request.messages) || request.messages.length === 0 ||
      (request.stream !== undefined && typeof request.stream !== 'boolean') ||
      (request.parallel_tool_calls !== undefined && typeof request.parallel_tool_calls !== 'boolean') ||
      (request.temperature !== undefined && (!Number.isFinite(request.temperature) || request.temperature < 0 || request.temperature > 2))) throw new Error();
  const toolNames = new Set();
  const tools = request.tools === undefined ? undefined : (() => {
    if (!Array.isArray(request.tools)) throw new Error();
    return request.tools.map(tool => {
      fields(tool, ['type', 'function']);
      fields(tool.function, ['name', 'description', 'parameters', 'strict']);
      const fn = tool.function;
      if (tool.type !== 'function' || !nonempty(fn.name) || toolNames.has(fn.name) ||
          (fn.description !== undefined && typeof fn.description !== 'string') ||
          !object(fn.parameters) || (fn.strict !== undefined && typeof fn.strict !== 'boolean')) throw new Error();
      toolNames.add(fn.name);
      return { name: fn.name, description: fn.description ?? '', parameters: fn.parameters };
    });
  })();
  let toolChoice = request.tool_choice;
  if (object(toolChoice)) {
    fields(toolChoice, ['type', 'function']);
    fields(toolChoice.function, ['name']);
    if (toolChoice.type !== 'function' || !toolNames.has(toolChoice.function.name)) throw new Error();
    toolChoice = { type: 'function', name: toolChoice.function.name };
  } else if (toolChoice !== undefined && !['auto', 'none', 'required'].includes(toolChoice)) throw new Error();
  if (toolChoice !== undefined && toolChoice !== 'none' && toolNames.size === 0) throw new Error();
  const calls = new Map();
  const pending = new Set();
  const timestamp = Date.now();
  const emptyUsage = () => ({ input: 0, output: 0, cacheRead: 0, cacheWrite: 0, totalTokens: 0,
    cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0, total: 0 } });
  const messages = request.messages.map(message => {
    if (!object(message)) throw new Error();
    if (message.role === 'tool') {
      fields(message, ['role', 'content', 'tool_call_id', 'name', 'is_error']);
      const name = calls.get(message.tool_call_id);
      if (!name || !pending.delete(message.tool_call_id) ||
          (message.name !== undefined && message.name !== name) ||
          (message.is_error !== undefined && typeof message.is_error !== 'boolean')) throw new Error();
      return { role: 'toolResult', toolCallId: message.tool_call_id, toolName: name,
        content: text(message.content), isError: message.is_error ?? false, timestamp };
    }
    // transformMessages repairs incomplete conversations by inventing tool results.
    // Reject those conversations before the SDK gets an opportunity to repair them.
    if (pending.size !== 0) throw new Error();
    if (message.role === 'system' || message.role === 'user') {
      fields(message, ['role', 'content']);
      return { role: message.role, content: text(message.content), timestamp };
    }
    if (message.role !== 'assistant') throw new Error();
    fields(message, ['role', 'content', 'tool_calls']);
    const content = message.content === null || message.content === undefined ? [] : text(message.content);
    if (message.tool_calls !== undefined) {
      if (!Array.isArray(message.tool_calls) || message.tool_calls.length === 0) throw new Error();
      for (const call of message.tool_calls) {
        fields(call, ['id', 'type', 'function']);
        fields(call.function, ['name', 'arguments']);
        if (call.type !== 'function' || !callId(call.id) || calls.has(call.id) ||
            !nonempty(call.function.name) || typeof call.function.arguments !== 'string') throw new Error();
        const parameters = JSON.parse(call.function.arguments);
        if (!object(parameters)) throw new Error();
        calls.set(call.id, call.function.name);
        pending.add(call.id);
        content.push({ type: 'toolCall', id: call.id, name: call.function.name, arguments: parameters });
      }
    }
    if (content.length === 0) throw new Error();
    return { role: 'assistant', content, api: 'openai-codex-responses', provider: 'openai-codex',
      model: requestedModel, usage: emptyUsage(), stopReason: pending.size ? 'toolUse' : 'stop', timestamp };
  });
  if (pending.size !== 0) throw new Error();
  // Context.tools is shorthand that prepends a system message even when one
  // already exists. Bind declarations directly to avoid duplicating the prompt.
  const nativeMessages = [...messages];
  if (tools?.length) {
    if (nativeMessages[0].role === 'system') nativeMessages[0] = { ...nativeMessages[0], toolsAdded: tools };
    else nativeMessages.unshift({ role: 'system', content: '', toolsAdded: tools, timestamp });
  }
  const context = { messages: nativeMessages };

  // Check expiry before loading OAuth or interpreting the access token. Never
  // call refresh: even near-expiry tokens are rejected, not refreshed/discarded.
  failure = 'auth-missing';
  const auth = JSON.parse(await readFile(authFile, 'utf8'));
  const credential = auth?.['openai-codex'];
  if (!credential || credential.type !== 'oauth' || typeof credential.access !== 'string' ||
      !credential.access || typeof credential.refresh !== 'string' || !credential.refresh ||
      !Number.isFinite(credential.expires)) throw new Error();
  failure = 'oauth-expired';
  const validUntil = Date.now() + Math.max(300000, timeoutMs);
  if (credential.expires <= validUntil) throw new Error();

  failure = 'dependency-missing';
  // Resolve the dependency from this installation, including nested npm layouts.
  const ai = dirname(findPackageJSON('@earendil-works/pi-ai', pathToFileURL(join(pi, 'package.json'))));
  const load = path => import(pathToFileURL(path).href);
  const { createModels } = await load(join(ai, 'dist/models.js'));
  const { openaiCodexProvider } = await load(join(ai, 'dist/providers/openai-codex.js'));
  const { getSystemMessageText } = await load(join(ai, 'dist/utils/text.js'));
  const { ReadOnlyAuthStorage } = await load(join(pi, 'dist/core/auth-storage.js'));
  const { applyHttpProxySettings, configureHttpDispatcher } = await load(join(pi, 'dist/core/http-dispatcher.js'));
  // The SDK alone does not install the CLI's proxy-aware fetch. Preserve the
  // already configured Codex proxy inside this process, without changing PATH
  // or any global/user configuration; do not silently attempt a direct route.
  applyHttpProxySettings(process.env.PI_PROXY_OPENAI_CODEX);
  configureHttpDispatcher(timeoutMs);
  failure = 'auth-missing';
  const storage = new ReadOnlyAuthStorage(authFile);
  const snapshot = await storage.read('openai-codex');
  if (snapshot?.type !== 'oauth' || snapshot.access !== credential.access ||
      snapshot.expires !== credential.expires) throw new Error();
  // Read only the validated snapshot; expiry is checked again on resolution.
  // modify/delete never execute any callback, so rotation cannot start at all.
  const credentials = {
    async read(id) {
      if (id !== 'openai-codex') return undefined;
      if (snapshot.expires <= Date.now() + Math.max(300000, timeoutMs)) {
        failure = 'oauth-expired'; throw new Error();
      }
      return structuredClone(snapshot);
    },
    async list() { return [{ providerId: 'openai-codex', type: 'oauth' }]; },
    async modify() { failure = 'oauth-expired'; throw new Error(); },
    async delete() { throw new Error(); },
  };
  const models = createModels({ credentials });
  models.setProvider(openaiCodexProvider());
  failure = 'model-unavailable';
  const model = models.getModel('openai-codex', requestedModel);
  if (!model || model.api !== 'openai-codex-responses') throw new Error();
  let observedModel;
  let completed = false;
  let requests = 0;
  let upstreamRequest;
  failure = 'provider-error';
  // Codex accepts ordered developer messages. Do not let the SDK's conservative
  // compatibility default collapse later system instructions into the first one.
  const nativeModel = { ...model, compat: { ...model.compat, supportsMidConvoSystemMessages: true } };
  const result = await models.complete(nativeModel, context, {
    transport: 'sse', maxRetries: 0, timeoutMs, signal: abort.signal,
    cacheRetention: 'none', temperature: request.temperature, toolChoice,
    // Codex does not emit maxTokens as a server output budget. This is checked
    // below as a delivery cap only; never claim the server enforced it.
    maxTokens: request.max_completion_tokens,
    onPayload(body) {
      // The provider otherwise invents "You are a helpful assistant." when the
      // leading system text is absent/empty. Keep only caller-supplied material.
      body.instructions = messages[0].role === 'system' ? getSystemMessageText(messages[0]) : '';
      if (request.parallel_tool_calls !== undefined) body.parallel_tool_calls = request.parallel_tool_calls;
      const declarations = [...(body.tools ?? []), ...(body.input ?? [])
        .filter(item => item.type === 'additional_tools').flatMap(item => item.tools ?? [])];
      for (const tool of declarations) {
        const source = request.tools?.find(value => value.function.name === tool.name)?.function;
        if (source?.strict !== undefined) tool.strict = source.strict;
      }
      upstreamRequest = structuredClone(body);
      return body;
    },
    // Disable redirects too: a redirect must not replay the credentialed POST.
    fetch: async (url, options) => {
      diagnostics.requests = ++requests;
      if (requests !== 1) throw new Error();
      try {
        const response = await globalThis.fetch(url, { ...options, redirect: 'error' });
        diagnostics.httpStatus = response.status;
        return response;
      } catch (error) {
        const code = error?.cause?.code;
        diagnostics.network = ['ENOTFOUND', 'EAI_AGAIN'].includes(code) ? 'dns'
          : ['ECONNREFUSED', 'ECONNRESET', 'UND_ERR_SOCKET'].includes(code) ? 'connect'
          : ['ETIMEDOUT', 'UND_ERR_CONNECT_TIMEOUT', 'UND_ERR_HEADERS_TIMEOUT'].includes(code) ? 'timeout'
          : ['CERT_HAS_EXPIRED', 'UNABLE_TO_VERIFY_LEAF_SIGNATURE', 'DEPTH_ZERO_SELF_SIGNED_CERT'].includes(code) ? 'tls'
          : 'other';
        throw error;
      }
    },
    onProviderStreamEvent(event) {
      if (!event || typeof event !== 'object') return;
      const identity = event.response?.model;
      if (identity !== undefined) {
        if (identity !== requestedModel) { failure = 'model-identity'; throw new Error(); }
        observedModel = identity;
      }
      if (event.type === 'response.completed' || event.type === 'response.done') completed = true;
    },
  });
  if (result.stopReason === 'error' || result.stopReason === 'aborted') throw new Error();
  failure = 'model-identity';
  // AssistantMessage.model is locally initialized by pi-ai, not server evidence.
  if (observedModel !== requestedModel || result.model !== requestedModel ||
      result.provider !== 'openai-codex' || result.api !== 'openai-codex-responses') throw new Error();
  failure = 'content-invalid';
  if (!completed || !['stop', 'toolUse'].includes(result.stopReason) || !Array.isArray(result.content) ||
      result.content.some(part => !part || !['text', 'thinking', 'toolCall'].includes(part.type) ||
        (part.type === 'text' && typeof part.text !== 'string') ||
        (part.type === 'toolCall' && (!callId(part.id) || !nonempty(part.name) ||
          !object(part.arguments) || part.argumentsError !== undefined || part.partialJson !== undefined ||
          part.namespace !== undefined)))) throw new Error();
  const content = result.content.filter(part => part.type === 'text').map(part => part.text).join('');
  // These are SDK-native toolCall blocks, never JSON extracted from model text.
  const tool_calls = result.content.filter(part => part.type === 'toolCall')
    .map(part => ({ id: part.id, name: part.name, arguments: part.arguments }));
  if ((result.stopReason === 'toolUse') !== (tool_calls.length > 0) ||
      new Set(tool_calls.map(call => call.id)).size !== tool_calls.length ||
      (!content.trim() && tool_calls.length === 0)) throw new Error();
  const usage = {};
  for (const key of ['input', 'output', 'cacheRead', 'cacheWrite', 'totalTokens']) {
    const value = result.usage?.[key];
    if (!Number.isFinite(value) || value < 0) throw new Error();
    usage[key] = value;
  }
  if (request.max_completion_tokens !== undefined && usage.output > request.max_completion_tokens) throw new Error();
  finish({ provider: result.provider, model: result.model, serverModel: observedModel, api: result.api,
    content, tool_calls, usage, stopReason: result.stopReason,
    upstreamRequest, upstreamResponse: result,
    outputLimit: request.max_completion_tokens === undefined ? undefined
      : { tokens: request.max_completion_tokens, enforcement: 'delivery-only' } }, 0);
} catch {
  reject(failure);
}
