// Acceptance-only observation of public native events; never supplies a result.
import { appendFileSync, mkdirSync, renameSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { randomUUID } from 'node:crypto';
import observeConfirmation from './chief-ui-observer.mjs';

export default function observeApproval(api) {
  observeConfirmation(api);
  const directory = process.env.CHIEF_MAP_CONFIRMATION_EVIDENCE_DIR;
  const forms = join(directory, 'forms');
  const observed = new WeakSet();
  const save = (id, kind, value) => {
    try {
      mkdirSync(forms, { recursive: true });
      const target = join(forms, `${id}.${kind}.json`);
      writeFileSync(target + '.pending', JSON.stringify(value, null, 2), { flag: 'wx' });
      renameSync(target + '.pending', target);
    } catch (error) {
      console.error('Native form observation failed', error);
    }
  };
  const installForm = ctx => {
    if (!ctx.hasUI || !ctx.ui) return;
    const ui = ctx.ui;
    const original = Object.getOwnPropertyDescriptor(ui, 'formDialog')?.value;
    if (typeof original !== 'function' || observed.has(original)) return;
    const wrapped = async (...args) => {
      const id = randomUUID();
      const started = performance.now();
      const common = { id, sessionId: ctx.sessionManager.getSessionId(), pid: process.pid, hasUI: ctx.hasUI };
      save(id, 'request', { ...common, html: args[0], timeoutMs: args[1]?.timeout, at: Date.now() });
      try {
        const result = await Reflect.apply(original, ui, args);
        save(id, 'response', { ...common, result, elapsedMs: performance.now() - started, at: Date.now() });
        return result;
      } catch (error) {
        save(id, 'error', { ...common, error: String(error), elapsedMs: performance.now() - started, at: Date.now() });
        throw error;
      }
    };
    try {
      observed.add(wrapped);
      ui.formDialog = wrapped;
    } catch (error) {
      console.error('Cannot observe public native form interface', error);
    }
  };
  const record = (event, ctx) => {
    installForm(ctx);
    appendFileSync(join(directory, 'native-events.jsonl'), JSON.stringify({
      at: Date.now(), monotonicMs: performance.now(), pid: process.pid,
      sessionId: ctx.sessionManager.getSessionId(), hasUI: ctx.hasUI,
      type: event.type, toolName: event.toolName, toolCallId: event.toolCallId,
      input: event.input, content: event.content, isError: event.isError,
    }) + '\n');
  };
  api.on('session_start', record);
  api.on('tool_call', record);
  api.on('tool_result', record);
}
