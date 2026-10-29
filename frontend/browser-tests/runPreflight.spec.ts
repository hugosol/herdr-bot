import fs from 'node:fs'
import path from 'node:path'
import { expect, test } from '@playwright/test'
import { startServer, type E2EServer } from './support'
import type { Bot } from '../src/schedulerApi'

let server: E2EServer | null = null
test.afterEach(async () => { await server?.cleanup(); server = null })

// Approved seams: S1 delivered scheduler form, S2 real HTTP, S3 external
// Herdr recorder, S4 isolated files. No model/native-host claim is made here.
// The bounded absence window is server start through the final completed
// response (each test has the runner's 90-second deadline), not indefinite absence.
function dataFiles(root: string): Record<string, string> {
  if (!fs.existsSync(root)) return {}
  const files: Record<string, string> = {}
  for (const entry of fs.readdirSync(root, { withFileTypes: true })) {
    const file = path.join(root, entry.name)
    if (entry.isDirectory()) for (const [name, bytes] of Object.entries(dataFiles(file))) files[`${entry.name}/${name}`] = bytes
    else files[entry.name] = fs.readFileSync(file).toString('base64')
  }
  return files
}

async function launchFixture() {
  server = await startServer({ separateCwd: true, herdr: {} })
  const maps = path.join(server.cwd, '.data/workflow-maps')
  const bots = path.join(server.cwd, '.data/bots')
  fs.mkdirSync(maps, { recursive: true }); fs.mkdirSync(bots, { recursive: true })
  const directory = path.dirname(server.configPath)
  const zonePath = path.join(directory, 'registered-zone')
  fs.mkdirSync(zonePath)
  const registered = await fetch(`${server.baseURL}/api/zones`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ path: zonePath }) })
  expect(registered.status).toBe(200)
  const zone: string = (await registered.json()).zones[0].path
  const ids: Record<string, string> = {}
  for (const role of ['manager', 'normal']) {
    const response = await fetch(`${server.baseURL}/api/tasks`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ name: role, taskType: role, agent: 'omp', chatName: '{chat.id}', model: 'default', thinking: 'default', skill: null, prompt: '', extensions: [] }) })
    expect(response.status).toBe(200)
    ids[role] = (await response.json()).tasks.find((task: { name: string }) => task.name === role).id
  }
  const node = { title: '工作', bot_id: 'worker', input: '', zone, blocked_by: [] as string[] }
  const map = { title: '预检图', description: '', nodes: { A: node, B: { ...node, blocked_by: ['A'] } } }
  const bot: Bot = { id: 'worker', name: '', when: '', plan_guide: '', node_guide: '', task_id: ids.normal }
  const mapFile = path.join(maps, 'preflight.json')
  const botFile = path.join(bots, 'worker.json')
  fs.writeFileSync(mapFile, JSON.stringify(map)); fs.writeFileSync(botFile, JSON.stringify(bot))
  return { zone, zonePath, ids, map, bot, mapFile, botFile, data: { managerTaskId: ids.manager, zone, userInput: '' } }
}

// P7.01: contract says an empty Map is saveable but never launchable.
// Violation: accepting it, or allocating a Run/Chat before checking the graph.
test('P7.01 empty Map is rejected before any Run or Chat side effect', async ({ request }, info) => {
  server = await startServer({ separateCwd: true, herdr: {} })
  const maps = path.join(server.cwd, '.data/workflow-maps')
  fs.mkdirSync(maps, { recursive: true })
  fs.writeFileSync(path.join(maps, 'empty.json'), JSON.stringify({ title: '空图', description: '', nodes: {} }))
  const before = dataFiles(server.cwd)
  const response = await request.post(`${server.baseURL}/api/scheduler/maps/empty.json/preflight`, { data: { managerTaskId: '', zone: '', userInput: '' } })
  expect(response.status()).toBe(400)
  const result = await response.json()
  expect(result).toMatchObject({ code: 'run-preflight-rejected', issues: [expect.objectContaining({ code: 'map-empty', target: 'map' })] })
  expect(result.message).toBeTruthy()
  expect(dataFiles(server.cwd)).toEqual(before)
  expect(server.herdrCalls()).toEqual([])
  fs.writeFileSync(info.outputPath('empty-map-evidence.json'), JSON.stringify({ response: result, before, after: dataFiles(server.cwd), calls: server.herdrCalls() }, null, 2))
})

// P7.01: independent invalid bindings, including dependent B rather than just
// ready A. A root-only check, manager-role acceptance, or fallback zone must fail.
test('P7.01 whole graph and manager bindings reject missing resources and wrong roles', async ({ request }, info) => {
  const fixture = await launchFixture()
  const evidence: unknown[] = []
  const originalTasks = fs.readFileSync(server!.storageFile, 'utf8')
  const scenarios = [
    { code: 'bot-missing', target: 'node', mutate: () => { fixture.map.nodes.B.bot_id = 'absent' } },
    { code: 'bot-task-required', target: 'bot', mutate: () => { delete fixture.bot.task_id } },
    { code: 'bot-task-required', target: 'bot', mutate: () => { fixture.bot.task_id = '' } },
    { code: 'task-missing', target: 'task', mutate: () => { fixture.bot.task_id = 'absent' } },
    { code: 'task-role-invalid', target: 'task', mutate: () => { fixture.bot.task_id = fixture.ids.manager } },
    { code: 'zone-required', target: 'node', mutate: () => { fixture.map.nodes.B.zone = '' } },
    { code: 'zone-not-registered', target: 'node', mutate: () => { fixture.map.nodes.B.zone = path.join(fixture.zonePath, 'missing') } },
    { code: 'task-missing', target: 'manager', mutate: () => { fixture.data.managerTaskId = 'absent' } },
    { code: 'task-role-invalid', target: 'manager', mutate: () => { fixture.data.managerTaskId = fixture.ids.normal } },
    { code: 'zone-required', target: 'manager', mutate: () => { fixture.data.zone = '' } },
  ]
  for (const scenario of scenarios) {
    fixture.map.nodes.B.bot_id = 'worker'; fixture.map.nodes.B.zone = fixture.zone
    fixture.bot.task_id = fixture.ids.normal; fixture.data.managerTaskId = fixture.ids.manager; fixture.data.zone = fixture.zone
    fs.writeFileSync(server!.storageFile, originalTasks)
    scenario.mutate()
    fs.writeFileSync(fixture.mapFile, JSON.stringify(fixture.map)); fs.writeFileSync(fixture.botFile, JSON.stringify(fixture.bot))
    const before = dataFiles(server!.cwd)
    const history = path.join(path.dirname(server!.configPath), 'session-history.json')
    const historyBefore = fs.existsSync(history) ? fs.readFileSync(history, 'utf8') : null
    const response = await request.post(`${server!.baseURL}/api/scheduler/maps/preflight.json/preflight`, { data: fixture.data })
    expect(response.status(), scenario.code).toBe(400)
    const result = await response.json()
    expect(result).toMatchObject({ code: 'run-preflight-rejected', issues: expect.arrayContaining([expect.objectContaining({ code: scenario.code, target: scenario.target })]) })
    expect(result.message).toBeTruthy()
    expect(dataFiles(server!.cwd)).toEqual(before)
    expect(fs.existsSync(history) ? fs.readFileSync(history, 'utf8') : null).toBe(historyBefore)
    expect(server!.herdrCalls()).toEqual([])
    evidence.push({ scenario: scenario.code, result, filesUnchanged: true, calls: server!.herdrCalls() })
  }
  fs.writeFileSync(info.outputPath('binding-rejection-evidence.json'), JSON.stringify(evidence, null, 2))
})

// P8.01: literal filesystem paths, real unregister/register requests. The
// violation is cached validity or silently following a moved/replaced root.
// Existing checked-root rules already enforce this; no artificial red is made.
test('P8.01 unregister and same-path registration recheck current graph without following moved or replaced directories', async ({ request }, info) => {
  const fixture = await launchFixture()
  const api = `${server!.baseURL}/api/scheduler/maps/preflight.json/preflight`
  const evidence: unknown[] = []
  async function check(expected: string | null) {
    const before = dataFiles(server!.cwd)
    const response = await request.post(api, { data: fixture.data })
    const result = await response.json()
    expect(response.status()).toBe(expected === null ? 200 : 400)
    if (expected === null) expect(result).toEqual({ workspaceName: 'registered-zone' })
    else expect(result.issues).toEqual(expect.arrayContaining([expect.objectContaining({ code: expected })]))
    expect(dataFiles(server!.cwd)).toEqual(before)
    expect(server!.herdrCalls()).toEqual([])
    evidence.push({ expected, result, filesUnchanged: true, calls: server!.herdrCalls() })
  }
  await check(null)
  const unregister = await request.post(`${server!.baseURL}/api/zones/remove`, { data: { path: fixture.zone } })
  expect(unregister.status()).toBe(200)
  await check('zone-not-registered')
  const register = await request.post(`${server!.baseURL}/api/zones`, { data: { path: fixture.zonePath } })
  expect(register.status()).toBe(200)
  fixture.map.nodes.B.bot_id = 'still-invalid'
  fs.writeFileSync(fixture.mapFile, JSON.stringify(fixture.map))
  await check('bot-missing')
  fixture.map.nodes.B.bot_id = 'worker'
  fs.writeFileSync(fixture.mapFile, JSON.stringify(fixture.map))
  await check(null)
  const moved = `${fixture.zonePath}-moved`
  fs.renameSync(fixture.zonePath, moved)
  expect((await request.post(`${server!.baseURL}/api/zones`, { data: { path: moved } })).status()).toBe(200)
  await check('zone-unavailable')
  fs.writeFileSync(fixture.zonePath, 'not a directory')
  await check('zone-unavailable')
  fs.writeFileSync(info.outputPath('zone-path-lifecycle-evidence.json'), JSON.stringify(evidence, null, 2))
})

// P9.01: only manager Tasks belong in the picker, regardless of host. A name or node override,
// mutable Workspace, or generic error that cannot reach the original Task fails.
test('P9.01 selected Map launch form offers only launch configuration and returns errors to the original Task', async ({ page }, info) => {
  const fixture = await launchFixture()
  fixture.bot.task_id = fixture.ids.manager
  fs.writeFileSync(fixture.botFile, JSON.stringify(fixture.bot))
  const stored = JSON.parse(fs.readFileSync(server!.storageFile, 'utf8'))
  stored.tasks.push({ ...stored.tasks.find((task: { id: string }) => task.id === fixture.ids.manager), id: 'pi-manager', name: 'Pi 管理者', agent: 'pi' })
  fs.writeFileSync(server!.storageFile, JSON.stringify(stored))
  await page.goto(`${server!.baseURL}?layout=expanded`)
  await page.getByTestId('tab-scheduler').click()
  await page.getByRole('button', { name: 'preflight.json', exact: true }).click()
  await page.getByRole('button', { name: '+ 发起 Run', exact: true }).click()
  const form = page.getByRole('form', { name: '发起 Run', exact: true })
  await expect(form).toBeVisible()
  await expect(form.getByLabel('管理者 Task', { exact: true }).locator('option')).toHaveText(['请选择管理者 Task', 'manager', 'Pi 管理者'])
  await form.getByLabel('管理者 Task', { exact: true }).selectOption('pi-manager')
  await form.getByLabel('管理者 zone', { exact: true }).selectOption(fixture.zone)
  await expect(form.getByLabel('Workspace', { exact: true })).toHaveValue('registered-zone')
  await expect(form.getByLabel('Workspace', { exact: true })).toHaveAttribute('readonly', '')
  await expect(form.locator('input,select,textarea')).toHaveCount(4)
  await form.getByLabel('本次要求', { exact: true }).fill('# 本次\n\n保留 **Markdown** 原文')
  await form.getByRole('button', { name: '校验启动配置', exact: true }).click()
  await expect(form.getByRole('alert')).toBeVisible()
  await expect(form).toBeVisible()
  await page.screenshot({ path: info.outputPath('launch-preflight-rejection.png'), fullPage: true })
  await form.getByRole('listitem').filter({ hasText: 'node B：' }).getByRole('button', { name: `修正 Task ${fixture.ids.manager}`, exact: true }).click()
  await expect(page.getByTestId('tab-tasks')).toHaveAttribute('aria-selected', 'true')
  await expect(page.getByTestId('task-dialog')).toBeVisible()
  await expect(page.getByTestId('task-name')).toHaveValue('manager')
  await expect(page.getByTestId('task-type')).toHaveValue('manager')
  expect(server!.herdrCalls()).toEqual([])
  fs.writeFileSync(info.outputPath('form-external-calls.json'), JSON.stringify(server!.herdrCalls()))
})

// P9.01/P7.01: use the delivered page for a stale registered directory and an
// empty Map too, not just request helpers. Fix node B in its original editor.
test('P9.01 P7.01 invalid zone and empty Map remain on form with original configuration correction', async ({ page }, info) => {
  const fixture = await launchFixture()
  fixture.map.nodes.B.zone = `${fixture.zone}-unregistered`
  fs.writeFileSync(fixture.mapFile, JSON.stringify(fixture.map))
  await page.goto(`${server!.baseURL}?layout=expanded`)
  await page.getByTestId('tab-scheduler').click()
  await page.getByRole('button', { name: 'preflight.json', exact: true }).click()
  await page.getByRole('button', { name: '+ 发起 Run', exact: true }).click()
  let form = page.getByRole('form', { name: '发起 Run', exact: true })
  await form.getByLabel('管理者 Task', { exact: true }).selectOption(fixture.ids.manager)
  await form.getByLabel('管理者 zone', { exact: true }).selectOption(fixture.zone)
  await form.getByRole('button', { name: '校验启动配置', exact: true }).click()
  await expect(form.getByRole('alert')).toContainText('没有登记')
  await form.getByRole('button', { name: '修正 node B', exact: true }).click()
  const editor = page.getByRole('form', { name: 'node 编辑', exact: true })
  await expect(editor.getByLabel('node ID', { exact: true })).toHaveValue('B')
  expect(JSON.parse(await editor.getByLabel('node 内容 JSON', { exact: true }).inputValue())).toEqual({ title: '工作', bot_id: 'worker', input: '', zone: `${fixture.zone}-unregistered`, blocked_by: ['A'] })
  await editor.getByLabel('node 内容 JSON', { exact: true }).fill(JSON.stringify({ ...fixture.map.nodes.B, zone: fixture.zone }))
  await editor.getByRole('button', { name: '保存 node', exact: true }).click()
  await expect(editor).toHaveCount(0)
  await form.getByRole('button', { name: '校验启动配置', exact: true }).click()
  await expect(form.getByRole('status')).toContainText('尚未启动，未创建 Run 或 Chat')
  await form.getByRole('button', { name: '返回 Map', exact: true }).click()
  fs.writeFileSync(fixture.mapFile, JSON.stringify({ title: '空图', description: '', nodes: {} }))
  await page.getByRole('button', { name: '刷新全部', exact: true }).click()
  await page.getByRole('button', { name: '+ 发起 Run', exact: true }).click()
  form = page.getByRole('form', { name: '发起 Run', exact: true })
  await form.getByRole('button', { name: '校验启动配置', exact: true }).click()
  await expect(form.getByRole('alert')).toContainText('空 Map 不能启动')
  await expect(form).toBeVisible()
  await page.screenshot({ path: info.outputPath('empty-map-launch-page.png'), fullPage: true })
  expect(server!.herdrCalls()).toEqual([])
  expect(Object.keys(dataFiles(server!.cwd))).not.toEqual(expect.arrayContaining([expect.stringMatching(/run\.json|history\.jsonl|messages\.jsonl/)]))
  fs.writeFileSync(info.outputPath('corrected-node-and-empty-map.json'), JSON.stringify({ files: dataFiles(server!.cwd), calls: server!.herdrCalls() }, null, 2))
})

// P9.01: a missing referenced Task is repaired at the original Bot binding,
// not by navigating only to an editor for a Task that no longer exists.
test('P9.01 missing Task reference returns to the original Bot binding', async ({ page }) => {
  const fixture = await launchFixture()
  fixture.bot.task_id = 'missing-task'
  fs.writeFileSync(fixture.botFile, JSON.stringify(fixture.bot))
  await page.goto(`${server!.baseURL}?layout=expanded`)
  await page.getByTestId('tab-scheduler').click()
  await page.getByRole('button', { name: 'preflight.json', exact: true }).click()
  await page.getByRole('button', { name: '+ 发起 Run', exact: true }).click()
  const form = page.getByRole('form', { name: '发起 Run', exact: true })
  await form.getByLabel('管理者 Task', { exact: true }).selectOption(fixture.ids.manager)
  await form.getByLabel('管理者 zone', { exact: true }).selectOption(fixture.zone)
  await form.getByRole('button', { name: '校验启动配置', exact: true }).click()
  const correction = form.getByRole('listitem').filter({ hasText: 'node B：' }).getByRole('button', { name: '修正 Bot worker', exact: true })
  await expect(correction).toBeVisible()
  await correction.click()
  const editor = page.getByRole('form', { name: 'Bot 编辑', exact: true })
  await expect(editor.getByLabel('Bot ID', { exact: true })).toHaveValue('worker')
  expect(JSON.parse(await editor.getByLabel('Bot 内容 JSON', { exact: true }).inputValue())).toEqual({ name: '', when: '', plan_guide: '', node_guide: '', task_id: 'missing-task' })
  expect(server!.herdrCalls()).toEqual([])
})

// P9.01: HTTP cannot bypass the no-name/no-node-overrides form contract.
test('P9.01 launch preflight rejects temporary overrides and old launch fields without side effects', async ({ request }, info) => {
  const fixture = await launchFixture()
  const before = dataFiles(server!.cwd)
  const evidence: unknown[] = []
  for (const override of [{ name: 'not allowed' }, { nodes: {} }, { workspaceName: 'other' }, { input: 'not userInput' }, { nodeOverrides: { B: { zone: fixture.zone } } }]) {
    const response = await request.post(`${server!.baseURL}/api/scheduler/maps/preflight.json/preflight`, { data: { ...fixture.data, ...override } })
    expect(response.status()).toBe(400)
    evidence.push(await response.json())
    expect(dataFiles(server!.cwd)).toEqual(before)
    expect(server!.herdrCalls()).toEqual([])
  }
  fs.writeFileSync(info.outputPath('forbidden-overrides.json'), JSON.stringify(evidence, null, 2))
})
