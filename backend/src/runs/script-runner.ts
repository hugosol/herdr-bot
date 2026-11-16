import { plugin } from "bun";
import { pathToFileURL } from "node:url";
import { resolve } from "node:path";

const prefix = "\u001eherdr-script:";
function send(value: unknown) {
  const completion = Promise.withResolvers<void>();
  process.stdout.write(prefix + JSON.stringify(value) + "\n", error => {
    if (error) completion.reject(error); else completion.resolve();
  });
  return completion.promise;
}
// User logging is diagnostic output, never a protocol message.
for (const level of ["log", "info", "debug", "warn", "error"] as const) {
  console[level] = (...args: unknown[]) => process.stderr.write(args.map(value => typeof value === "string" ? value : Bun.inspect(value)).join(" ") + "\n");
}
const pending = new Map<number, { resolve(value: unknown): void; reject(error: Error): void }>();
let nextId = 0;
type Request = { script: string; source: string; context: Record<string, unknown> };
const initial = Promise.withResolvers<Request>();
void (async () => {
  let first = true;
  for await (const line of consoleLines()) {
    const message: unknown = JSON.parse(line);
    if (!message || typeof message !== "object") throw new Error("Invalid host message");
    if (first) {
      if (!("script" in message) || typeof message.script !== "string" || !("source" in message) || typeof message.source !== "string" || !("context" in message) || !message.context || typeof message.context !== "object" || Array.isArray(message.context)) throw new Error("Invalid script request");
      first = false;
      initial.resolve({ script: message.script, source: message.source, context: Object.fromEntries(Object.entries(message.context)) });
      continue;
    }
    if (!("id" in message) || typeof message.id !== "number" || !("ok" in message)) throw new Error("Invalid host response");
    const call = pending.get(message.id);
    if (!call) throw new Error("Unknown host response id");
    pending.delete(message.id);
    if (message.ok === true && "value" in message) call.resolve(message.value);
    else if ("error" in message && message.error && typeof message.error === "object" && "message" in message.error && typeof message.error.message === "string" && "code" in message.error && typeof message.error.code === "string") call.reject(Object.assign(new Error(message.error.message), message.error));
    else throw new Error("Invalid host error");
  }
  for (const call of pending.values()) call.reject(new Error("Host input closed"));
})();
async function* consoleLines() {
  const decoder = new TextDecoder();
  let buffer = "";
  for await (const chunk of Bun.stdin.stream()) {
    buffer += decoder.decode(chunk, { stream: true });
    let newline: number;
    while ((newline = buffer.indexOf("\n")) >= 0) {
      const line = buffer.slice(0, newline); buffer = buffer.slice(newline + 1);
      if (line) yield line;
    }
  }
}
try {
  const request = await initial.promise;
  const entry = resolve(request.script);
  const entryUrl = pathToFileURL(entry).href;
  const flags = process.platform === "win32" ? "i" : "";
  const patterns = [entry, entryUrl].map(path => path.split(/[\\/]/).map(part => part.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")).join("[\\\\/]"));
  const entryFilter = new RegExp("^" + patterns[0] + "$", flags);
  const resolveFilter = new RegExp("^(?:" + patterns.join("|") + ")$", flags);
  plugin({ name: "herdr-frozen-entry", setup(build) {
    build.onResolve({ filter: resolveFilter }, () => ({ path: entry, namespace: "file" }));
    build.onLoad({ filter: entryFilter }, () => ({ contents: request.source, loader: "ts" }));
  } });
  // The entry is selected by the Bot binding at runtime, not at author time.
  const module = await import(entryUrl);
  if (typeof module.default !== "function") throw new Error("Script must export default async function(context)");
  const host = (operation: string, args: unknown) => {
    const completion = Promise.withResolvers<unknown>();
    const id = ++nextId;
    pending.set(id, completion);
    void send({ type: "call", id, operation, args }).catch(completion.reject);
    return completion.promise;
  };
  const value = await module.default({ ...request.context, host });
  if (pending.size) throw new Error("Script returned with unfinished host operations");
  await send({ type: "result", value });
  process.exit(0);
} catch (error) {
  const completion = Promise.withResolvers<void>();
  process.stderr.write((error instanceof Error ? error.stack ?? error.message : Bun.inspect(error)) + "\n", () => completion.resolve());
  await completion.promise;
  process.exit(1);
}
