/**
 * 真实浏览器验收的支撑设施：启动隔离的真实 herdr-bot 进程。
 *
 * 每个测试使用独立临时目录与 Bot 存储文件；页面、公开 HTTP 接口与进程重启都来自交付二进制，
 * 不经过任何前端 mock。Herdr / agent 控制不在本票范围，固定操作指向安全夹具即可。
 */

import { spawn, type ChildProcessWithoutNullStreams } from 'node:child_process'
import fs from 'node:fs'
import os from 'node:os'
import net from 'node:net'
import { randomInt } from 'node:crypto'
import path from 'node:path'
import readline from 'node:readline'
import { fileURLToPath } from 'node:url'

const here = path.dirname(fileURLToPath(import.meta.url))
const repoRoot = path.resolve(here, '../..')
const binary = path.join(repoRoot, 'backend', 'target', 'debug', 'herdr-bot.exe')
const fixtures = path.join(repoRoot, 'fixtures', 'operations')
const herdrFixtureBinary = path.join(repoRoot, 'backend', 'target', 'debug', 'herdr-fixture.exe')

export type BotView = {
  id: string
  name: string
  chatName: string
  agent: string
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
  herdr?: HerdrFixtureSetup
  /** Axum 监听地址；默认只监听回环。局域网用例传 `0.0.0.0:0`。 */
  listenAddr?: string
}

export type E2EServer = {
  baseURL: string
  /** 同一进程在指定主机名/IP 上的入口；用来真实请求局域网地址。 */
  urlForHost: (host: string) => string
  storageFile: string
  metadataFile: string
  configPath: string
  /** Actual delivered backend stdout/stderr; diagnostic observation only. */
  diagnostics: () => string
  getBots: () => Promise<{ bots: BotView[]; options: unknown }>
  /** 受控 Herdr 自己写下的外部调用记录。 */
  herdrCalls: () => Record<string, unknown>[]
  /** 受控 Herdr 自己写下的新建 Tab 记录。 */
  herdrTabs: () => Record<string, unknown>[]
  setHerdrBehavior: (behavior: Record<string, unknown>) => void
  stop: () => Promise<void>
  restart: () => Promise<void>
  cleanup: () => Promise<void>
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
  for (let attempt = 0; attempt < 32; attempt++) {
    const port = randomInt(49152, 65536)
    const server = net.createServer()
    const { promise, resolve, reject } = Promise.withResolvers<boolean>()
    server.once('error', (error: NodeJS.ErrnoException) => {
      if (error.code === 'EADDRINUSE' || error.code === 'EACCES') resolve(false)
      else reject(error)
    })
    server.listen(port, host, () => server.close(error => error ? reject(error) : resolve(true)))
    if (await promise) return `${host}:${port}`
  }
  throw new Error('无法为浏览器夹具分配安全的动态端口')
}

function writeConfig(
  directory: string,
  storage: string,
  herdr: { workspaceName: string; executable?: string },
  listenAddr: string,
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
      '[bots]',
      `storage_file = '${storage}'`,
      `metadata_file = '${path.join(directory, 'bot-options.toml')}'`,
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

async function waitForBots(baseURL: string): Promise<void> {
  const deadline = Date.now() + 20_000
  while (Date.now() < deadline) {
    try {
      const response = await fetch(`${baseURL}/api/bots`)
      if (response.ok) return
    } catch {
      // 进程尚未开始监听。
    }
    await new Promise((resolve) => setTimeout(resolve, 50))
  }
  throw new Error(`herdr-bot 没有在 20 秒内开始提供 Bot 接口：${baseURL}`)
}

function startProcess(configPath: string, executable: string): Promise<{ child: ChildProcessWithoutNullStreams; port: number; diagnostics: () => string }> {
  const child = spawn(executable, ['--config', configPath], {
    cwd: path.dirname(configPath),
    stdio: ['ignore', 'pipe', 'pipe'],
    windowsHide: true,
  })

  return new Promise((resolve, reject) => {
    let settled = false
    const log: string[] = []
    const timer = setTimeout(() => {
      if (settled) return
      settled = true
      child.kill()
      reject(new Error(`herdr-bot 没有在 30 秒内打印监听端口：\n${log.join('')}`))
    }, 30_000)

    const reader = readline.createInterface({ input: child.stdout })
    reader.on('line', (line) => {
      log.push(`${line}\n`)
      const match = line.match(/http:\/\/([^\s/]+):(\d+)\//)
      if (match === null || settled) return
      settled = true
      clearTimeout(timer)
      resolve({ child, port: Number(match[2]), diagnostics: () => log.join('') })
    })
    child.stderr.on('data', (chunk: Buffer) => {
      log.push(chunk.toString('utf8'))
    })
    child.on('error', (error) => {
      if (settled) return
      settled = true
      clearTimeout(timer)
      reject(error)
    })
    child.on('exit', (code) => {
      if (settled) return
      settled = true
      clearTimeout(timer)
      reject(new Error(`herdr-bot 提前退出（code ${code}）：\n${log.join('')}`))
    })
  })
}

export async function startServer(options: StartServerOptions = {}): Promise<E2EServer> {
  const executable = options.safeShutdown
    ? path.join(repoRoot, 'backend', 'target', 'debug', 'examples', 'safe_shutdown_shell.exe')
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
  const storageFile = path.join(directory, 'bots.json')
  const metadataFile = path.join(directory, 'bot-options.toml')
  // Real isolated entry files: browser acceptance must not depend on a developer toolkit installation.
  const extensionDirectory = path.join(directory, 'extensions')
  fs.mkdirSync(extensionDirectory)
  const metadata = fs.readFileSync(path.join(repoRoot, 'meta-data', 'bot-options.toml'), 'utf8')
  fs.writeFileSync(metadataFile, metadata.replace(/(\[\[extensions\]\][\s\S]*?entrypoint = )'[^']+'/g, (_line, prefix: string) => {
    const id = prefix.match(/^id = '([^']+)'/m)![1]
    const relative = `${id}.ts`
    const entrypoint = path.join(extensionDirectory, relative)
    fs.writeFileSync(entrypoint, `import fs from 'node:fs'; fs.writeFileSync(${JSON.stringify(path.join(directory, 'extension-executed'))}, ${JSON.stringify(relative)}); throw new Error('FILE CHECK MUST NOT EXECUTE')\n`)
    return `${prefix}'${entrypoint}'`
  }))
  const herdrDirectory = path.join(directory, 'herdr')

  let herdrExecutable: string | undefined
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
    await browserListenAddress(options.listenAddr ?? '127.0.0.1:0'),
  )

  let running: { child: ChildProcessWithoutNullStreams; port: number; diagnostics: () => string } | null = null

  async function start() {
    running = await startProcess(configPath, executable)
    await waitForBots(`http://127.0.0.1:${running.port}`)
  }

  await start()

  return {
    get baseURL() {
      if (running === null) throw new Error('herdr-bot 进程未运行')
      return `http://127.0.0.1:${running.port}`
    },
    urlForHost(host: string) {
      if (running === null) throw new Error('herdr-bot 进程未运行')
      return `http://${host}:${running.port}`
    },
    storageFile,
    metadataFile,
    configPath,
    diagnostics() { return running?.diagnostics() ?? '' },
    async getBots() {
      const response = await fetch(`${this.baseURL}/api/bots`)
      if (!response.ok) {
        throw new Error(`Bot 列表读取失败：HTTP ${response.status}`)
      }
      return (await response.json()) as { bots: BotView[]; options: unknown }
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
    async stop() {
      const child = running?.child
      running = null
      if (child === undefined || child.exitCode !== null) return
      const exited = Promise.withResolvers<void>()
      child.once('exit', () => exited.resolve())
      const exitTimer = setTimeout(() => exited.resolve(), 5000)
      if (process.platform === 'win32') {
        // Independent identity queries are real child processes. Killing only
        // their parent leaves a live copied EXE holding the test directory open.
        const killed = Promise.withResolvers<void>()
        const killer = spawn('taskkill.exe', ['/PID', String(child.pid), '/T', '/F'], { windowsHide: true, stdio: 'ignore' })
        killer.once('error', killed.reject)
        killer.once('exit', code => code === 0 || child.exitCode !== null ? killed.resolve() : killed.reject(new Error(`taskkill failed: ${code}`)))
        await killed.promise
      } else {
        child.kill()
      }
      await exited.promise
      clearTimeout(exitTimer)
    },
    async restart() {
      await this.stop()
      await start()
    },
    async cleanup() {
      await this.stop()
      if (process.platform === 'win32' && herdrExecutable !== undefined) {
        // taskkill /T can miss an identity reader spawned while it snapshots the
        // tree. The backend is now stopped, so no new readers can appear. Reap
        // only this fixture's copied executable and wait for its handles to close.
        const reaped = Promise.withResolvers<void>()
        const reaper = spawn('powershell.exe', ['-NoProfile', '-Command',
            "$ErrorActionPreference = 'Stop'; Get-CimInstance Win32_Process -Filter \"Name='herdr-fixture.exe'\" | Where-Object { $_.ExecutablePath -eq $env:HERDR_FIXTURE_EXECUTABLE } | ForEach-Object { $p = Get-Process -Id $_.ProcessId -ErrorAction SilentlyContinue; if ($p -and $p.Path -eq $env:HERDR_FIXTURE_EXECUTABLE) { Stop-Process -InputObject $p -ErrorAction SilentlyContinue; $p.WaitForExit() } }",
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
    },
  }
}
