import { join } from "node:path";

export interface Target { type: "task" | "map"; task_id?: string; map_file?: string }
export interface Bot {
  id: string; name: string; when: string; plan_guide: string; node_guide: string;
  group?: string; task_id?: string; map_file?: string; script?: string;
}
export interface Revision { file: string; original: string }
export interface Request {
  schema_version: number; kind: string; target: Target;
  bot?: { id?: string; name?: string; group?: string };
  requirements: string; constraints: string[]; non_goals: string[]; resource_limits: string[];
}
export interface Guide {
  schema_version: number; kind: string; stage: string; bot_id: string; target: Target; field: string;
  sources: { binding_id: string; direction: string; role: string }[];
  confirmation: { evidence: string; text_sha256: string; full_text: boolean; save_authorized: boolean; semantics_preserved: boolean };
  constraints: string[]; unverified: string[];
}
export interface HostOperations {
  read_json: { args: { path: string }; result: unknown };
  parse_json: { args: { text: string }; result: unknown };
  read_text: { args: { path: string }; result: string };
  target: { args: Target; result: { configuration: unknown; name: string; default_id: string } };
  bot_read: { args: { id: string }; result: { bot: Bot; revision: Revision } };
  bot_save: { args: { id: string; content: Omit<Bot, "id">; revision: Revision | null }; result: Bot };
  material: { args: { id: string; document: unknown }; result: { id: string; path: string } };
}
export interface Context {
  inputs?: { id: string; path: string }[] | null;
  outputRule?: { id: string }[] | null;
  outputDirectory: string;
  host<K extends keyof HostOperations>(operation: K, args: HostOperations[K]["args"]): Promise<HostOperations[K]["result"]>;
}
export type ObjectValue = Record<string, unknown>;
function errorField(error: unknown, key: string): unknown {
  return error !== null && typeof error === "object" ? (error as ObjectValue)[key] : undefined;
}
function errorMessage(error: unknown): string { return String(errorField(error, "message") ?? error); }
export class BusinessError extends Error {
  constructor(public stage: string, public code: string, message: string) { super(message); }
}
export function fail(stage: string, code: string, message: string): never { throw new BusinessError(stage, code, message); }
export function object(value: unknown, required: string[], optional: string[] = []): ObjectValue {
  if (value === null || typeof value !== "object" || Array.isArray(value)) fail("validate", "invalid-format", "材料必须是单个 JSON 对象。");
  for (const key of required) if (!Object.hasOwn(value, key)) fail("validate", "missing-field", `缺少 ${key} 字段。`);
  for (const key of Object.keys(value)) if (!required.includes(key) && !optional.includes(key)) fail("validate", "invalid-format", `未知字段 ${key}。`);
  return value as ObjectValue;
}
export function string(value: unknown): asserts value is string {
  if (typeof value !== "string") fail("validate", "invalid-format", "字段必须是字符串，不能为 null。");
}
export function boolean(value: unknown): asserts value is boolean {
  if (typeof value !== "boolean") fail("validate", "invalid-format", "字段必须是布尔值。");
}
export function strings(value: unknown): asserts value is string[] {
  if (!Array.isArray(value)) fail("validate", "invalid-format", "字段必须是字符串数组。");
  value.forEach(string);
}
export function version(value: unknown): asserts value is number {
  if (typeof value !== "number" || !Number.isInteger(value) || value < 0 || value > 0xffffffff) fail("validate", "invalid-format", "schema_version 必须是 u32。");
}
export function locator(input: unknown): Target {
  const value = object(input, ["type"], ["task_id", "map_file"]);
  string(value.type);
  if (value.type === "task") {
    object(value, ["type", "task_id"]); string(value.task_id);
    if (!value.task_id.trim()) fail("validate", "target-conflict", "Task 目标不能为空。");
  } else if (value.type === "map") {
    object(value, ["type", "map_file"]); string(value.map_file);
    if (!value.map_file || !value.map_file.endsWith(".json") || /[/\\:\0]/.test(value.map_file)) fail("validate", "target-conflict", "Map 目标文件名无效。");
  } else fail("validate", "invalid-format", "创建仅接受 Task 或 Map 目标。");
  return value as unknown as Target;
}
export function request(input: unknown): Request {
  const value = object(input, ["schema_version", "kind", "target", "requirements", "constraints", "non_goals", "resource_limits"], ["bot"]);
  version(value.schema_version); string(value.kind); string(value.requirements);
  const collections = [value.constraints, value.non_goals, value.resource_limits].map(items => { strings(items); return items; });
  if (Object.hasOwn(value, "bot")) {
    const identity = object(value.bot, [], ["id", "name", "group"]);
    Object.values(identity).forEach(string);
    if (identity.group !== undefined) string(identity.group);
    if (["id", "name"].some(key => identity[key] === "") || (identity.group !== undefined && (!identity.group || identity.group.trim() !== identity.group))) fail("validate", "invalid-format", "Bot 身份或分组无效。");
  }
  if (value.schema_version !== 1 || value.kind !== "request" || !value.requirements || collections.some(items => items.some(item => !item))) fail("validate", "invalid-format", "创建 request 格式无效。");
  locator(value.target);
  return value as unknown as Request;
}
export function bot(input: unknown): Bot {
  const value = object(input, ["id", "name", "when", "plan_guide", "node_guide"], ["group", "task_id", "map_file", "script"]);
  Object.values(value).forEach(string);
  return value as unknown as Bot;
}
export function guide(input: unknown): Guide {
  const value = object(input, ["schema_version", "kind", "stage", "bot_id", "target", "field", "sources", "confirmation", "constraints", "unverified"]);
  version(value.schema_version);
  for (const key of ["kind", "stage", "bot_id", "field"]) string(value[key]);
  locator(value.target);
  strings(value.constraints); strings(value.unverified);
  if (!Array.isArray(value.sources)) fail("validate", "invalid-format", "sources 必须是数组。");
  for (const item of value.sources as unknown[]) {
    const source = object(item, ["binding_id", "direction", "role"]);
    Object.values(source).forEach(string);
  }
  const confirmation = object(value.confirmation, ["evidence", "text_sha256", "full_text", "save_authorized", "semantics_preserved"]);
  string(confirmation.evidence); string(confirmation.text_sha256);
  for (const key of ["full_text", "save_authorized", "semantics_preserved"]) boolean(confirmation[key]);
  return value as unknown as Guide;
}
export function rules(context: Context, required: string[]): void {
  if (!context.outputRule) fail("validate", "missing-field", "缺少冻结输出声明。");
  const ids = new Set(context.outputRule.map(rule => rule.id));
  if (ids.size !== context.outputRule.length || required.some(id => !ids.has(id))) fail("validate", "ambiguous-request", `输出必须明确且唯一声明 ${required.join(" 和 ")}。`);
}
export function inputs(context: Context): { id: string; path: string }[] {
  if (!context.inputs) fail("validate", "missing-field", "缺少冻结 inputs。");
  return context.inputs;
}
export class Work {
  write = "not_attempted";
  readBack = "not_attempted";
  materials: { id: string; path: string; status: string }[] = [];
  outputs: { id: string; path: string }[] = [];
  constructor(public context: Context) {}
  async call<K extends keyof HostOperations>(operation: K, args: HostOperations[K]["args"], stage: string, code: string | ((error: unknown) => string)): Promise<HostOperations[K]["result"]> {
    try { return await this.context.host(operation, args); }
    catch (error: unknown) { fail(stage, typeof code === "function" ? code(error) : code, errorMessage(error)); }
  }
  async document(id: string): Promise<ObjectValue> {
    const input = inputs(this.context).find(file => file.id === id);
    if (!input) fail("validate", "missing-field", `缺少 ${id} 命名输入。`);
    const value = await this.call("read_json", { path: input.path }, "parse", error => /duplicate/i.test(errorMessage(error)) ? "ambiguous-request" : "invalid-format");
    if (value === null || typeof value !== "object" || Array.isArray(value)) fail("parse", "invalid-format", "材料必须是单个 JSON 对象。");
    return value as ObjectValue;
  }
  async material(id: string, document: unknown): Promise<void> {
    const effect = { id, path: join(this.context.outputDirectory, `${id}.json`), status: "failed" };
    this.materials.push(effect);
    let output;
    try { output = await this.context.host("material", { id, document }); }
    catch (error: unknown) {
      const path = errorField(error, "path");
      if (typeof path === "string") effect.path = path;
      const code = errorField(error, "code");
      fail("materials", code === "material-read-back-failed" ? code : "material-write-failed", errorMessage(error));
    }
    effect.path = output.path;
    effect.status = "succeeded";
    this.outputs.push({ id: output.id, path: output.path });
  }
  async run(action: () => Promise<void>) {
    try { await action(); return { status: "success", outputs: this.outputs }; }
    catch (error: unknown) {
      // The host checks activity before attempting the public Bot save.
      if (error instanceof BusinessError && error.code === "definition-in-use") this.write = "not_attempted";
      return { status: "failure", outputs: this.outputs, failure: {
        stage: error instanceof BusinessError ? error.stage : "execute",
        code: error instanceof BusinessError ? error.code : "execution-failed",
        message: errorMessage(error),
        effects: { write: this.write, readBack: this.readBack, materials: this.materials },
      } };
    }
  }
}
export function saveCode(error: unknown, conflict: string): string {
  const code = errorField(error, "code");
  switch (code) {
    case "bot-conflict": return conflict;
    case "target-conflict": case "ambiguous-request": case "definition-in-use": return code;
    default: return "write-failed";
  }
}
