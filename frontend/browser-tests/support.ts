/**
 * 真实浏览器验收的支撑设施：启动隔离的真实 herdr-bot 进程。
 *
 * 每个测试使用独立临时目录与模板存储文件；页面、公开 HTTP 接口与进程重启都来自交付二进制，
 * 不经过任何前端 mock。Herdr / agent 控制不在本票范围，固定操作指向安全夹具即可。
 */

import { spawn, type ChildProcessWithoutNullStreams } from 'node:child_process'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import readline from 'node:readline'
import { fileURLToPath } from 'node:url'

const here = path.dirname(fileURLToPath(import.meta.url))
const repoRoot = path.resolve(here, '../..')
const binary = path.join(repoRoot, 'backend', 'target', 'debug', 'herdr-bot.exe')
const fixtures = path.join(repoRoot, 'fixtures', 'operations')
const herdrFixtureBinary = path.join(repoRoot, 'backend', 'target', 'debug', 'herdr-fixture.exe')

export type TemplateView = {
  id: string
  name: string
  botName: string
  agent: string
  model: string
  thinking: string
  skill: string | null
  prompt: string
}

/** 受控 Herdr 副本的初始数据；它只在自动浏览器验收中使用。 */
export type HerdrFixtureSetup = {
  workspaceName?: string
  workspaces?: unknown[]
  behavior?: Record<string, unknown>
}

export type StartServerOptions = {
  herdr?: HerdrFixtureSetup
}

export type E2EServer = {
  baseURL: string
  storageFile: string
  configPath: string
  getTemplates: () => Promise<{ templates: TemplateView[]; options: unknown }>
  /** 受控 Herdr 自己写下的外部调用记录。 */
  herdrCalls: () => Record<string, unknown>[]
  /** 受控 Herdr 自己写下的新建 Tab 记录。 */
  herdrTabs: () => Record<string, unknown>[]
  setHerdrBehavior: (behavior: Record<string, unknown>) => void
  stop: () => Promise<void>
  restart: () => Promise<void>
  cleanup: () => Promise<void>
}

function writeConfig(
  directory: string,
  storage: string,
  herdr: { workspaceName: string; executable?: string },
): string {
  const configPath = path.join(directory, 'herdr-bot.toml')
  const herdrLines = ['[herdr]', `workspace_name = '${herdr.workspaceName}'`]
  if (herdr.executable !== undefined) {
    herdrLines.push(`executable = '${herdr.executable}'`)
  }
  fs.writeFileSync(
    configPath,
    [
      'listen_addr = "127.0.0.1:0"',
      '',
      '[operations.start]',
      `script = '${path.join(fixtures, 'success.bat')}'`,
      `working_directory = '${fixtures}'`,
      '',
      '[operations.stop]',
      `script = '${path.join(fixtures, 'success.bat')}'`,
      `working_directory = '${fixtures}'`,
      '',
      '[mdns]',
      'enabled = false',
      '',
      ...herdrLines,
      '',
      '[zones]',
      `registry_file = '${path.join(directory, 'zones.json')}'`,
      '',
      '[bot_templates]',
      `storage_file = '${storage}'`,
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

async function waitForTemplates(baseURL: string): Promise<void> {
  const deadline = Date.now() + 20_000
  while (Date.now() < deadline) {
    try {
      const response = await fetch(`${baseURL}/api/bot-templates`)
      if (response.ok) return
    } catch {
      // 进程尚未开始监听。
    }
    await new Promise((resolve) => setTimeout(resolve, 50))
  }
  throw new Error(`herdr-bot 没有在 20 秒内开始提供模板接口：${baseURL}`)
}

function startProcess(configPath: string): Promise<{ child: ChildProcessWithoutNullStreams; port: number }> {
  const child = spawn(binary, ['--config', configPath], {
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
      resolve({ child, port: Number(match[2]) })
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
  if (!fs.existsSync(binary)) {
    throw new Error(
      `缺少交付二进制 ${binary}；请先在 backend 目录执行 cargo build。`,
    )
  }
  if (!fs.existsSync(path.join(repoRoot, 'backend', 'static', 'index.html'))) {
    throw new Error('缺少打包页面；请先在 frontend 目录执行 npm run build。')
  }

  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'herdr-bot browser '))
  const storageFile = path.join(directory, 'bot-templates.json')
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

  const configPath = writeConfig(directory, storageFile, {
    workspaceName: options.herdr?.workspaceName ?? 'bot-playground',
    executable: herdrExecutable,
  })

  let running: { child: ChildProcessWithoutNullStreams; port: number } | null = null

  async function start() {
    running = await startProcess(configPath)
    await waitForTemplates(`http://127.0.0.1:${running.port}`)
  }

  await start()

  return {
    get baseURL() {
      if (running === null) throw new Error('herdr-bot 进程未运行')
      return `http://127.0.0.1:${running.port}`
    },
    storageFile,
    configPath,
    async getTemplates() {
      const response = await fetch(`${this.baseURL}/api/bot-templates`)
      if (!response.ok) {
        throw new Error(`模板列表读取失败：HTTP ${response.status}`)
      }
      return (await response.json()) as { templates: TemplateView[]; options: unknown }
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
      await new Promise<void>((resolve) => {
        child.once('exit', () => resolve())
        child.kill()
        setTimeout(() => resolve(), 5000)
      })
    },
    async restart() {
      await this.stop()
      await start()
    },
    async cleanup() {
      await this.stop()
      try {
        fs.chmodSync(storageFile, 0o666)
      } catch {
        // 文件可能不存在。
      }
      fs.rmSync(directory, { recursive: true, force: true })
    },
  }
}
