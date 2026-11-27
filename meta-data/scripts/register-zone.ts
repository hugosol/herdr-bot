import { execFile } from "node:child_process";
import { lstat, realpath } from "node:fs/promises";
import { isAbsolute, join, parse, resolve } from "node:path";

type Binding = { id: string; path: string };
type Context = {
  serviceUrl: string;
  inputs?: Binding[];
  outputRule?: { id: string }[];
  host(operation: "read_json", args: { path: string }): Promise<unknown>;
  host(operation: "material", args: { id: string; document: unknown }): Promise<Binding>;
};
const TIMEOUT_MS = 10_000;
class RegistrationError extends Error {
  constructor(readonly code: string, message: string, readonly details?: unknown) { super(message); }
}
function reject(code: string, message: string): never { throw new RegistrationError(code, message); }
function message(error: unknown): string { return error instanceof Error ? error.message : String(error); }
function object(value: unknown): value is Record<string, unknown> { return !!value && typeof value === "object" && !Array.isArray(value); }
function exact(value: unknown, keys: string[]): value is Record<string, unknown> {
  return object(value) && Object.keys(value).length === keys.length && keys.every(key => Object.hasOwn(value, key));
}
function samePath(a: string, b: string): boolean { return resolve(a).toLowerCase() === resolve(b).toLowerCase(); }
function absolute(value: unknown): string {
  if (typeof value !== "string" || !isAbsolute(value) || !/^(?:[A-Za-z]:[\\/]|\\\\[^\\/]+[\\/][^\\/]+)/.test(value) || /[\x00-\x1f]/.test(value) || value.startsWith("\\\\?\\") || value.startsWith("\\\\.\\") || value.split(/[\\/]/).some(part => part === "." || part === ".." || /[. ]$/.test(part)) || /:/.test(value.slice(2))) {
    reject("unsafe-path", "必须提供无设备前缀、路径穿越或歧义分量的 Windows 绝对目录路径。");
  }
  return value;
}
async function directory(path: string): Promise<string> {
  const full = resolve(path);
  let current = parse(full).root;
  for (const part of full.slice(current.length).split(/[\\/]/).filter(Boolean)) {
    current = join(current, part);
    const stat = await lstat(current);
    if (!stat.isDirectory() || stat.isSymbolicLink() || !samePath(await realpath(current), current)) reject("unsafe-path", `目录不存在、包含链接或被重定向：${current}`);
  }
  return realpath(full);
}
function query(git: string, cwd: string, args: string[], env: NodeJS.ProcessEnv): Promise<string> {
  const { promise, resolve: accept, reject: fail } = Promise.withResolvers<string>();
  execFile(git, ["-c", "core.hooksPath=/dev/null", "-C", cwd, ...args], { cwd, env, windowsHide: true, timeout: TIMEOUT_MS, maxBuffer: 65_536, encoding: "utf8" }, (error, stdout, stderr) => {
    if (error) fail(new RegistrationError("git-verification-failed", message(error), { stderr }));
    else accept(stdout.trim());
  });
  return promise;
}
async function http(serviceUrl: string, method: "GET" | "POST", body?: unknown): Promise<{ status: number; document: unknown }> {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), TIMEOUT_MS);
  try {
    const response = await fetch(`${serviceUrl}/api/zones`, {
      method, redirect: "error", signal: controller.signal,
      ...(body === undefined ? {} : { headers: { "Content-Type": "application/json" }, body: JSON.stringify(body) }),
    });
    const text = await response.text();
    if (!response.ok) throw new RegistrationError("zone-http-failed", `zone 接口返回 HTTP ${response.status}。`, { status: response.status, body: text.slice(0, 65_536) });
    let document: unknown;
    try { document = JSON.parse(text); }
    catch { reject("invalid-zone-response", "zone 接口未返回合法 JSON。"); }
    return { status: response.status, document };
  } finally { clearTimeout(timer); }
}
function zones(document: unknown): { id: string; path: string }[] {
  if (!object(document) || !Array.isArray(document.zones)) reject("invalid-zone-response", "zone 响应缺少 zones 数组。");
  const seen = new Set<string>();
  for (const item of document.zones) {
    if (!exact(item, ["id", "path"]) || typeof item.id !== "string" || !item.id || typeof item.path !== "string" || !isAbsolute(item.path) || seen.has(item.id)) reject("invalid-zone-response", "zone 响应包含无效或重复登记项。");
    seen.add(item.id);
  }
  return document.zones as { id: string; path: string }[];
}

export default async function registerZone(context: Context) {
  let stage = "validate";
  let zoneId: string | undefined;
  let path: string | undefined;
  let postAttempted = false;
  const effects: Record<string, unknown> = { registration: "not_attempted", observation: "not_attempted", material: "not_attempted" };
  try {
    if (process.platform !== "win32") reject("unsupported-platform", "本脚本面向 Windows 后端。");
    if (!Array.isArray(context.inputs) || context.inputs.length !== 1 || context.inputs[0].id !== "cloned_repository" || !isAbsolute(context.inputs[0].path)) reject("invalid-input-bindings", "必须且只能绑定一个绝对地址输入 cloned_repository。");
    if (!Array.isArray(context.outputRule) || context.outputRule.length !== 1 || context.outputRule[0].id !== "registered_zone") reject("invalid-output-rules", "必须且只能声明输出 registered_zone。");
    const input = await context.host("read_json", { path: context.inputs[0].path });
    if (!exact(input, ["owner", "repo", "repository_id", "ssh_url", "parent_directory", "repository_directory", "head_commit", "checks"])) reject("invalid-cloned-repository", "cloned_repository 必须使用 G1 的完整 JSON 输出字段。");
    if (typeof input.owner !== "string" || !/^[A-Za-z0-9](?:[A-Za-z0-9-]{0,37}[A-Za-z0-9])?$/.test(input.owner) || input.owner.includes("--") || typeof input.repo !== "string" || !/^[A-Za-z0-9_.-]{1,100}$/.test(input.repo) || input.repo === "." || input.repo === ".." || input.repo.endsWith(".") || /^(?:con|prn|aux|nul|com[0-9]|lpt[0-9])(?:\.|$)/i.test(input.repo)) reject("invalid-repository-identity", "owner/repo 不是安全、明确的 GitHub 仓库身份。");
    zoneId = `${input.owner}/${input.repo}`;
    if (input.repository_id !== zoneId || input.ssh_url !== `git@github.com:${zoneId}.git` || typeof input.head_commit !== "string" || !/^(?:[0-9a-f]{40}|[0-9a-f]{64})$/.test(input.head_commit)) reject("invalid-repository-identity", "仓库身份、SSH URL 或 HEAD 字段不一致。");
    if (!exact(input.checks, ["git_exit_code", "origin_matches", "path_matches", "is_shallow"]) || input.checks.git_exit_code !== 0 || input.checks.origin_matches !== true || input.checks.path_matches !== true || input.checks.is_shallow !== true) reject("unverified-clone", "G1 材料未声明完整成功核实事实。");
    const parent = absolute(input.parent_directory);
    path = absolute(input.repository_directory);
    if (!samePath(join(parent, input.repo), path)) reject("repository-path-mismatch", "仓库必须位于指定父目录的 repo 直接子目录。");
    stage = "verify_repository";
    const parentPath = await directory(parent);
    path = await directory(path);
    if (!samePath(join(parentPath, input.repo), path)) reject("repository-path-mismatch", "真实仓库路径不符合直接子目录要求。");
    await directory(join(path, ".git"));
    const before = await lstat(path);
    const checkDirectory = async () => {
      const actual = await directory(path!);
      const now = await lstat(actual);
      if (now.dev !== before.dev || now.ino !== before.ino) reject("repository-changed", "仓库目录在登记期间被替换。");
    };
    const git = Bun.which("git");
    if (!git) reject("git-unavailable", "后端 PATH 必须预先提供 Git。");
    const env = Object.fromEntries(Object.entries(process.env).filter(([key]) => !key.toUpperCase().startsWith("GIT_")));
    Object.assign(env, { GIT_CONFIG_NOSYSTEM: "1", GIT_CONFIG_GLOBAL: "/dev/null", GIT_TERMINAL_PROMPT: "0", GIT_OPTIONAL_LOCKS: "0" });
    const top = await query(git, path, ["rev-parse", "--show-toplevel"], env);
    const origin = await query(git, path, ["config", "--local", "--no-includes", "--get-all", "remote.origin.url"], env);
    const head = await query(git, path, ["rev-parse", "--verify", "HEAD^{commit}"], env);
    const shallow = await query(git, path, ["rev-parse", "--is-shallow-repository"], env);
    if (!samePath(top, path) || origin !== input.ssh_url || head !== input.head_commit || shallow !== "true") reject("repository-verification-failed", "实际 Git 根目录、origin、HEAD 或浅仓库事实与 G1 材料不符。");
    await checkDirectory();
    stage = "preflight";
    if (zones((await http(context.serviceUrl, "GET")).document).some(zone => zone.id === zoneId)) reject("zone-already-registered", "zone ID 已登记；不复用、覆盖、改绑或生成替代 ID。");
    await checkDirectory();
    stage = "register";
    postAttempted = true;
    effects.registration = "unknown";
    const posted = await http(context.serviceUrl, "POST", { id: zoneId, path });
    effects.registration = "succeeded";
    const returned = zones(posted.document).find(zone => zone.id === zoneId);
    if (!returned || !samePath(returned.path, path)) reject("registration-response-mismatch", "登记响应未包含预期绑定；不重投或回滚。");
    stage = "read_back";
    const readBack = zones((await http(context.serviceUrl, "GET")).document).find(zone => zone.id === zoneId);
    effects.observation = readBack ? "registered" : "not_registered";
    if (readBack) effects.binding = readBack;
    if (!readBack || !samePath(readBack.path, path)) reject("zone-readback-mismatch", "登记后的公开读回与预期绑定不一致。");
    await checkDirectory();
    stage = "material";
    effects.material = "unknown";
    const output = await context.host("material", { id: "registered_zone", document: {
      service_url: context.serviceUrl, zone_id: zoneId, repository_id: zoneId, repository_directory: path,
      ssh_url: input.ssh_url, head_commit: head, binding: readBack,
      checks: { repository_verified: true, post_status: posted.status, read_back_matches: true },
    } });
    if (output.id !== "registered_zone" || !isAbsolute(output.path)) reject("invalid-host-output", "宿主返回了无效 registered_zone 输出绑定。");
    effects.material = "succeeded";
    return { status: "success", outputs: [output] };
  } catch (error) {
    if (postAttempted && effects.observation === "not_attempted") {
      try {
        const found = zones((await http(context.serviceUrl, "GET")).document).find(zone => zone.id === zoneId);
        effects.observation = found ? "registered" : "not_registered";
        if (found) effects.binding = found;
        effects.expected_binding_matches = !!found && !!path && samePath(found.path, path);
      } catch (observation) { effects.observation = "unknown"; effects.observation_error = message(observation); }
    }
    if (object(error) && typeof error.path === "string") effects.material_path = error.path;
    return { status: "failure", outputs: [], failure: {
      stage, code: object(error) && "code" in error ? String(error.code) : "execution-failed", message: message(error),
      ...(error instanceof RegistrationError && error.details !== undefined ? { details: error.details } : {}), effects,
    } };
  }
}
