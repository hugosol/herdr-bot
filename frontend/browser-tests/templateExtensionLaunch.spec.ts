import fs from 'node:fs'
import path from 'node:path'
import os from 'node:os'
import { spawn } from 'node:child_process'
import { expect, test, type Page } from '@playwright/test'
import { startServer, type E2EServer } from './support'
import { zoneRowFor } from './ui'

let server: E2EServer | null = null
test.afterEach(async () => { await server?.cleanup(); server = null })
const template = { name: '扩展启动', botName: '{zone.name}-{bot.id}', agent: 'omp', model: 'default', thinking: 'default', skill: null, prompt: '固定指示' }
async function post(endpoint: string, body: unknown) {
  return fetch(`${server!.baseURL}${endpoint}`, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body) })
}
async function setup() {
  server = await startServer({ herdr: {} })
  const root = path.dirname(server.metadataFile)
  const zonePath = path.join(root, 'launch-zone')
  fs.mkdirSync(zonePath)
  const response = await post('/api/zones', { path: zonePath })
  const zone = (await response.json()).zones[0].path as string
  return { root, zone, original: fs.readFileSync(server.metadataFile, 'utf8') }
}
async function save(value: Record<string, unknown>) {
  const response = await post('/api/bot-templates', value)
  expect(response.status).toBe(200)
  return (await response.json()).templates.at(-1)
}
async function create(zone: string, id: string, extra = {}) {
  const preview = await (await post('/api/bots/preview', { zone, templateId: id })).json()
  return post('/api/bots', { zone, templateId: id, botId: preview.botId, input: '本次输入', ...extra })
}
async function finish(response: Response) {
  expect(response.status).toBe(202)
  const job = await response.json()
  await expect.poll(async () => (await (await fetch(`${server!.baseURL}/api/bots/${job.id}`)).json()).stage).toBe('succeeded')
  return (await (await fetch(`${server!.baseURL}/api/bots/${job.id}`)).json())
}
async function dialog(page: Page, zone: string) {
  await page.goto(server!.baseURL)
  await page.getByTestId('tab-files').click()
  await zoneRowFor(page, zone).getByTestId('zone-create-bot').click()
  await expect(page.getByTestId('bot-name-preview')).toHaveAttribute('data-state', 'ready')
  await page.locator('#bot-workspace-target').selectOption('default')
  return page.getByTestId('bot-creation-dialog')
}

// S1/S2/S3 P16/P21/P22/P24: literal six-combination expectations, not generated from launch code.
// Violation: ambient discovery remains on, fixed item is omitted, or unselected entries leak into args.
test('六种模板集合准确显式启动，固定项不可取消，创建页面没有临时扩展选择', async ({ page }, testInfo) => {
  const { root, zone } = await setup()
  const ext = path.join(root, 'extensions')
  const cases = [
    { agent: 'omp', extensions: [], entries: ['omp-herdr-agent-state.ts', 'omp-model-cost.ts'] },
    { agent: 'omp', extensions: ['omp-codebase-tools'], entries: ['omp-herdr-agent-state.ts', 'omp-model-cost.ts', 'omp-codebase-tools.ts'] },
    { agent: 'omp', extensions: ['omp-connect-bot'], entries: ['omp-herdr-agent-state.ts', 'omp-model-cost.ts', 'omp-connect-bot.ts'] },
    { agent: 'omp', extensions: ['omp-connect-bot', 'omp-codebase-tools'], entries: ['omp-herdr-agent-state.ts', 'omp-model-cost.ts', 'omp-codebase-tools.ts', 'omp-connect-bot.ts'] },
    { agent: 'pi', extensions: [], entries: ['pi-herdr-agent-state.ts', 'pi-model-cost.ts'] },
    { agent: 'pi', extensions: ['pi-codebase-tools'], entries: ['pi-herdr-agent-state.ts', 'pi-model-cost.ts', 'pi-codebase-tools.ts'] },
  ]
  // Agent settings are sentinels, never launch configuration output.
  const settings = ['.omp/config.yml', '.omp/config.yaml', '.omp/settings.json', '.pi/settings.json'].map(relative => path.join(zone, relative))
  for (const file of settings) { fs.mkdirSync(path.dirname(file), { recursive: true }); fs.writeFileSync(file, '{"extensions":["ambient.ts"]}\n') }
  const userSettings = ['.omp/agent/config.yml', '.omp/agent/config.yaml', '.pi/agent/settings.json'].map(relative => path.join(os.homedir(), relative))
  const userBefore = userSettings.map(file => fs.existsSync(file) ? fs.readFileSync(file) : null)
  for (const [index, scenario] of cases.entries()) {
    const saved = await save({ ...template, ...scenario, entries: undefined })
    if (index === 0) {
      const form = await dialog(page, zone)
      await expect(form.locator('input[type=checkbox]')).toHaveCount(0)
      for (const override of [{ extensions: [] }, { entrypoint: 'arbitrary.ts' }, { fixedExtensions: [] }]) {
        const rejected = await create(zone, saved.id, override)
        expect(rejected.status).toBe(400)
        expect(server!.herdrCalls()).toEqual([])
        expect(server!.herdrTabs()).toEqual([])
      }
      await form.getByTestId('bot-input').fill('本次输入')
      await form.getByTestId('bot-create-submit').click()
      await expect(page.getByTestId('bot-create-success')).toBeVisible()
      await expect(page.getByTestId('tab-files')).toHaveAttribute('aria-selected', 'true')
      await expect(form).not.toContainText('已接入')
      await page.getByTestId('bot-create-close').click()
    } else {
      await finish(await create(zone, saved.id))
    }
    const start = server!.herdrCalls().filter(call => call.kind === 'agent_start').at(-1)!
    expect(start.agent_kind).toBe(scenario.agent)
    const args = start.args as string[]
    expect(args).toEqual(['--no-extensions', ...scenario.entries.flatMap(entry => ['--extension', path.join(ext, entry)])])
    expect(server!.herdrCalls().filter(call => call.kind === 'agent_prompt')).toHaveLength(index + 1)
    expect(server!.herdrCalls().filter(call => call.kind === 'agent_prompt').at(-1)!.text).toBe('固定指示\n\n本次输入')
  }
  for (const file of settings) expect(fs.readFileSync(file, 'utf8')).toBe('{"extensions":["ambient.ts"]}\n')
  expect(userSettings.map(file => fs.existsSync(file) ? fs.readFileSync(file) : null)).toEqual(userBefore)
  expect(fs.existsSync(path.join(root, 'extension-executed'))).toBe(false)
  const evidence = testInfo.outputPath('controlled-herdr-six-combinations-NOT-real-loading.json')
  fs.writeFileSync(evidence, JSON.stringify({ calls: server!.herdrCalls(), tabs: server!.herdrTabs(), expected: cases }, null, 2))
  await testInfo.attach('controlled-herdr-six-combinations-NOT-real-loading', { path: evidence, contentType: 'application/json' })
})

// P25/P26/P34/P39: same stable IDs resolve a fresh mapping, while unselected missing entries stay irrelevant.
// Violation: stale template path copies, or checking all files in the catalog instead of this creation's set.
test('同进程相同已保存标识使用新入口且忽略未选缺失入口，不执行扩展', async ({}, testInfo) => {
  const { root, zone, original } = await setup()
  const saved = await save({ ...template, extensions: ['omp-codebase-tools'] })
  const persistedBefore = fs.readFileSync(server!.storageFile, 'utf8')
  fs.unlinkSync(path.join(root, 'extensions', 'omp-connect-bot.ts'))
  fs.unlinkSync(path.join(root, 'extensions', 'pi-model-cost.ts'))
  await finish(await create(zone, saved.id))
  const freshFixed = path.join(root, 'next-fixed.ts')
  const freshOptional = path.join(root, 'next-optional.ts')
  for (const file of [freshFixed, freshOptional]) {
    fs.writeFileSync(file, `import fs from 'node:fs'; fs.writeFileSync(${JSON.stringify(path.join(root, 'extension-executed'))}, 'EXECUTED'); throw new Error('must not execute during check')`)
  }
  const oldFixed = path.join(root, 'extensions', 'omp-model-cost.ts')
  const oldOptional = path.join(root, 'extensions', 'omp-codebase-tools.ts')
  fs.writeFileSync(server!.metadataFile, original.replace(oldFixed, freshFixed).replace(oldOptional, freshOptional))
  fs.unlinkSync(oldFixed)
  fs.unlinkSync(oldOptional)
  await finish(await create(zone, saved.id))
  const starts = server!.herdrCalls().filter(call => call.kind === 'agent_start')
  const official = path.join(root, 'extensions', 'omp-herdr-agent-state.ts')
  expect(starts[0].args).toEqual(['--no-extensions', '--extension', official, '--extension', oldFixed, '--extension', oldOptional])
  expect(starts[1].args).toEqual(['--no-extensions', '--extension', official, '--extension', freshFixed, '--extension', freshOptional])
  expect(fs.readFileSync(server!.storageFile, 'utf8')).toBe(persistedBefore)
  expect((await server!.getTemplates()).templates[0]).toEqual(saved)
  expect(fs.existsSync(path.join(root, 'extension-executed'))).toBe(false)
  // Independent native-history reads are not launch/configuration controls.
  expect(server!.herdrCalls().filter(call => call.kind !== 'agent_get').map(call => call.kind)).toEqual(['workspace_list', 'tab_create', 'agent_start', 'agent_prompt', 'workspace_list', 'tab_create', 'agent_start', 'agent_prompt'])
  const evidence = testInfo.outputPath('controlled-herdr-dynamic-entry-mapping-NOT-real-loading.json')
  fs.writeFileSync(evidence, JSON.stringify({ calls: server!.herdrCalls(), tabs: server!.herdrTabs() }, null, 2))
  await testInfo.attach('controlled-herdr-dynamic-entry-mapping-NOT-real-loading', { path: evidence, contentType: 'application/json' })
})

// A real Windows sharing-denied handle, not chmod (which does not deny reads on Windows).
async function denyRead(file: string) {
  const child = spawn('powershell.exe', ['-NoProfile', '-NonInteractive', '-Command', `$f=[System.IO.File]::Open('${file.replaceAll("'", "''")}', 'Open', 'Read', 'None'); [Console]::Out.WriteLine('locked'); [Console]::In.ReadLine() | Out-Null; $f.Dispose()`])
  const ready = Promise.withResolvers<void>()
  child.stdout.on('data', chunk => { if (chunk.toString().includes('locked')) ready.resolve() })
  child.once('error', ready.reject)
  child.once('exit', code => { if (code !== 0) ready.reject(new Error(`Read-denial setup exited ${code}`)) })
  await ready.promise
  return async () => {
    const exited = Promise.withResolvers<void>()
    child.once('exit', () => exited.resolve())
    child.stdin.end('\n')
    await exited.promise
  }
}

// Readable extension directories are supported; discovery within them belongs to the agent.
for (const entry of ['omp-model-cost', 'omp-codebase-tools']) {
  test(`目录入口 ${entry} 按原路径交给 agent 且不执行入口内容`, async ({ page }) => {
    const { root, zone } = await setup()
    await save({ ...template, extensions: ['omp-codebase-tools'] })
    const directory = path.join(root, 'extensions', `${entry}.ts`)
    fs.unlinkSync(directory)
    fs.mkdirSync(directory)
    fs.writeFileSync(path.join(directory, 'index.ts'), `throw new Error('DIRECTORY CHECK MUST NOT EXECUTE')\n`)
    const form = await dialog(page, zone)
    await form.getByTestId('bot-input').fill('本次输入')
    await form.getByTestId('bot-create-submit').click()
    await expect(page.getByTestId('bot-create-success')).toBeVisible()
    const start = server!.herdrCalls().find(call => call.kind === 'agent_start')!
    expect(start.args).toContain(directory)
    expect(server!.herdrCalls().filter(call => call.kind === 'agent_prompt')).toHaveLength(1)
    expect(fs.existsSync(path.join(root, 'extension-executed'))).toBe(false)
  })
}

// P33/P35: genuine missing/sharing-denied files, for fixed AND selected optional items.
// Violation: check only fixed items, only existence, or start a tab before validating. Real files and adapter stay in path.
for (const entry of ['omp-model-cost', 'omp-codebase-tools']) {
  for (const fault of ['missing', 'unreadable']) {
    test(`自动入口检查 ${entry}/${fault} 在页面及HTTP拒绝且无三个副作用`, async ({ page }) => {
      const { root, zone } = await setup()
      const saved = await save({ ...template, extensions: ['omp-codebase-tools'] })
      const form = await dialog(page, zone)
      const file = path.join(root, 'extensions', `${entry}.ts`)
      let unlock: (() => Promise<void>) | undefined
      if (fault === 'missing') fs.unlinkSync(file)
      if (fault === 'unreadable') unlock = await denyRead(file)
      try {
        const response = await create(zone, saved.id)
        expect(response.status).toBe(500)
        const error = await response.json()
        expect(error.code).toBe('extension-entrypoint-invalid')
        expect(error.message).toContain(entry === 'omp-model-cost' ? 'omp-model-cost' : 'omp-codebase-tools')
        expect(error.message).toContain(file)
        expect(error.message).toContain({ missing: '不存在', unreadable: '无法读取' }[fault])
        expect(error.message).toContain('未创建 Herdr Tab、未启动 agent、未提交输入')
        await form.getByTestId('bot-create-submit').click()
        await expect(page.getByTestId('bot-request-error')).toContainText('未创建 Herdr Tab、未启动 agent、未提交输入')
        await expect(page.getByTestId('bot-request-error')).toContainText(file)
        expect(server!.herdrCalls()).toEqual([])
        expect(server!.herdrTabs()).toEqual([])
        expect(fs.readdirSync(path.join(root, 'herdr')).sort()).toEqual(['behavior.json', 'herdr-fixture.exe', 'workspaces.json'])
        expect(fs.existsSync(path.join(root, 'extension-executed'))).toBe(false)
        await expect(page.getByTestId('bot-result')).toHaveCount(0)
      } finally { await unlock?.() }
    })
  }
}

// P35 metadata and persisted selection categories must be rejected by create itself, even with a dialog already open.
for (const fault of ['metadata-missing', 'metadata-unreadable', 'metadata-syntax', 'metadata-duplicate', 'metadata-relation', 'selection-removed', 'selection-agent', 'selection-fixed', 'selection-duplicate', 'template-model', 'template-thinking', 'template-skill', 'template-agent']) {
  test(`前置配置检查 ${fault} 的公开创建和实际页面无外部副作用`, async ({ page }) => {
    const { root, zone, original } = await setup()
    const saved = await save({ ...template, extensions: ['omp-codebase-tools'] })
    const form = await dialog(page, zone)
    let unlock: (() => Promise<void>) | undefined
    if (fault === 'metadata-missing') fs.unlinkSync(server!.metadataFile)
    if (fault === 'metadata-unreadable') unlock = await denyRead(server!.metadataFile)
    if (fault === 'metadata-syntax') fs.writeFileSync(server!.metadataFile, '[[extensions')
    if (fault === 'metadata-duplicate') fs.writeFileSync(server!.metadataFile, original + "\n[[agents]]\nid='omp'\nlabel='duplicate'\n")
    if (fault === 'metadata-relation') fs.writeFileSync(server!.metadataFile, original.replace('omp = [', 'unknown = ['))
    if (fault.startsWith('selection-')) {
      const persisted = JSON.parse(fs.readFileSync(server!.storageFile, 'utf8'))
      persisted.templates[0].extensions = {
        'selection-removed': ['removed-extension'], 'selection-agent': ['pi-codebase-tools'],
        'selection-fixed': ['omp-model-cost'], 'selection-duplicate': ['omp-codebase-tools', 'omp-codebase-tools'],
      }[fault]
      fs.writeFileSync(server!.storageFile, JSON.stringify(persisted))
    }
    if (fault.startsWith('template-')) {
      const persisted = JSON.parse(fs.readFileSync(server!.storageFile, 'utf8'))
      const field = fault.slice('template-'.length)
      persisted.templates[0][field] = 'removed-choice'
      if (field === 'thinking') persisted.templates[0].model = 'gpt-6.1-sol'
      fs.writeFileSync(server!.storageFile, JSON.stringify(persisted))
    }
    try {
      const response = await post('/api/bots', { zone, templateId: saved.id, botId: 'abcdef', input: '本次输入' })
      expect(response.status).toBe(fault.startsWith('metadata-') || fault === 'template-agent' ? 500 : 400)
      const error = await response.json()
      expect(error.code).toBe(fault.startsWith('metadata-') ? 'metadata-invalid' : fault === 'template-agent' ? 'templates-corrupt' : fault.startsWith('template-') ? `${fault}-invalid` : 'template-extension-invalid')
      expect(error.message).toContain('未创建 Herdr Tab、未启动 agent、未提交输入')
      await form.getByTestId('bot-create-submit').click()
      await expect(page.getByTestId('bot-request-error')).toContainText('未创建 Herdr Tab、未启动 agent、未提交输入')
      expect(server!.herdrCalls()).toEqual([])
      expect(server!.herdrTabs()).toEqual([])
      expect(fs.readdirSync(path.join(root, 'herdr')).sort()).toEqual(['behavior.json', 'herdr-fixture.exe', 'workspaces.json'])
      await expect(page.getByTestId('bot-result')).toHaveCount(0)
    } finally { await unlock?.() }
  })
}
