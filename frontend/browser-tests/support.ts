/**
 * 真实浏览器验收的支撑设施：启动隔离的真实 herdr-bot 进程。
 *
 * 每个测试使用独立临时目录与 Task 存储文件；页面、公开 HTTP 接口与进程重启都来自交付二进制，
 * 不经过任何前端 mock。Herdr / agent 控制不在本票范围，固定操作指向安全夹具即可。
 */

import { spawn, type ChildProcess } from 'node:child_process'
import fs from 'node:fs'
import os from 'node:os'
import net from 'node:net'
import { randomInt } from 'node:crypto'
import path from 'node:path'
import readline from 'node:readline'
import { fileURLToPath } from 'node:url'

const here = path.dirname(fileURLToPath(import.meta.url))
const repoRoot = path.resolve(here, '../..')
const targetDirectory = process.env.CARGO_TARGET_DIR
  ? path.resolve(process.env.CARGO_TARGET_DIR)
  : path.join(repoRoot, 'backend', 'target')
const binary = path.join(targetDirectory, 'debug', 'herdr-bot.exe')
const fixtures = path.join(repoRoot, 'fixtures', 'operations')
const herdrFixtureBinary = path.join(targetDirectory, 'debug', 'herdr-fixture.exe')

export type TaskView = {
  id: string
  name: string
  chatName: string
  agent: string
  taskType: string
  model: string
  thinking: string
  skill: string | null
  prompt: string
  extensions: string[]
}

/** 受控 Herdr 副本的初始数据；它只在自动浏览器验收中使用。 */
export type HerdrFixtureSetup = {
  workspaceName?: string
  workspaces?: unknown[]
  behavior?: Record<string, unknown>
}

export type StartServerOptions = {
  /** Existing record-only PowerOff shell; required for tests sending shutdown commands. */
  safeShutdown?: boolean
  /** Keep the real process cwd separate from its configuration file. */
  separateCwd?: boolean
  herdr?: HerdrFixtureSetup
  /** Axum 数值监听地址；普通入口只允许 127/8 或 [::1]，非回环仅供 startManualServer。 */
  listenAddr?: string
}

export type E2EServer = {
  baseURL: string
  /** 同一进程在指定主机名/IP 上的入口；用来真实请求局域网地址。 */
  urlForHost: (host: string) => string
  storageFile: string
  metadataFile: string
  configPath: string
  cwd: string
  /** Actual delivered backend stdout/stderr; diagnostic observation only. */
  diagnostics: () => string
  getTasks: () => Promise<{ tasks: TaskView[]; options: unknown }>
  /** 受控 Herdr 自己写下的外部调用记录。 */
  herdrCalls: () => Record<string, unknown>[]
  /** 受控 Herdr 自己写下的新建 Tab 记录。 */
  herdrTabs: () => Record<string, unknown>[]
  setHerdrBehavior: (behavior: Record<string, unknown>) => void
  stop: () => Promise<void>
  restart: () => Promise<void>
  cleanup: () => Promise<void>
}

/** Update scheduler fixture settings without introducing a second TOML table. */
export function configureScheduler(server: E2EServer, values: Record<string, string | number>): void {
  const lines = fs.readFileSync(server.configPath, 'utf8').split('\n')
  const start = lines.findIndex(line => line.trim() === '[scheduler]')
  if (start < 0) throw new Error('Missing scheduler fixture configuration')
  let end = start + 1
  while (end < lines.length && !lines[end].trim().startsWith('[')) end++
  for (const [key, value] of Object.entries(values)) {
    const setting = `${key} = ${typeof value === 'number' ? value : `'${value}'`}`
    const index = lines.findIndex((line, index) => index > start && index < end && line.split('=')[0].trim() === key)
    if (index >= 0) lines[index] = setting
    else lines.splice(end++, 0, setting)
  }
  fs.writeFileSync(server.configPath, lines.join('\n'))
}

/**
 * 本机可用于局域网访问的 IPv4；没有时返回 null（调用方跳过用例）。
 *
 * 优先取常见私网地址，排除回环与链路本地地址——后者不是手机日常访问的入口。
 */
export function lanIPv4(): string | null {
  const candidates: string[] = []
  for (const entries of Object.values(os.networkInterfaces())) {
    for (const entry of entries ?? []) {
      if (entry.family !== 'IPv4' || entry.internal) continue
      if (entry.address.startsWith('169.254.')) continue
      candidates.push(entry.address)
    }
  }
  const privateAddress = candidates.find(
    (address) =>
      address.startsWith('192.168.') ||
      address.startsWith('10.') ||
      /^172\.(1[6-9]|2\d|3[01])\./.test(address),
  )
  return privateAddress ?? candidates[0] ?? null
}

// Windows may allocate port 0 inside Fetch/Chrome's forbidden service-port range
// (for example 6566 or 6667). Probe the unrestricted dynamic range instead.
async function browserListenAddress(address: string): Promise<string> {
  if (!address.endsWith(':0')) return address
  const host = address.slice(0, -2)
  const bindHost = host.replace(/^\[|\]$/g, '')
  for (let attempt = 0; attempt < 32; attempt++) {
    const port = randomInt(49152, 65536)
    const server = net.createServer()
    const { promise, resolve, reject } = Promise.withResolvers<boolean>()
    server.once('error', (error: NodeJS.ErrnoException) => {
      if (error.code === 'EADDRINUSE' || error.code === 'EACCES') resolve(false)
      else reject(error)
    })
    server.listen(port, bindHost, () => server.close(error => error ? reject(error) : resolve(true)))
    if (await promise) return `${host}:${port}`
  }
  throw new Error('无法为浏览器夹具分配安全的动态端口')
}

function writeConfig(
  directory: string,
  storage: string,
  herdr: { workspaceName: string; executable?: string },
  listenAddr: string,
  cwd: string,
): string {
  const configPath = path.join(directory, 'herdr-bot.toml')
  const herdrLines = ['[herdr]', `workspace_name = '${herdr.workspaceName}'`]
  if (herdr.executable !== undefined) {
    herdrLines.push(`executable = '${herdr.executable}'`)
  }
  fs.writeFileSync(
    configPath,
    [
      `listen_addr = "${listenAddr}"`,
      `paths_file = '${path.join(directory, 'paths.toml')}'`,
      `static_directory = '${path.join(repoRoot, 'backend', 'static')}'`,
      `data_directory = '${path.join(cwd, '.data')}'`,
      '',
      '[scheduler]',
      `map_directory = '${path.join(cwd, 'meta-data', 'workflow-maps')}'`,
      `bot_directory = '${path.join(cwd, 'meta-data', 'bots')}'`,
      `run_directory = '${path.join(cwd, '.data', 'workflow-runs')}'`,
      `run_instructions = '${path.join(repoRoot, 'docs', 'workflow-runtime.md')}'`,
      '',
      '[operations.start]',
      `script = '${path.join(fixtures, 'success.bat')}'`,
      `working_directory = '${fixtures}'`,
      '',
      '[operations.stop]',
      `script = '${path.join(fixtures, 'success.bat')}'`,
      `working_directory = '${fixtures}'`,
      '',
      '[sessions]',
      `history_file = '${path.join(directory, 'session-history.json')}'`,
      '',
      '[mdns]',
      'enabled = false',
      '',
      ...herdrLines,
      '',
      '[zones]',
      `registry_file = '${path.join(directory, 'zones.json')}'`,
      '',
      '[tasks]',
      `storage_file = '${storage}'`,
      `metadata_file = '${path.join(directory, 'task-options.toml')}'`,
      '',
    ].join('\n'),
    'utf8',
  )
  return configPath
}

/** 读取受控 Herdr 的 JSONL 记录；文件尚不存在时返回空列表。 */
function readHerdrRecords(file: string): Record<string, unknown>[] {
  if (!fs.existsSync(file)) return []
  return fs
    .readFileSync(file, 'utf8')
    .split('\n')
    .filter((line) => line.trim() !== '')
    .map((line) => JSON.parse(line) as Record<string, unknown>)
}

async function waitForTasks(baseURL: string): Promise<void> {
  const deadline = Date.now() + 20_000
  while (Date.now() < deadline) {
    try {
      const response = await fetch(`${baseURL}/api/tasks`)
      if (response.ok) return
    } catch {
      // 进程尚未开始监听。
    }
    await new Promise((resolve) => setTimeout(resolve, 50))
  }
  throw new Error(`herdr-bot 没有在 20 秒内开始提供 Task 接口：${baseURL}`)
}

type ServerProcess = {
  child: ChildProcess
  port: number
  diagnostics: () => string
  listening: Promise<void>
}

function startProcess(configPath: string, executable: string, cwd: string): ServerProcess {
  const child = spawn(executable, ['--config', configPath], {
    cwd,
    // The copied fixture must never inherit a developer's external data root.
    env: {
      ...process.env,
      HERDR_FIXTURE_DIR: path.join(path.dirname(configPath), 'herdr'),
      HERDR_BOT_FIXTURE_RECORD_DIR: path.join(path.dirname(configPath), 'operations'),
    },
    stdio: ['ignore', 'pipe', 'pipe'],
    windowsHide: true,
  })
  const log: string[] = []
  let port = 0
  const listening = Promise.withResolvers<void>()
  let settled = false
  const timer = setTimeout(() => {
    if (settled) return
    settled = true
    listening.reject(new Error(`herdr-bot 没有在 30 秒内打印监听端口：\n${log.join('')}`))
  }, 30_000)

  const reader = readline.createInterface({ input: child.stdout })
  reader.on('line', (line) => {
    log.push(`${line}\n`)
    const match = line.match(/http:\/\/([^\s/]+):(\d+)\//)
    if (match === null || settled) return
    settled = true
    port = Number(match[2])
    clearTimeout(timer)
    listening.resolve()
  })
  child.stderr.on('data', (chunk: Buffer) => {
    log.push(chunk.toString('utf8'))
  })
  child.on('error', (error) => {
    if (settled) return
    settled = true
    clearTimeout(timer)
    listening.reject(error)
  })
  child.on('exit', (code) => {
    if (settled) return
    settled = true
    clearTimeout(timer)
    listening.reject(new Error(`herdr-bot 提前退出（code ${code}）：\n${log.join('')}`))
  })
  return { child, get port() { return port }, diagnostics: () => log.join(''), listening: listening.promise }
}

/** 普通浏览器验收只允许数值回环地址，不解析 hostname。 */
export async function startServer(options: StartServerOptions = {}): Promise<E2EServer> {
  const address = options.listenAddr ?? '127.0.0.1:0'
  const match = /^(\[[^\]]+\]|[^:]+):(\d+)$/.exec(address)
  const host = match?.[1].replace(/^\[|\]$/g, '') ?? ''
  const family = net.isIP(host)
  const loopback = family === 4
    ? host.startsWith('127.')
    : family === 6 && new URL(`http://[${host}]`).hostname === '[::1]'
  if (!loopback || Number(match?.[2]) > 65535) {
    throw new Error(`普通浏览器测试只允许数值回环监听地址，收到 ${address}；真实网卡验收请运行 npm run test:browser:manual。`)
  }
  return startServerImpl(options, address)
}

/** 仅显式手动验收入口授权真实网卡监听；普通 helper 永不接受此授权。 */
export async function startManualServer(options: StartServerOptions = {}): Promise<E2EServer> {
  if (process.env.HERDR_BOT_MANUAL_NETWORK !== '1') {
    throw new Error('真实网卡验收需要 HERDR_BOT_MANUAL_NETWORK=1；请运行 npm run test:browser:manual。')
  }
  return startServerImpl(options, options.listenAddr ?? '127.0.0.1:0')
}

async function startServerImpl(options: StartServerOptions, listenAddr: string): Promise<E2EServer> {
  const listenHost = listenAddr.slice(0, listenAddr.lastIndexOf(':'))
  const localHost = listenHost === '0.0.0.0' ? '127.0.0.1' : listenHost === '[::]' ? '[::1]' : listenHost
  const executable = options.safeShutdown
    ? path.join(targetDirectory, 'debug', 'examples', 'safe_shutdown_shell.exe')
    : binary
  if (!fs.existsSync(executable)) {
    throw new Error(
      `缺少验收二进制 ${executable}；请先在 backend 目录执行 cargo build --bins --example safe_shutdown_shell。`,
    )
  }
  if (!fs.existsSync(path.join(repoRoot, 'backend', 'static', 'index.html'))) {
    throw new Error('缺少打包页面；请先在 frontend 目录执行 npm run build。')
  }

  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'herdr-bot browser '))
  const storageFile = path.join(directory, 'tasks.json')
  const metadataFile = path.join(directory, 'task-options.toml')
  const herdrDirectory = path.join(directory, 'herdr')
  let herdrExecutable: string | undefined
  let running: ServerProcess | null = null

  const cwd = options.separateCwd ? path.join(directory, 'process-cwd') : directory
  try {
    fs.mkdirSync(cwd, { recursive: true })
    // Real isolated entry files: browser acceptance must not depend on a developer toolkit installation.
    const extensionDirectory = path.join(directory, 'extensions')
    fs.mkdirSync(extensionDirectory)
    fs.writeFileSync(path.join(directory, 'paths.toml'), `[roots]\nbrowser_extensions = '${extensionDirectory}'\n`)
    const metadata = fs.readFileSync(path.join(repoRoot, 'meta-data', 'task-options.toml'), 'utf8')
    fs.writeFileSync(metadataFile, metadata.replace(/(\[\[extensions\]\][\s\S]*?entrypoint = )'[^']+'/g, (_line, prefix: string) => {
      const id = prefix.match(/^id = '([^']+)'/m)![1]
      const relative = `${id}.ts`
      const entrypoint = path.join(extensionDirectory, relative)
      fs.writeFileSync(entrypoint, `import fs from 'node:fs'; fs.writeFileSync(${JSON.stringify(path.join(directory, 'extension-executed'))}, ${JSON.stringify(relative)}); throw new Error('FILE CHECK MUST NOT EXECUTE')\n`)
      return `${prefix}'\${roots.browser_extensions}/${relative}'`
    }))
    if (options.herdr !== undefined) {
      if (!fs.existsSync(herdrFixtureBinary)) {
        throw new Error(
          `缺少受控 Herdr 夹具 ${herdrFixtureBinary}；请先在 backend 目录执行 cargo build。`,
        )
      }
      fs.mkdirSync(herdrDirectory, { recursive: true })
      herdrExecutable = path.join(herdrDirectory, 'herdr-fixture.exe')
      fs.copyFileSync(herdrFixtureBinary, herdrExecutable)
      fs.writeFileSync(
        path.join(herdrDirectory, 'workspaces.json'),
        JSON.stringify(
          options.herdr.workspaces ?? [
            { workspace_id: 'w-other', label: 'other', focused: true, cwd: 'C:\\elsewhere\\focused' },
            { workspace_id: 'w-target', label: 'bot-playground', focused: false },
            { workspace_id: 'w-decoy', label: 'bot-playground-extra', focused: false },
          ],
        ),
        'utf8',
      )
      fs.writeFileSync(
        path.join(herdrDirectory, 'behavior.json'),
        JSON.stringify(options.herdr.behavior ?? {}),
        'utf8',
      )
    }

    const configPath = writeConfig(
      directory,
      storageFile,
      {
        workspaceName: options.herdr?.workspaceName ?? 'bot-playground',
        executable: herdrExecutable,
      },
      await browserListenAddress(listenAddr),
      cwd,
    )

    async function start() {
      for (let attempt = 0; ; attempt++) {
        running = startProcess(configPath, executable, cwd)
        try {
          await running.listening
        } catch (error) {
          // The probe cannot reserve its port across process creation. Only a
          // confirmed bind collision may allocate another dynamic fixture port;
          // fixed addresses and every other startup failure remain failures.
          const bindCollision = /绑定监听地址失败：[\s\S]*os error (?:10048|98|48)\)/.test(running.diagnostics())
          if (!listenAddr.endsWith(':0') || !bindCollision || attempt >= 31) throw error
          const address = await browserListenAddress(listenAddr)
          const config = fs.readFileSync(configPath, 'utf8')
          fs.writeFileSync(configPath, config.replace(/^listen_addr = "[^"]*"/m, `listen_addr = "${address}"`))
          continue
        }
        await waitForTasks(`http://${localHost}:${running.port}`)
        return
      }
    }

    const server: E2EServer = {
      get baseURL() {
        if (running === null) throw new Error('herdr-bot 进程未运行')
        return `http://${localHost}:${running.port}`
      },
      urlForHost(host: string) {
        if (running === null) throw new Error('herdr-bot 进程未运行')
        return `http://${host}:${running.port}`
      },
      storageFile,
      metadataFile,
      configPath,
      cwd,
      diagnostics() { return running?.diagnostics() ?? '' },
      async getTasks() {
        const response = await fetch(`${this.baseURL}/api/tasks`)
        if (!response.ok) {
          throw new Error(`Task 列表读取失败：HTTP ${response.status}`)
        }
        return (await response.json()) as { tasks: TaskView[]; options: unknown }
      },
      herdrCalls() {
        return readHerdrRecords(path.join(herdrDirectory, 'calls.jsonl'))
      },
      herdrTabs() {
        return readHerdrRecords(path.join(herdrDirectory, 'tabs.jsonl'))
      },
      setHerdrBehavior(behavior: Record<string, unknown>) {
        fs.mkdirSync(herdrDirectory, { recursive: true })
        fs.writeFileSync(
          path.join(herdrDirectory, 'behavior.json'),
          JSON.stringify(behavior),
          'utf8',
        )
      },
      stop,
      async restart() {
        await stop()
        try { await start() }
        catch (error) {
          await cleanupAfterFailure(error)
        }
      },
      cleanup,
    }
    await start()
    return server
  } catch (error) {
    return cleanupAfterFailure(error)
  }

  async function cleanupAfterFailure(error: unknown): Promise<never> {
    try { await cleanup() }
    catch (cleanupError) {
      throw new AggregateError([error, cleanupError], '浏览器测试服务器初始化失败且资源清理失败', { cause: error })
    }
    throw error
  }

  async function stop() {
    const child = running?.child
    if (child === undefined) return
    if (child.pid === undefined || child.exitCode !== null || child.signalCode !== null) {
      running = null
      return
    }
    const exited = Promise.withResolvers<void>()
    child.once('exit', () => exited.resolve())
    // Give Node a full confirmation window after the termination command has
    // completed; taskkill itself can take longer than this under global load.
    let exitTimer: NodeJS.Timeout | undefined
    try {
      if (process.platform === 'win32') {
        // Independent identity queries are real child processes. Killing only
        // their parent leaves a live copied EXE holding the test directory open.
        const killed = Promise.withResolvers<number | null>()
        const killer = spawn('taskkill.exe', ['/PID', String(child.pid), '/T', '/F'], { windowsHide: true, stdio: 'ignore' })
        killer.once('error', killed.reject)
        killer.once('exit', killed.resolve)
        const code = await killed.promise
        exitTimer = setTimeout(() => exited.resolve(), 5000)
        // taskkill can observe an already-exited process before Node delivers
        // its exit event. Wait for that event before treating a nonzero code
        // as a cleanup failure; do not hide a process that actually remains alive.
        await exited.promise
        if (code !== 0 && child.exitCode === null && child.signalCode === null) {
          throw new Error(`taskkill failed: ${code}`)
        }
      } else {
        child.kill()
        exitTimer = setTimeout(() => exited.resolve(), 5000)
      }
      await exited.promise
      if (child.exitCode === null && child.signalCode === null) {
        throw new Error('herdr-bot 测试进程没有在 5 秒内退出')
      }
      running = null
    } finally {
      clearTimeout(exitTimer)
    }
  }

  async function cleanup() {
    await stop()
    if (process.platform === 'win32' && herdrExecutable !== undefined) {
      // taskkill /T can miss an identity reader spawned while it snapshots the
      // tree. The backend is now stopped, so no new readers can appear. Reap
      // only this fixture's copied executable and wait for its handles to close.
      // Get-Process reads the local process table without initializing CIM; still
      // re-open each candidate by PID and verify its full executable path.
      const reaped = Promise.withResolvers<void>()
      const reaper = spawn('powershell.exe', ['-NoProfile', '-Command',
          "$ErrorActionPreference = 'Stop'; $candidates = @(Get-Process -Name 'herdr-fixture' -ErrorAction SilentlyContinue); $candidates | Where-Object { $_.Path -eq $env:HERDR_FIXTURE_EXECUTABLE } | ForEach-Object { $p = Get-Process -Id $_.Id -ErrorAction SilentlyContinue; if ($p -and $p.Path -eq $env:HERDR_FIXTURE_EXECUTABLE) { Stop-Process -InputObject $p -ErrorAction SilentlyContinue; $p.WaitForExit() } }",
        ], { windowsHide: true, stdio: 'ignore', env: { ...process.env, HERDR_FIXTURE_EXECUTABLE: herdrExecutable } })
      reaper.once('error', reaped.reject)
      reaper.once('exit', (code) => code === 0 ? reaped.resolve() : reaped.reject(new Error(`fixture cleanup failed: ${code}`)))
      await reaped.promise
    }
    try {
      fs.chmodSync(storageFile, 0o666)
    } catch {
      // 文件可能不存在。
    }
    fs.rmSync(directory, { recursive: true, force: true })
  }
}
