// Acceptance-only observation of public native events; never supplies a result.
import { appendFileSync } from 'node:fs';
import { join } from 'node:path';
import observeConfirmation from './chief-ui-observer.mjs';

export default function observeApproval(api) {
  observeConfirmation(api);
  const directory = process.env.CHIEF_MAP_CONFIRMATION_EVIDENCE_DIR;
  const record = (event, ctx) => {
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
