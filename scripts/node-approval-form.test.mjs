// Acceptance-helper regression only: no model, agent process, service or native UI claim.
import { expect, test } from 'bun:test';
import { mkdtempSync, readdirSync, readFileSync, realpathSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { pathToFileURL } from 'node:url';
import observe from './node-approval-observer.mjs';

const installation = process.env.NODE_APPROVAL_HANDOFF_OMP_PACKAGE;
if (!installation) throw new Error('Missing explicit NODE_APPROVAL_HANDOFF_OMP_PACKAGE');
const resolve = specifier => Bun.resolveSync(specifier, realpathSync(installation));
const { parseHtmlForm } = await import(pathToFileURL(resolve('@oh-my-pi/pi-tui/html-form/parse')).href);
const { HtmlFormDialog } = await import(pathToFileURL(resolve('@oh-my-pi/pi-tui/html-form/dialog')).href);
const { formAnswers } = await import(pathToFileURL(resolve('@oh-my-pi/pi-tui/html-form/data')).href);
const html = value => `<form><p>订单7件，每件1200分，折扣尚未提供。</p><label>折扣百分比 <input type="number" name="discountPercent" min="0" max="100" required value="${value}"></label></form>`;

function harness(ui) {
  const directory = mkdtempSync(join(tmpdir(), 'native-form-observer-'));
  const previous = process.env.CHIEF_MAP_CONFIRMATION_EVIDENCE_DIR;
  process.env.CHIEF_MAP_CONFIRMATION_EVIDENCE_DIR = directory;
  const handlers = new Map();
  const api = { on(name, callback) { handlers.set(name, [...(handlers.get(name) ?? []), callback]); } };
  const ctx = { hasUI: true, ui, sessionManager: { getSessionId: () => 'owned-offline-form-session' } };
  observe(api);
  const emit = name => { for (const handler of handlers.get(name) ?? []) handler({ type: name }, ctx); };
  emit('session_start');
  emit('tool_call'); // Reinstalling must not wrap the same underlying method again.
  return {
    directory, ctx, emit,
    files: suffix => readdirSync(join(directory, 'forms')).filter(name => name.endsWith(suffix)),
    read: name => JSON.parse(readFileSync(join(directory, 'forms', name), 'utf8')),
    dispose() {
      if (previous === undefined) delete process.env.CHIEF_MAP_CONFIRMATION_EVIDENCE_DIR;
      else process.env.CHIEF_MAP_CONFIRMATION_EVIDENCE_DIR = previous;
      rmSync(directory, { recursive: true, force: true });
    },
  };
}

for (const initial of ['', '10']) {
  test(`actual installed form receives 25 through typing and Submit (initial=${JSON.stringify(initial)})`, async () => {
    let dialog;
    let calls = 0;
    const options = { signal: new AbortController().signal };
    const ui = {
      formDialog(source, actualOptions) {
        calls++;
        expect(this).toBe(ui);
        expect(actualOptions).toBe(options);
        return new Promise(resolve => {
          dialog = new HtmlFormDialog(parseHtmlForm(source), {
            onSubmit(data, notes) { resolve({ data, notes }); },
            onCancel() { throw new Error('The business form must not be cancelled'); },
          });
        });
      },
    };
    const owned = harness(ui);
    try {
      const pending = ui.formDialog(html(initial), options);
      expect(owned.files('.request.json')).toHaveLength(1);
      expect(owned.files('.response.json')).toHaveLength(0);
      dialog.handleInput('\u001b[H');
      for (const _ of initial) dialog.handleInput('\u001b[3~');
      dialog.handleInput('2');
      dialog.handleInput('5');
      expect(dialog.data()).toEqual({ discountPercent: 25 });
      dialog.handleInput('\t');
      expect(owned.files('.response.json')).toHaveLength(0);
      dialog.handleInput('\r');
      const result = await pending;
      expect(result.data).toEqual({ discountPercent: 25 });
      expect(formAnswers(parseHtmlForm(html(initial)), result.data)).toEqual([
        { name: 'discountPercent', question: '折扣百分比', answer: '25' },
      ]);
      expect(calls).toBe(1);
      const request = owned.read(owned.files('.request.json')[0]);
      const response = owned.read(owned.files('.response.json')[0]);
      expect(request.html).toBe(html(initial));
      expect(request.sessionId).toBe('owned-offline-form-session');
      expect(response.result).toEqual(result);
    } finally {
      dialog?.dispose();
      owned.dispose();
    }
  });
}

test('observer preserves the original result reference and error without supplying a decision', async () => {
  const result = { data: { discountPercent: '25' } };
  const failure = new Error('actual native UI failure');
  let fail = false;
  const ui = { async formDialog() { if (fail) throw failure; return result; } };
  const owned = harness(ui);
  try {
    expect(await ui.formDialog(html(''))).toBe(result);
    fail = true;
    let caught;
    try { await ui.formDialog(html('')); } catch (error) { caught = error; }
    expect(caught).toBe(failure);
    expect(owned.files('.response.json')).toHaveLength(1);
    expect(owned.files('.error.json')).toHaveLength(1);
  } finally {
    owned.dispose();
  }
});
