import fs from 'node:fs'
import path from 'node:path'
import os from 'node:os'
import { createHash } from 'node:crypto'
import { expect, test, type Page, type TestInfo } from '@playwright/test'
import { startServer, type E2EServer } from './support'

const environmentKeys = ['HOME', 'USERPROFILE', 'APPDATA', 'LOCALAPPDATA', 'XDG_CONFIG_HOME', 'XDG_DATA_HOME', 'XDG_STATE_HOME', 'PI_CODING_AGENT_DIR', 'OMP_CODING_AGENT_DIR']
let environment: Record<string, string | undefined> = {}
let isolatedHome: string
let server: E2EServer | null = null
test.beforeEach(async ({ browser }) => {
  void browser.version()
  isolatedHome = fs.mkdtempSync(path.join(os.tmpdir(), 'script-definition-home-'))
  environment = Object.fromEntries(environmentKeys.map(key => [key, process.env[key]]))
  for (const key of environmentKeys) {
    const directory = path.join(isolatedHome, key)
    fs.mkdirSync(directory)
    process.env[key] = directory
  }
})
test.afterEach(async ({ browser }, info) => {
  const root = path.resolve(import.meta.dirname, '../..')
  const target = process.env.CARGO_TARGET_DIR ?? path.join(root, 'backend/target')
  const files = ['frontend/bun.lock', 'backend/Cargo.lock', 'backend/src/scheduler/mod.rs', 'frontend/src/scheduler/BotTaskSelect.tsx', 'frontend/src/scheduler/MapNodeDetails.tsx', 'frontend/src/scheduler/NodeSettings.tsx', 'frontend/src/scheduler/SchedulerPanel.tsx', 'frontend/src/scheduler/schedulerApi.ts', 'frontend/browser-tests/scriptDefinitions.spec.ts', 'backend/static/index.html', ...fs.readdirSync(path.join(root, 'backend/static/assets')).map(file => `backend/static/assets/${file}`), path.join(target, 'debug/herdr-bot.exe')]
  const record = { browser: { channel: 'chrome', version: browser.version() }, files: files.map(file => ({ file, sha256: createHash('sha256').update(fs.readFileSync(path.resolve(root, file))).digest('hex') })), isolatedHome, baseURL: server?.baseURL, calls: server?.herdrCalls(), tabs: server?.herdrTabs(), diagnostics: server?.diagnostics(), processesExited: false }
  try {
    await server?.cleanup()
    record.processesExited = true
    server = null
    fs.rmSync(isolatedHome, { recursive: true, force: true })
  } finally {
    const evidencePath = info.outputPath('script-definition-process-evidence.json')
    fs.writeFileSync(evidencePath, JSON.stringify(record, null, 2))
    await info.attach('script-definition-process-evidence', { path: evidencePath, contentType: 'application/json' })
    for (const key of environmentKeys) {
      if (environment[key] === undefined) delete process.env[key]
      else process.env[key] = environment[key]
    }
  }
})
const guides = { name: '独占脚本能力', when: '', plan_guide: '', node_guide: '' }

async function evidence(page: Page, info: TestInfo, name: string, readback: unknown) {
  const http = info.outputPath(`${name}-http.json`), screenshot = info.outputPath(`${name}-page.png`)
  fs.writeFileSync(http, JSON.stringify(readback, null, 2))
  await page.screenshot({ path: screenshot, fullPage: true })
  await info.attach(`${name}-http`, { path: http, contentType: 'application/json' })
  await info.attach(`${name}-page`, { path: screenshot, contentType: 'image/png' })
}
async function readBot(page: Page) {
  const response = await page.request.post(`${server!.baseURL}/api/scheduler/bots/edit`, { data: { id: 'script-page-only' } })
  expect(response.status()).toBe(200)
  return await response.json()
}
async function reopenBot(page: Page) {
  await page.reload()
  await page.getByTestId('tab-bot').click()
  await page.getByRole('button', { name: '独占脚本能力 · script-page-only', exact: true }).click()
  await page.getByRole('button', { name: 'edit', exact: true }).click()
}

// Approved S3 actual bundled Bot/scheduler page + S1 HTTP/file interface.
// P30: fixed Rust target must survive create/edit/save/leave/reload with empty
// guides. Plausible violation: JSON saves a target but picker/preview omits it,
// or changing back to Task/Map leaves rust_script bound. Literal objects below
// are independent expectations; the real backend remains in the exercised path.
for (const capability of ['create_blank_bot', 'save_bot_guides'] as const) {
test(`P30 ${capability} is maintained and reloaded without copying Task or Map configuration`, async ({ page }, info) => {
  server = await startServer({ separateCwd: true, herdr: {} })
  const taskResponse = await page.request.post(`${server.baseURL}/api/tasks`, { data: { name: '页面普通 Task', taskType: 'normal', agent: 'omp', chatName: '{chat.id}', model: 'default', thinking: 'default', skill: null, prompt: '' } })
  expect(taskResponse.status()).toBe(200)
  const tasksBefore = await taskResponse.json()
  const task = tasksBefore.tasks.find((item: { name: string }) => item.name === '页面普通 Task')
  const maps = path.join(server.cwd, 'meta-data/workflow-maps')
  fs.mkdirSync(maps, { recursive: true })
  const child = { title: '页面 Map 目标', description: '', nodes: {} }
  fs.writeFileSync(path.join(maps, 'page-child.json'), JSON.stringify(child))
  await page.goto(`${server.baseURL}?layout=expanded`)
  await page.getByTestId('tab-bot').click()
  await page.getByRole('button', { name: 'new', exact: true }).click()
  await page.getByLabel('Bot ID', { exact: true }).fill('script-page-only')
  const json = page.getByLabel('Bot 内容 JSON', { exact: true })
  await json.fill(JSON.stringify(guides))
  await page.getByRole('combobox', { name: '固定 Rust 能力', exact: true }).selectOption('save_bot_guides')
  expect(JSON.parse(await json.inputValue())).toEqual({ ...guides, rust_script: 'save_bot_guides' })
  await page.getByRole('combobox', { name: '固定 Rust 能力', exact: true }).selectOption(capability)
  expect(JSON.parse(await json.inputValue())).toEqual({ ...guides, rust_script: capability })
  await page.getByRole('button', { name: 'save', exact: true }).click()
  const preview = page.getByRole('region', { name: 'Bot 预览', exact: true })
  await expect(preview).toContainText(capability)
  for (const section of ['when', 'plan_guide', 'node_guide']) {
    await preview.getByRole('button', { name: section, exact: true }).click()
    await expect(preview.getByRole('region', { name: section, exact: true })).toBeEmpty()
    await expect(preview.getByRole('status')).toHaveText(`当前 ${section}：空文本`)
  }
  const first = await readBot(page)
  expect(first.content).toEqual({ ...guides, rust_script: capability })
  await evidence(page, info, 'created-empty-guides', first)
  await page.getByTestId('tab-sessions').click()
  await reopenBot(page)
  await expect(page.getByRole('combobox', { name: '固定 Rust 能力', exact: true })).toHaveValue(capability)
  expect(JSON.parse(await json.inputValue())).toEqual(first.content)

  for (const invalid of [
    { ...guides, rust_script: 'unknown_capability' },
    { ...guides, rust_script: capability, task_id: task.id },
    { ...guides, rust_script: capability, map_file: 'page-child.json' },
    { ...guides, rust_script: { command: 'anything' } },
  ]) {
    const raw = JSON.stringify(invalid, null, 2)
    await json.fill(raw)
    await page.getByRole('button', { name: 'save', exact: true }).click()
    await expect(page.getByRole('alert')).toBeVisible()
    await expect(json).toHaveValue(raw)
    expect(await readBot(page)).toEqual(first)
  }
  await evidence(page, info, 'rejected-draft-preserved', await readBot(page))
  await json.fill(JSON.stringify(first.content))
  for (const target of ['task', 'script', 'map', 'script'] as const) {
    if (target === 'task') await page.getByRole('combobox', { name: 'Task', exact: true }).selectOption(`task:${task.id}`)
    if (target === 'map') await page.getByRole('combobox', { name: 'Map', exact: true }).selectOption('page-child.json')
    if (target === 'script') await page.getByRole('combobox', { name: '固定 Rust 能力', exact: true }).selectOption(capability)
    const expected = { ...guides, ...(target === 'task' ? { task_id: task.id } : target === 'map' ? { map_file: 'page-child.json' } : { rust_script: capability }) }
    expect(JSON.parse(await json.inputValue())).toEqual(expected)
    await page.getByRole('button', { name: 'save', exact: true }).click()
    await expect(preview).toBeVisible()
    expect((await readBot(page)).content).toEqual(expected)
    await reopenBot(page)
    expect(JSON.parse(await json.inputValue())).toEqual(expected)
  }
  await page.getByRole('button', { name: 'cancel', exact: true }).click()
  expect(await (await page.request.get(`${server.baseURL}/api/tasks`)).json()).toEqual(tasksBefore)
  expect(await (await page.request.get(`${server.baseURL}/api/scheduler/maps/page-child.json`)).json()).toMatchObject(child)
  expect(fs.readFileSync(path.join(maps, 'page-child.json'), 'utf8')).toBe(JSON.stringify(child))
  expect(server.herdrCalls()).toEqual([])
  await evidence(page, info, 'reloaded-rust-target', { definition: await readBot(page), hostCalls: server.herdrCalls() })
})

}

// P30/P32 definition partition. A script node has no ordinary Task/manager
// binding, consumes named materials rather than input, and cannot be configured
// to await manual reporting or close a nonexistent Chat. A UI-only restriction
// is insufficient: submit each illegal policy through real HTTP and compare
// the public definition plus original file bytes after every refusal.
test('P30 P32 scheduler saves empty-input script nodes and refuses Chat-only policy through HTTP', async ({ page }, info) => {
  server = await startServer({ separateCwd: true, herdr: {} })
  const bots = path.join(server.cwd, 'meta-data/bots'), maps = path.join(server.cwd, 'meta-data/workflow-maps')
  fs.mkdirSync(bots, { recursive: true }); fs.mkdirSync(maps, { recursive: true })
  fs.writeFileSync(path.join(bots, 'only-script.json'), JSON.stringify({ id: 'script-page-only', ...guides, rust_script: 'create_blank_bot' }))
  const mapFile = path.join(maps, 'script-page.json')
  fs.writeFileSync(mapFile, JSON.stringify({ title: '独占脚本 Map', description: '', nodes: { script: { title: '文件业务', bot_id: 'draft-unbound', input: '', zone: 'test/script', blocked_by: [] } } }))
  await page.goto(`${server.baseURL}?layout=expanded`)
  await page.getByTestId('tab-scheduler').click()
  await page.getByRole('button', { name: '独占脚本 Map', exact: true }).click()
  await page.getByRole('region', { name: '节点列表', exact: true }).getByRole('button', { name: '文件业务 script', exact: true }).click()
  await page.getByRole('button', { name: '修改 node', exact: true }).click()
  await page.getByRole('combobox', { name: '选择已有 Bot', exact: true }).selectOption('script-page-only')
  const detail = page.getByRole('region', { name: 'node 详情', exact: true })
  await expect(detail).toContainText('create_blank_bot')
  await expect(detail.getByRole('region', { name: 'Task.prompt', exact: true })).not.toContainText('尚未绑定 Task')
  await expect(detail.getByRole('radio', { name: 'manual', exact: true })).toHaveCount(0)
  await expect(detail.getByRole('checkbox', { name: 'auto_close', exact: true })).toHaveCount(0)
  await expect(detail.getByRole('combobox', { name: 'manager Task', exact: true })).toHaveCount(0)
  await expect(detail.getByLabel('输入内容', { exact: true })).toHaveValue('')
  await expect(detail).toContainText('不消费 node.input')
  await page.getByRole('button', { name: '保存 node', exact: true }).click()
  await expect(page.getByRole('form', { name: 'node 编辑', exact: true })).toHaveCount(0)
  const before = await (await page.request.get(`${server.baseURL}/api/scheduler/maps/script-page.json`)).json()
  expect(before.nodes.script).toEqual({ title: '文件业务', bot_id: 'script-page-only', input: '', input_rule: [], output_rule: [], report_policy: 'auto', auto_close: false, zone: 'test/script', blocked_by: [] })
  await page.reload()
  await page.getByTestId('tab-scheduler').click()
  await page.getByRole('button', { name: '独占脚本 Map', exact: true }).click()
  await page.getByRole('region', { name: '节点列表', exact: true }).getByRole('button', { name: '文件业务 script', exact: true }).click()
  await expect(detail).toContainText('create_blank_bot')
  await evidence(page, info, 'scheduler-script-reloaded', before)
  const bytes = fs.readFileSync(mapFile, 'utf8')
  const edit = await (await page.request.post(`${server.baseURL}/api/scheduler/maps/script-page.json/nodes/edit`, { data: { id: 'script' } })).json()
  const rejections = []
  for (const patch of [{ report_policy: 'manual' }, { auto_close: true }, { manager_task_id: 'forbidden-manager' }]) {
    const response = await page.request.post(`${server.baseURL}/api/scheduler/maps/script-page.json/nodes/save`, { data: { id: edit.id, revision: edit.revision, create: false, content: JSON.stringify({ ...edit.content, ...patch }) } })
    expect(response.status()).toBe(409)
    const rejected = await response.json()
    expect(rejected.message).toContain('脚本')
    rejections.push({ patch, rejected })
    expect(await (await page.request.get(`${server.baseURL}/api/scheduler/maps/script-page.json`)).json()).toEqual(before)
    expect(fs.readFileSync(mapFile, 'utf8')).toBe(bytes)
  }
  expect(server.herdrCalls()).toEqual([])
  await evidence(page, info, 'script-policy-rejections', { rejections, definition: before, hostCalls: server.herdrCalls() })
})

// P30: real preflight must admit an empty-input script node with available
// named materials and no ordinary/node-manager Task. A plausible violation is
// treating every non-Task target as unbound, or collapsing an invalid capability
// into an unexplained missing Bot. Change actual definition/material files
// between explicit browser checks; never stub the preflight response.
for (const capability of ['create_blank_bot', 'save_bot_guides'] as const) {
test(`P30 ${capability} preflight accepts available materials and explicitly rejects unknown conflicting or unavailable inputs without starting`, async ({ page }, info) => {
  server = await startServer({ separateCwd: true, herdr: {} })
  const root = path.dirname(server.configPath), zonePath = path.join(root, 'script-preflight-zone')
  fs.mkdirSync(zonePath)
  expect((await page.request.post(`${server.baseURL}/api/zones`, { data: { id: 'test/script', path: zonePath } })).status()).toBe(200)
  const taskResponse = await page.request.post(`${server.baseURL}/api/tasks`, { data: { name: '真实 Run 管理者', taskType: 'manager', agent: 'omp', chatName: '{chat.id}', model: 'default', thinking: 'default', skill: null, prompt: '' } })
  expect(taskResponse.status()).toBe(200)
  const tasks = await taskResponse.json()
  expect(tasks.tasks).toHaveLength(1)
  const manager = tasks.tasks[0].id
  const bots = path.join(server.cwd, 'meta-data/bots'), maps = path.join(server.cwd, 'meta-data/workflow-maps')
  fs.mkdirSync(bots, { recursive: true }); fs.mkdirSync(maps, { recursive: true })
  const bot = { id: 'script-page-only', ...guides, rust_script: capability }, botFile = path.join(bots, 'preflight-script.json')
  fs.writeFileSync(botFile, JSON.stringify(bot))
  fs.writeFileSync(path.join(maps, 'material-target.json'), JSON.stringify({ title: '不执行的材料目标', description: '', nodes: {} }))
  const inputIds = capability === 'create_blank_bot' ? ['request'] : ['baseline', 'when_text', 'when_handoff', 'plan_guide_text', 'plan_guide_handoff', 'node_guide_text', 'node_guide_handoff']
  const map = { title: '脚本真实预检', description: '', input_files: inputIds.map(id => ({ id, required: true })), nodes: { script: { title: '预检文件业务', bot_id: bot.id, input: '', zone: 'test/script', blocked_by: [], input_rule: inputIds.map(id => ({ id, type: 'run', input_id: id })), output_rule: [{ id: capability === 'create_blank_bot' ? 'baseline' : 'saved' }, { id: 'handoff' }] } } }
  const mapFile = path.join(maps, 'script-preflight.json')
  fs.writeFileSync(mapFile, JSON.stringify(map))
  const request = { schema_version: 1, kind: 'request', target: { type: 'map', map_file: 'material-target.json' }, bot: { id: 'preflight-target', name: '预检保存目标' }, requirements: '仅创建空白能力，不执行目标', constraints: [], non_goals: [], resource_limits: [], authorization: { create_blank: true, write_materials: true, save_guides: false }, confirmation: { evidence: '独占验收授权', scope: 'create_blank' } }
  const materials = Object.fromEntries(inputIds.map(id => [id, path.join(zonePath, `${id}.json`)]))
  if (capability === 'create_blank_bot') {
    fs.writeFileSync(materials.request, JSON.stringify(request))
  } else {
    const target = { name: '预检保存目标', when: '', plan_guide: '', node_guide: '', map_file: 'material-target.json' }
    expect((await page.request.post(`${server.baseURL}/api/scheduler/bots/save`, { data: { id: 'preflight-target', revision: null, content: JSON.stringify(target) } })).status()).toBe(200)
    const editResponse = await page.request.post(`${server.baseURL}/api/scheduler/bots/edit`, { data: { id: 'preflight-target' } })
    expect(editResponse.status()).toBe(200)
    const edit = await editResponse.json()
    fs.writeFileSync(materials.baseline, JSON.stringify({ schema_version: 1, kind: 'baseline', bot: JSON.parse(edit.revision.original), revision: edit.revision, target: { ...request.target, configuration: { title: '不执行的材料目标', description: '', nodes: {} } }, request }))
    for (const field of ['when', 'plan_guide', 'node_guide']) {
      const text = `预检独占 ${field} 正文\n`
      fs.writeFileSync(materials[`${field}_text`], text)
      fs.writeFileSync(materials[`${field}_handoff`], JSON.stringify({ schema_version: 1, kind: 'handoff', stage: 'guide', bot_id: 'preflight-target', target: request.target, field, sources: [{ binding_id: 'baseline', direction: 'input', role: 'baseline' }, { binding_id: 'text', direction: 'output', role: 'text' }], confirmation: { evidence: '独占正文确认与保存授权', text_sha256: createHash('sha256').update(text).digest('hex'), full_text: true, save_authorized: true, semantics_preserved: true }, constraints: [], unverified: ['不执行目标'] }))
    }
  }
  const missingId = inputIds[0], material = materials[missingId]
  await page.goto(`${server.baseURL}?layout=expanded`)
  await page.getByTestId('tab-scheduler').click()
  await page.getByRole('button', { name: '脚本真实预检', exact: true }).click()
  await page.getByRole('button', { name: '+ 发起 Run', exact: true }).click()
  const form = page.getByRole('form', { name: '发起 Run', exact: true })
  await form.getByRole('combobox', { name: '管理者 Task', exact: true }).selectOption(manager)
  await form.getByRole('combobox', { name: '管理者 zone', exact: true }).selectOption('test/script')
  for (const id of inputIds) await form.getByLabel(`文件输入 ${id}（必需）`, { exact: true }).fill(materials[id])
  const observations: unknown[] = []
  async function check(status: number, message: string) {
    const response = page.waitForResponse(response => response.url().endsWith('/script-preflight.json/preflight') && response.request().method() === 'POST')
    await form.getByRole('button', { name: '校验启动配置', exact: true }).click()
    const actual = await response
    expect(actual.status()).toBe(status)
    const body = await actual.json()
    observations.push({ status, body })
    await expect(form).toContainText(message)
    expect((await (await page.request.get(`${server!.baseURL}/api/workflow/runs`)).json()).runs).toEqual([])
    expect(server!.herdrCalls()).toEqual([])
  }
  await check(200, '配置校验通过')
  await evidence(page, info, 'script-preflight-valid', { observations, bot, map })
  fs.writeFileSync(botFile, JSON.stringify({ ...bot, rust_script: 'unknown-capability' }))
  await check(400, '未知')
  await evidence(page, info, 'script-preflight-unknown', observations)
  fs.writeFileSync(botFile, JSON.stringify({ ...bot, map_file: 'material-target.json' }))
  await check(400, '互斥')
  await evidence(page, info, 'script-preflight-conflict', observations)
  fs.writeFileSync(botFile, JSON.stringify(bot))
  fs.unlinkSync(material)
  await check(400, '文件')
  await expect(form.getByRole('alert')).toContainText(missingId)
  await expect(form.getByLabel(`文件输入 ${missingId}（必需）`, { exact: true })).toHaveValue(material)
  expect(fs.readFileSync(mapFile, 'utf8')).toBe(JSON.stringify(map))
  expect(await (await page.request.get(`${server.baseURL}/api/tasks`)).json()).toEqual(tasks)
  await evidence(page, info, 'script-preflight-missing-material', { observations, hostCalls: server.herdrCalls(), runs: (await (await page.request.get(`${server.baseURL}/api/workflow/runs`)).json()).runs })
})

}

// P32: switching an existing definition to a script must not silently coerce
// manual/close/manager values, nor trap the user with uneditable bad settings.
// Keep the malformed original visible, reject save, then explicitly repair it
// using page actions and independently read the saved definition.
test('P32 existing script policy conflict retains original values until the user explicitly corrects the draft', async ({ page }, info) => {
  server = await startServer({ separateCwd: true, herdr: {} })
  const bots = path.join(server.cwd, 'meta-data/bots'), maps = path.join(server.cwd, 'meta-data/workflow-maps')
  fs.mkdirSync(bots, { recursive: true }); fs.mkdirSync(maps, { recursive: true })
  fs.writeFileSync(path.join(bots, 'repair-script.json'), JSON.stringify({ id: 'repair-script', ...guides, rust_script: 'create_blank_bot' }))
  const node = { title: '修正策略', bot_id: 'repair-script', input: '', zone: 'test/script', blocked_by: [], report_policy: 'manual', auto_close: true, manager_task_id: 'old-manager' }
  const mapFile = path.join(maps, 'repair-script.json'), original = JSON.stringify({ title: '显式修正脚本策略', description: '', nodes: { script: node } })
  fs.writeFileSync(mapFile, original)
  await page.goto(`${server.baseURL}?layout=expanded`)
  await page.getByTestId('tab-scheduler').click()
  await page.getByRole('button', { name: '显式修正脚本策略', exact: true }).click()
  await page.getByRole('region', { name: '节点列表', exact: true }).getByRole('button', { name: '修正策略 script', exact: true }).click()
  await page.getByRole('button', { name: '修改 node', exact: true }).click()
  const detail = page.getByRole('region', { name: 'node 详情', exact: true })
  await expect(detail.getByRole('region', { name: 'node 配置概览', exact: true })).toContainText('manual')
  await expect(detail.getByRole('region', { name: 'node 配置概览', exact: true })).toContainText('true')
  await page.getByRole('button', { name: '保存 node', exact: true }).click()
  await expect(page.getByRole('alert')).toContainText('脚本')
  expect(fs.readFileSync(mapFile, 'utf8')).toBe(original)
  await expect(detail.getByRole('combobox', { name: 'manager Task', exact: true })).toHaveCount(0)
  await evidence(page, info, 'script-conflicting-draft', await (await page.request.get(`${server.baseURL}/api/scheduler/maps/repair-script.json`)).json())
  await detail.getByRole('button', { name: '改为自动报告且不关闭会话', exact: true }).click()
  await detail.getByRole('button', { name: '移除 manager_task_id', exact: true }).click()
  await page.getByRole('button', { name: '保存 node', exact: true }).click()
  await expect(page.getByRole('form', { name: 'node 编辑', exact: true })).toHaveCount(0)
  const saved = await (await page.request.get(`${server.baseURL}/api/scheduler/maps/repair-script.json`)).json()
  expect(saved.nodes.script).toEqual({ title: '修正策略', bot_id: 'repair-script', input: '', zone: 'test/script', blocked_by: [], report_policy: 'auto', auto_close: false, input_rule: [], output_rule: [] })
  expect(server.herdrCalls()).toEqual([])
  await evidence(page, info, 'script-explicitly-repaired', saved)
})
