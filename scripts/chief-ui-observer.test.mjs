import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, readdirSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import observe from './chief-ui-observer.mjs';

// External host boundary: OMP handler contexts expose a forwarding Proxy whose
// method setter mutates its target. The real native run separately covers OMP.
test('confirmation observation preserves decisions across scoped host contexts', async () => {
  const directory = mkdtempSync(join(tmpdir(), 'chief-observer-'));
  const before = process.env.CHIEF_MAP_CONFIRMATION_EVIDENCE_DIR;
  process.env.CHIEF_MAP_CONFIRMATION_EVIDENCE_DIR = directory;
  try {
    const handlers = new Map();
    observe({ on: (event, handler) => handlers.set(event, handler) });
    const calls = [];
    const native = { async confirm(...args) { calls.push(args); return false; } };
    const scoped = () => new Proxy(native, { get(target, key) {
      if (key === 'confirm') return (...args) => target.confirm(...args);
      return Reflect.get(target, key);
    } });
    const context = () => ({ ui: scoped(), hasUI: true, sessionManager: { getSessionId: () => 'owned-session' } });
    handlers.get('session_start')({}, context());
    handlers.get('tool_call')({}, context());
    const options = { timeout: 20000, signal: new AbortController().signal };
    assert.equal(await native.confirm('actual-title', 'complete-message', options), false);
    assert.equal(calls.length, 1);
    assert.deepEqual(calls[0], ['actual-title', 'complete-message', options]);
    const records = readdirSync(directory).map(file => JSON.parse(readFileSync(join(directory, file), 'utf8')));
    assert.equal(records.length, 2);
    assert.equal(records.find(record => record.title)?.message, 'complete-message');
    assert.equal(records.find(record => 'result' in record)?.result, false);
  } finally {
    if (before === undefined) delete process.env.CHIEF_MAP_CONFIRMATION_EVIDENCE_DIR;
    else process.env.CHIEF_MAP_CONFIRMATION_EVIDENCE_DIR = before;
    rmSync(directory, { recursive: true });
  }
});
