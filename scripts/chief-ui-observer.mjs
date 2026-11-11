// Acceptance-only observer of the installed host's public extension UI API.
// It never chooses a tool, changes arguments/results, approves, or cancels UI.
import { mkdirSync, renameSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { randomUUID } from 'node:crypto';

export default function observeNativeConfirmation(api) {
  const directory = process.env.CHIEF_MAP_CONFIRMATION_EVIDENCE_DIR;
  if (!directory) throw new Error('Missing owned native confirmation evidence directory');
  mkdirSync(directory, { recursive: true });
  const observed = new WeakSet();
  const save = (id, kind, value) => {
    try {
      const target = join(directory, `${id}.${kind}.json`);
      const pending = target + '.pending';
      writeFileSync(pending, JSON.stringify(value, null, 2), { flag: 'wx' });
      renameSync(pending, target);
    } catch (error) {
      // Observation failure must not alter the actual native decision. The
      // acceptance driver requires these artifacts and therefore fails closed.
      console.error('Native confirmation observation failed', error);
    }
  };
  const install = ctx => {
    const ui = ctx.ui;
    if (!ctx.hasUI || !ui || typeof ui.confirm !== 'function' || observed.has(ui)) return;
    const original = ui.confirm;
    try {
      ui.confirm = async (...args) => {
        const id = randomUUID();
        const started = performance.now();
        const common = { id, sessionId: ctx.sessionManager.getSessionId(), pid: process.pid, hasUI: ctx.hasUI };
        const options = args[2];
        save(id, 'request', { ...common, title: args[0], message: args[1], timeoutMs: options?.timeout, at: Date.now() });
        options?.signal?.addEventListener('abort', () => save(id, 'abort', {
          ...common, elapsedMs: performance.now() - started, at: Date.now(),
        }), { once: true });
        try {
          const result = await Reflect.apply(original, ui, args);
          save(id, 'response', { ...common, result, elapsedMs: performance.now() - started, at: Date.now() });
          return result;
        } catch (error) {
          save(id, 'error', { ...common, error: String(error), elapsedMs: performance.now() - started, at: Date.now() });
          throw error;
        }
      };
      observed.add(ui);
    } catch (error) {
      console.error('Cannot observe public native confirmation interface', error);
    }
  };
  api.on('session_start', (_event, ctx) => install(ctx));
  api.on('tool_call', (_event, ctx) => install(ctx));
}
