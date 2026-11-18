// Acceptance-only scheduling of one marked, invocation-owned native initial
// turn. No tool choice, model result, identity, UI result or payload is replaced.
// Both installed runners await context handlers in registration order. Separate
// lifecycle events are NOT serial. Four bounded context handlers provide an
// owner-released scheduling window without extending the 30s handler budget.
import { appendFileSync, existsSync, readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { createHash } from 'node:crypto';

const canonical = value => {
  if (Array.isArray(value)) return value.map(canonical);
  if (value && typeof value === 'object') return Object.fromEntries(Object.keys(value).sort().map(key => [key, canonical(value[key])]));
  return value;
};

export default function lifecycleGate(api) {
  const configuration = process.env.NODE_APPROVAL_LIFECYCLE_GATE;
  if (!configuration) throw new Error('Missing owned lifecycle gate configuration');
  let marked = false;
  let sequence = 0;
  const save = (directory, value) => {
    appendFileSync(join(directory, 'events.jsonl'), JSON.stringify(value) + '\n');
  };
  api.on('before_agent_start', (event) => {
    if (!existsSync(configuration)) return;
    const config = JSON.parse(readFileSync(configuration, 'utf8'));
    marked = typeof event.prompt === 'string' && event.prompt.includes(config.marker);
  });
  api.on('before_provider_request', (event, ctx) => {
    const payload = event.payload ?? {};
    const projected = JSON.parse(JSON.stringify({model:payload.model, messages:payload.messages ?? [], tools:payload.tools ?? []}));
    const signature = createHash('sha256').update(JSON.stringify(canonical(projected))).digest('hex');
    appendFileSync(join(dirname(configuration), 'native-model-catalogs.jsonl'), JSON.stringify({
      type: 'before_provider_request', sessionId: ctx.sessionManager.getSessionId(), pid: process.pid,
      at: Date.now(), monotonicMs: performance.now(), signature,
    }) + '\n');
  });
  for (let stage = 0; stage < 4; stage++) {
    api.on('context', async (_event, ctx) => {
      if (!marked || !existsSync(configuration)) return;
      const config = JSON.parse(readFileSync(configuration, 'utf8'));
      if (existsSync(config.release)) return;
      const common = { sequence: sequence++, phase: 'context', stage, sessionId: ctx.sessionManager.getSessionId(),
        pid: process.pid, at: Date.now(), monotonicMs: performance.now() };
      save(config.evidence, { ...common, state: 'held-before-first-business-request', hookBudgetMs: 25000 });
      const started = performance.now();
      while (!existsSync(config.release) && performance.now() - started < 25000) {
        await new Promise(resolve => setTimeout(resolve, 25));
      }
      save(config.evidence, { ...common, at: Date.now(), monotonicMs: performance.now(),
        state: existsSync(config.release) ? 'released' : stage === 3 ? 'exhausted' : 'advancing-to-next-pre-request-hook',
        elapsedMs: performance.now() - started });
      // Undefined preserves the public event's original payload and behavior.
    });
  }
}
