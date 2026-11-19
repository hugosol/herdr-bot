import { spawn, spawnSync } from "node:child_process";
import { lstat, mkdir, realpath } from "node:fs/promises";
import type { Stats } from "node:fs";
import { isAbsolute, join, parse, resolve } from "node:path";

type FileBinding = { id: string; path: string };
type Context = {
  inputs?: FileBinding[];
  outputRule?: { id: string }[];
  host(operation: "read_json", args: { path: string }): Promise<unknown>;
  host(operation: "material", args: { id: string; document: unknown }): Promise<FileBinding>;
};
const TIMEOUT_MS = 600_000;
const LOG_BYTES = 65_536;

class CloneError extends Error {
  constructor(readonly code: string, message: string, readonly details?: unknown) {
    super(message);
  }
}
function reject(code: string, message: string): never { throw new CloneError(code, message); }
function message(error: unknown): string { return error instanceof Error ? error.message : String(error); }
function code(error: unknown): string | undefined {
  return error && typeof error === "object" && "code" in error ? String(error.code) : undefined;
}
function repository(input: unknown) {
  if (typeof input !== "string") reject("invalid-request", "github_url 必须是字符串。");
  const match = /^(?:https:\/\/github\.com\/|git@github\.com:|ssh:\/\/git@github\.com\/)([A-Za-z0-9](?:[A-Za-z0-9-]{0,37}[A-Za-z0-9])?)\/([A-Za-z0-9_.-]+)$/.exec(input);
  if (!match || match[1].includes("--")) reject("invalid-url", "仅接受无查询、片段或子路径的 GitHub 仓库 HTTPS／SSH 地址。");
  const owner = match[1];
  const repo = match[2].replace(/\.git$/, "");
  if (!repo || repo === "." || repo === ".." || repo.endsWith(".") || repo.length > 100 || /^(?:con|prn|aux|nul|com[0-9]|lpt[0-9])(?:\.|$)/i.test(repo)) {
    reject("unsafe-repository-name", "仓库名无法安全表示为本机目录。");
  }
  return { owner, repo, repository_id: `${owner}/${repo}`, ssh_url: `git@github.com:${owner}/${repo}.git` };
}
function samePath(a: string, b: string) {
  return resolve(a).toLowerCase() === resolve(b).toLowerCase();
}
async function directory(path: string): Promise<string> {
  const absolute = resolve(path);
  let current = parse(absolute).root;
  for (const part of absolute.slice(current.length).split(/[\\/]/).filter(Boolean)) {
    current = join(current, part);
    const stat = await lstat(current);
    if (!stat.isDirectory() || stat.isSymbolicLink()) reject("unsafe-path", `目录不存在、不是普通目录或包含链接：${current}`);
    if (!samePath(await realpath(current), current)) reject("unsafe-path", `目录被重定向：${current}`);
  }
  const stat = await lstat(absolute);
  if (!stat.isDirectory() || stat.isSymbolicLink()) reject("unsafe-path", `不是普通目录：${absolute}`);
  return realpath(absolute);
}
function tail(previous: Buffer, chunk: Buffer): Buffer {
  if (chunk.length >= LOG_BYTES) return Buffer.from(chunk.subarray(-LOG_BYTES));
  return Buffer.concat([previous.subarray(Math.max(0, previous.length + chunk.length - LOG_BYTES)), chunk]);
}

// The Git process must finish before publishing a result; timeout kills its Windows process tree.
async function gitCommand(executable: string, args: string[], cwd: string, env: NodeJS.ProcessEnv, deadline: number) {
  const remaining = deadline - Date.now();
  if (remaining <= 0) reject("clone-timeout", "本次操作已超过 10 分钟。");
  const { promise, resolve: accept, reject: fail } = Promise.withResolvers<string>();
  let stdout = Buffer.alloc(0), stderr = Buffer.alloc(0);
  let timedOut = false;
  let terminationError: string | undefined;
  const child = spawn(executable, args, { cwd, env, shell: false, windowsHide: true, stdio: ["ignore", "pipe", "pipe"] });
  child.stdout.on("data", (data: Buffer) => { stdout = tail(stdout, data); });
  child.stderr.on("data", (data: Buffer) => { stderr = tail(stderr, data); });
  const timer = setTimeout(() => {
    timedOut = true;
    if (child.pid) {
      const kill = spawnSync(join(process.env.SystemRoot || "C:\\Windows", "System32", "taskkill.exe"), ["/PID", String(child.pid), "/T", "/F"], { windowsHide: true, timeout: 5000, encoding: "utf8" });
      if (kill.error || kill.status !== 0) {
        terminationError = kill.error ? message(kill.error) : `${kill.stdout}\n${kill.stderr}`;
        child.kill("SIGKILL");
        child.stdout.destroy();
        child.stderr.destroy();
      }
    }
  }, remaining);
  child.on("error", error => { clearTimeout(timer); fail(error); });
  child.on("close", (exitCode, signal) => {
    clearTimeout(timer);
    const details = { exit_code: exitCode, signal, stdout_tail: stdout.toString("utf8"), stderr_tail: stderr.toString("utf8"), termination_error: terminationError };
    if (timedOut) fail(new CloneError("clone-timeout", "本次操作超过 10 分钟，已尝试终止进程树；不清理残留。", details));
    else if (exitCode !== 0) fail(new CloneError("git-failed", `Git 失败（退出码 ${exitCode}）。`, details));
    else accept(stdout.toString("utf8").trim());
  });
  return promise;
}

export default async function githubClone(context: Context) {
  let stage = "validate";
  let target: string | undefined;
  let created: Stats | undefined;
  const effects: Record<string, unknown> = {
    target_directory_created: false, clone: "not_attempted", verification: "not_attempted", material: "not_attempted",
  };
  try {
    if (process.platform !== "win32") reject("unsupported-platform", "本脚本已实现的进程树终止契约仅支持 Windows 后端。");
    if (!Array.isArray(context.inputs) || context.inputs.length !== 1 || context.inputs[0].id !== "clone_request" || !isAbsolute(context.inputs[0].path)) {
      reject("invalid-input-bindings", "必须且只能绑定一个绝对地址输入 clone_request。");
    }
    if (!Array.isArray(context.outputRule) || context.outputRule.length !== 1 || context.outputRule[0].id !== "cloned_repository") {
      reject("invalid-output-rules", "必须且只能声明输出 cloned_repository。");
    }
    const request = await context.host("read_json", { path: context.inputs[0].path });
    if (!request || typeof request !== "object" || Array.isArray(request) || Object.keys(request).length !== 2 || !("github_url" in request) || !("parent_directory" in request)) {
      reject("invalid-request", "clone_request 仅接受 github_url 与 parent_directory 两个字段。");
    }
    const identity = repository(request.github_url);
    const parent = request.parent_directory;
    if (typeof parent !== "string" || !isAbsolute(parent) || !/^(?:[A-Za-z]:[\\/]|\\\\[^\\/]+[\\/][^\\/]+)/.test(parent) || /[\x00-\x1f]/.test(parent) || parent.startsWith("\\\\?\\") || parent.startsWith("\\\\.\\") || parent.split(/[\\/]/).some(part => part === "." || part === ".." || /[. ]$/.test(part)) || /:/.test(parent.slice(2))) {
      reject("invalid-parent", "parent_directory 必须是无设备前缀、路径穿越或歧义分量的绝对目录路径。");
    }
    stage = "preflight";
    const parent_directory = await directory(parent);
    target = join(parent_directory, identity.repo);
    effects.repository_directory = target;
    try { await lstat(target); reject("target-exists", `目标已有占用，不复用或覆盖：${target}`); }
    catch (error) { if (code(error) !== "ENOENT") throw error; }
    const git = Bun.which("git");
    const ssh = Bun.which("ssh");
    if (!git || !ssh) reject("tool-unavailable", "后端 PATH 必须提供 Git 与 SSH。");
    // Ignore inherited Git overrides, global filters, URL rewrites and templates without changing user configuration.
    const env = Object.fromEntries(Object.entries(process.env).filter(([key]) => !key.toUpperCase().startsWith("GIT_")));
    Object.assign(env, {
      GIT_CONFIG_NOSYSTEM: "1", GIT_CONFIG_GLOBAL: "/dev/null", GIT_TERMINAL_PROMPT: "0", GIT_LFS_SKIP_SMUDGE: "1",
      GIT_SSH_VARIANT: "ssh",
      GIT_SSH_COMMAND: `"${ssh.replaceAll("\\", "/")}" -o BatchMode=yes -o StrictHostKeyChecking=yes -o UpdateHostKeys=no -o PermitLocalCommand=no -o RemoteCommand=none -o ClearAllForwardings=yes`,
    });
    stage = "reserve";
    await directory(parent_directory);
    try { await mkdir(target); }
    catch (error) { if (code(error) === "EEXIST") reject("target-exists", `目标被占用，不复用或覆盖：${target}`); throw error; }
    effects.target_directory_created = true;
    created = await lstat(target);
    const checkTarget = async () => {
      const actual = await directory(target!);
      const now = await lstat(target!);
      if (!samePath(actual, target!) || now.dev !== created!.dev || now.ino !== created!.ino) reject("target-changed", "目标目录在执行期间被替换。");
      return actual;
    };
    await checkTarget();
    stage = "clone";
    effects.clone = "attempted";
    const deadline = Date.now() + TIMEOUT_MS;
    const config = ["-c", "core.hooksPath=/dev/null", "-c", "protocol.allow=never", "-c", "protocol.ssh.allow=always"];
    await gitCommand(git, [...config, "clone", "--depth=1", "--no-recurse-submodules", "--template=", "--", identity.ssh_url, target], parent_directory, env, deadline);
    effects.clone = "succeeded";
    stage = "verify";
    effects.verification = "attempted";
    const repository_directory = await checkTarget();
    const query = (args: string[]) => gitCommand(git, [...config, "-C", repository_directory, ...args], parent_directory, env, deadline);
    const top = await query(["rev-parse", "--show-toplevel"]);
    const origin = await query(["config", "--local", "--get-all", "remote.origin.url"]);
    const head_commit = await query(["rev-parse", "--verify", "HEAD^{commit}"]);
    const shallow = await query(["rev-parse", "--is-shallow-repository"]);
    if (!samePath(top, repository_directory) || origin !== identity.ssh_url || !/^(?:[0-9a-f]{40}|[0-9a-f]{64})$/.test(head_commit) || shallow !== "true") {
      reject("verification-failed", "实际仓库路径、origin、HEAD 或浅仓库事实不符合要求。");
    }
    await checkTarget();
    effects.verification = "succeeded";
    const document = { ...identity, parent_directory, repository_directory, head_commit, checks: { git_exit_code: 0, origin_matches: true, path_matches: true, is_shallow: true } };
    stage = "materials";
    effects.material = "attempted";
    const output = await context.host("material", { id: "cloned_repository", document });
    if (output.id !== "cloned_repository" || !isAbsolute(output.path)) reject("invalid-host-output", "宿主返回了无效输出绑定。");
    effects.material = "succeeded";
    return { status: "success", outputs: [output] };
  } catch (error) {
    if (stage === "clone" && effects.clone === "attempted") effects.clone = "failed";
    if (stage === "verify") effects.verification = "failed";
    if (stage === "materials") {
      effects.material = "unknown";
      if (error && typeof error === "object" && "path" in error) effects.material_path = String(error.path);
    }
    if (target) {
      try {
        const stat = await lstat(target);
        effects.target_observation = { exists: true, directory: stat.isDirectory(), link: stat.isSymbolicLink(), same_reserved_directory: created ? stat.dev === created.dev && stat.ino === created.ino : null };
      } catch (observation) {
        effects.target_observation = code(observation) === "ENOENT" ? { exists: false } : { exists: "unknown", error: message(observation) };
      }
    }
    return { status: "failure", outputs: [], failure: {
      stage, code: code(error) ?? "execution-failed", message: message(error),
      ...(error instanceof CloneError && error.details ? { details: error.details } : {}), effects,
    } };
  }
}
