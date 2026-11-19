import { expect, test } from "bun:test";
import { link, mkdtemp, realpath, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";

// An isolated executable supplies Git's pipe bytes without using a repository or SSH.
const gitFixture = `
function emit(pipe, data) {
  const { promise, resolve, reject } = Promise.withResolvers();
  pipe.write(data, error => error ? reject(error) : resolve());
  return promise;
}
async function waitForParent(path) {
  const release = Bun.file(path);
  const deadline = Date.now() + 5_000;
  while (!await release.exists()) {
    if (Date.now() >= deadline) process.exit(98);
    // Poll a file written by the real reader; the elapsed time is only a dead-child guard.
    await Bun.sleep(5);
  }
}
const [out, err] = [process.stdout, process.stderr];
const args = process.argv.slice(2);
const clone = args.includes("clone");
const target = clone ? args.at(-1) : args[args.indexOf("-C") + 1];
if (clone) {
  if (!args.includes("--progress") || err.isTTY) process.exit(97);
  await emit(out, "clone out 1\\r");
  await emit(err, Buffer.concat([Buffer.from("clone err 1\\r"), Buffer.from([0xe9, 0x9b])]));
  await waitForParent(target + ".first");
  await emit(out, "\\nclone out 2\\r");
  await waitForParent(target + ".second");
  await emit(err, Buffer.concat([Buffer.from([0xaa]), Buffer.from("\\rclone err 2\\r\\n")]));
  if (target.endsWith("failure")) {
    await waitForParent(target + ".third");
    await emit(out, "a".repeat(65_536) + "\\r\\nend-out\\r");
    await waitForParent(target + ".fourth");
    await emit(err, "b".repeat(65_536) + "\\rend-err\\r\\n");
    process.exit(6);
  }
} else {
  await emit(err, "quiet validation diagnostic\\n");
  if (args.includes("--show-toplevel")) await emit(out, target + "\\n");
  else if (args.includes("--get-all")) await emit(out, "git@github.com:local/success.git\\n");
  else if (args.includes("--verify")) await emit(out, "0123456789012345678901234567890123456789\\n");
  else if (args.includes("--is-shallow-repository")) await emit(out, "true\\n");
  else process.exit(99);
}
`;

// Exercise the public script result and the actual inherited fd 2, with an explicit Bun exit.
const entry = pathToFileURL(resolve(import.meta.dir, "../meta-data/scripts/github-clone.ts")).href;
const runClone = `
import clone from ${JSON.stringify(entry)};
const [parent, repo] = process.argv.slice(2);
const materialPath = parent + "/" + repo + ".json";
const result = await clone({
  inputs: [{ id: "clone_request", path: parent + "/request.json" }],
  outputRule: [{ id: "cloned_repository" }],
  async host(operation, args) {
    if (operation === "read_json") return { github_url: "https://github.com/local/" + repo, parent_directory: parent };
    if (operation !== "material") throw new Error("unexpected host operation");
    await Bun.write(materialPath, JSON.stringify(args.document));
    return { id: args.id, path: materialPath };
  },
});
const { promise, resolve, reject } = Promise.withResolvers();
process.stdout.write(JSON.stringify(result), error => error ? reject(error) : resolve());
await promise;
process.exit(0);
`;

test.skipIf(process.platform !== "win32")("Git clone pipes stream live without polluting results, validation output or failure tails", async () => {
  const root = await mkdtemp(join(await realpath(tmpdir()), "github-clone-test-"));
  try {
    const fixture = join(root, "fixture.ts");
    const wrapper = join(root, "run-clone.ts");
    const git = join(root, "git.exe");
    await Promise.all([Bun.write(fixture, gitFixture), Bun.write(wrapper, runClone)]);
    const build = Bun.spawnSync([process.execPath, "build", "--compile", fixture, "--outfile", git], { cwd: root, timeout: 45_000 });
    if (build.exitCode !== 0) throw new Error(build.stderr.toString());
    // Only the fixture is reachable through this PATH; no real Git or SSH can run.
    await link(git, join(root, "ssh.exe"));
    const env = Object.fromEntries(Object.entries(process.env).filter(([name]) => name.toUpperCase() !== "PATH"));
    env.PATH = root;

    async function run(repo: string) {
      const child = Bun.spawn([process.execPath, wrapper, root, repo], {
        cwd: root, env, stdin: "ignore", stdout: "pipe", stderr: "pipe", signal: AbortSignal.timeout(15_000),
      });
      const gates = [
        { marker: "clone err 1\n", name: "first" },
        { marker: "clone out 2\n", name: "second" },
        ...(repo === "failure" ? [{ marker: "clone err 2\n", name: "third" }, { marker: "end-out\n", name: "fourth" }] : []),
      ];
      let display = "";
      let gate = 0;
      const result = new Response(child.stdout).text();
      const readLogs = (async () => {
        for await (const chunk of child.stderr.pipeThrough(new TextDecoderStream())) {
          display += chunk;
          const next = gates[gate];
          if (next && display.includes("clone out 1\n") && display.includes(next.marker)) {
            await Bun.write(join(root, `${repo}.${next.name}`), "ready");
            gate++;
          }
        }
      })();
      const [exit, stdout] = await Promise.all([child.exited, result, readLogs]);
      expect(exit).toBe(0);
      expect(gate).toBe(gates.length);
      // Cross-pipe ordering is unspecified; each pipe must keep its own order and bytes.
      expect(display).toContain("clone out 1\n");
      expect(display).toContain("clone out 2\n");
      expect(display).toContain("clone err 1\n");
      expect(display).toContain("雪\nclone err 2\n");
      expect(display).not.toMatch(/[\r\uFFFD]|\n\n|quiet validation diagnostic|git@github\.com:local\/success\.git/);
      return { result: JSON.parse(stdout), display };
    }

    const success = await run("success");
    expect(success.result).toEqual({ status: "success", outputs: [{ id: "cloned_repository", path: root + "/success.json" }] });
    expect(success.display.trimEnd().split("\n").sort()).toEqual(["clone err 1", "clone err 2", "clone out 1", "clone out 2", "雪"].sort());
    const failure = await run("failure");
    expect(failure.result).toMatchObject({ status: "failure", outputs: [], failure: { stage: "clone", code: "git-failed", details: {
      exit_code: 6, signal: null,
      stdout_tail: "a".repeat(65_526) + "\r\nend-out\r", stderr_tail: "b".repeat(65_526) + "\rend-err\r\n",
    } } });
    expect(failure.display).toContain("\nend-out\n");
    expect(failure.display).toContain("\nend-err\n");
  } finally {
    await rm(root, { recursive: true, force: true, maxRetries: 8, retryDelay: 200 });
  }
}, 80_000);
