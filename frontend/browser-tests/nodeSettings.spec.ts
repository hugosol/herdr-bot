import fs from 'node:fs'
import path from 'node:path'
import { expect, test, type Page } from '@playwright/test'
import { startServer, type E2EServer } from './support'

let server: E2EServer | null = null
test.afterEach(async () => { await server?.cleanup(); server = null })
const legacyNode = { title: '旧节点', bot_id: 'shared', input: '保留原文 {"nested":[1,null]}', zone: 'unregistered-zone', blocked_by: [] as string[] }
const original = { title: '策略编辑', description: '保留说明', nodes: { A: legacyNode, B: { ...legacyNode, title: '另一个节点' }, C: { ...legacyNode, title: '无关节点', blocked_by: ['B'] } } }
async function setup(page: Page) {
  server = await startServer({ separateCwd: true, herdr: {} })
  const root = path.join(server.cwd, 'meta-data/workflow-maps')
  fs.mkdirSync(root, { recursive: true })
  fs.writeFileSync(path.join(root, 'settings.json'), JSON.stringify(original))
  fs.writeFileSync(path.join(root, 'untouched.json'), JSON.stringify(original))
  await page.goto(`${server.baseURL}?layout=expanded`)
  await page.getByTestId('tab-scheduler').click()
  await page.locator('[data-map-file="settings.json"]').click()
  await expect(page.getByRole('region', { name: 'Map 预览', exact: true })).toBeVisible()
  return root
}
async function edit(page: Page, id: string, title: string) {
  await page.getByRole('region', { name: '节点列表', exact: true }).getByRole('button', { name: `${title} ${id}`, exact: true }).click()
  await page.getByRole('button', { name: '修改 node', exact: true }).click()
  await expect(page.getByRole('form', { name: 'node 编辑', exact: true })).toBeVisible()
}
async function save(page: Page) {
  await page.getByRole('button', { name: '保存 node', exact: true }).click()
  await expect(page.getByRole('form', { name: 'node 编辑', exact: true })).toHaveCount(0)
}
async function readMap(page: Page) {
  const response = await page.request.get(`${server!.baseURL}/api/scheduler/maps/settings.json`)
  expect(response.status()).toBe(200)
  return response.json()
}

// P1/P3/P5: independent literal settings for nodes sharing a Bot. Violation:
// a shared setting, stale control state, a select instead of radio buttons, or
// loss of adjacent definitions. Reopen both and read through production HTTP.
test('P1 P3 P5 node settings are independent, radio-selected and explain Tab termination', async ({ page }, info) => {
  const root = await setup(page)
  const untouched = fs.readFileSync(path.join(root, 'untouched.json'), 'utf8')
  const before = await readMap(page)
  expect(before).toMatchObject(original)
  await edit(page, 'A', '旧节点')
  const autoClose = page.getByRole('checkbox', { name: '完成后自动关闭', exact: true })
  const auto = page.getByRole('radio', { name: 'auto', exact: true })
  const manual = page.getByRole('radio', { name: 'manual', exact: true })
  await expect(auto).toBeChecked()
  await expect(manual).not.toBeChecked()
  const help = page.locator('#node-auto-close-help')
  await expect(help).toBeVisible()
  await expect(help).toContainText(/Tab/)
  await expect(help).toContainText(/终止.*终端.*进程/)
  await expect(autoClose).toHaveAttribute('aria-describedby', 'node-auto-close-help')
  await autoClose.check()
  await manual.check()
  await expect(auto).not.toBeChecked()
  await expect(manual).toBeChecked()
  await page.screenshot({ path: info.outputPath('node-settings-controls.png') })
  await save(page)
  await edit(page, 'B', '另一个节点')
  await expect(autoClose).not.toBeChecked()
  await manual.check()
  await auto.check()
  await expect(manual).not.toBeChecked()
  await save(page)
  const after = await readMap(page)
  expect(after).toEqual({ ...before, nodes: { ...before.nodes,
    A: { ...before.nodes.A, auto_close: true, report_policy: 'manual' },
    B: { ...before.nodes.B, auto_close: false, report_policy: 'auto' },
  } })
  expect(fs.readFileSync(path.join(root, 'untouched.json'), 'utf8')).toBe(untouched)
  for (const [id, title, closes, policy] of [['A', '旧节点', true, 'manual'], ['B', '另一个节点', false, 'auto']] as const) {
    await edit(page, id, title)
    await expect(autoClose).toBeChecked({ checked: closes })
    await expect(page.getByRole('radio', { name: policy, exact: true })).toBeChecked()
    await page.getByRole('button', { name: '放弃编辑', exact: true }).click()
  }
  fs.writeFileSync(info.outputPath('independent-node-settings.json'), JSON.stringify({ before, after }, null, 2))
})

// P4 source: ticket's latest-edit-wins rule, not a parallel form model.
// A plausible violation merges stale controls on submit or overwrites malformed
// JSON when a control changes. Alternate both directions, observe full readback,
// and keep invalid raw drafts in the real save/validation path.
test('P4 controls and JSON share the latest draft and retain invalid raw content', async ({ page }) => {
  await setup(page)
  const before = await readMap(page)
  expect(before).toMatchObject(original)
  const json = page.getByLabel('node 内容 JSON', { exact: true })
  const closes = page.getByRole('checkbox', { name: '完成后自动关闭', exact: true })
  const auto = page.getByRole('radio', { name: 'auto', exact: true })
  const manual = page.getByRole('radio', { name: 'manual', exact: true })
  await edit(page, 'A', '旧节点')
  await closes.check()
  await manual.check()
  expect(JSON.parse(await json.inputValue())).toEqual({ ...before.nodes.A, auto_close: true, report_policy: 'manual' })
  await json.fill(JSON.stringify({ ...legacyNode, auto_close: false, report_policy: 'auto' }))
  await expect(closes).not.toBeChecked()
  await expect(auto).toBeChecked()
  await save(page)
  expect((await readMap(page)).nodes.A).toEqual({ ...legacyNode, auto_close: false, report_policy: 'auto' })
  await edit(page, 'A', '旧节点')
  await json.fill(JSON.stringify({ ...legacyNode, auto_close: true, report_policy: 'manual' }))
  await expect(closes).toBeChecked()
  await expect(manual).toBeChecked()
  await closes.uncheck()
  await auto.check()
  expect(JSON.parse(await json.inputValue())).toEqual({ ...legacyNode, auto_close: false, report_policy: 'auto' })
  await json.fill(JSON.stringify({ ...legacyNode, auto_close: false, report_policy: 'auto' }))
  await auto.check()
  await closes.uncheck()
  await manual.check()
  await closes.check()
  await save(page)
  const expected = { ...before, nodes: { ...before.nodes, A: { ...legacyNode, auto_close: true, report_policy: 'manual' } } }
  expect(await readMap(page)).toEqual(expected)
  await edit(page, 'A', '旧节点')
  await expect(closes).toBeChecked()
  await expect(manual).toBeChecked()
  for (const raw of ['{ "auto_close": true,', '[true]', JSON.stringify({ ...legacyNode, auto_close: 'true', report_policy: 'auto' }), JSON.stringify({ ...legacyNode, auto_close: false, report_policy: null })]) {
    await json.fill(raw)
    await expect(closes).toBeDisabled()
    await expect(auto).toBeDisabled()
    await expect(manual).toBeDisabled()
    await page.getByRole('button', { name: '保存 node', exact: true }).click()
    await expect(page.getByRole('form', { name: 'node 编辑', exact: true })).toBeVisible()
    await expect(page.getByRole('alert').filter({ hasText: /JSON|对象|结构/ })).toBeVisible()
    await expect(json).toHaveValue(raw)
    expect(await readMap(page)).toEqual(expected)
  }
  await json.fill(JSON.stringify({ ...legacyNode, auto_close: false, report_policy: 'auto' }))
  await expect(closes).toBeEnabled()
  await closes.check()
  await manual.check()
  await save(page)
  expect(await readMap(page)).toEqual(expected)
})

// P1/P2 S2 validation: optional means absent or a real boolean, not coercion.
// Wrong-type saves must leave the definition bytes unchanged; malformed reads
// must fail rather than silently enabling/disabling a node. No internal doubles.
test('P1 auto_close accepts only booleans through existing definition interfaces', async ({ page }) => {
  const root = await setup(page)
  const file = path.join(root, 'settings.json')
  const before = fs.readFileSync(file, 'utf8')
  const endpoint = `${server!.baseURL}/api/scheduler/maps/settings.json`
  const edited = await page.request.post(`${endpoint}/nodes/edit`, { data: { id: 'A' } })
  expect(edited.status()).toBe(200)
  const draft = await edited.json()
  for (const value of [null, 'true', 'false', 0, 1, [], {}, { enabled: true }]) {
    const response = await page.request.post(`${endpoint}/nodes/save`, { data: { id: 'A', content: JSON.stringify({ ...legacyNode, auto_close: value }), revision: draft.revision, create: false } })
    expect(response.status()).toBe(500)
    const error = await response.json()
    expect(error.code).toBe('node-invalid')
    expect(error.message).toContain('auto_close')
    expect(fs.readFileSync(file, 'utf8')).toBe(before)
    fs.writeFileSync(file, JSON.stringify({ ...original, nodes: { ...original.nodes, A: { ...legacyNode, auto_close: value } } }))
    const invalid = await page.request.get(endpoint)
    expect(invalid.status()).toBe(500)
    expect((await invalid.json()).code).toBe('map-read-failed')
    fs.writeFileSync(file, before)
  }
})

// P6 uses S1 delivered page, S2 real workflow interfaces and S3 external Herdr
// recorder. Only the host is controlled, not policy composition/dispatch/report/
// complete. Violation: wrong prompt, new approval gate, or node completing itself.
// Prompt delivery is evidence of instructions, never of model compliance.
test('P6 saved report policies reach node dispatch without approval gates or changed manager authority', async ({ page }, info) => {
  await setup(page)
  const zone = path.join(server!.cwd, 'bot-playground')
  fs.mkdirSync(zone)
  const post = (route: string, data: object) => page.request.post(`${server!.baseURL}${route}`, { data })
  expect((await post('/api/zones', { path: zone })).status()).toBe(200)
  const ids: Record<string, string> = {}
  for (const role of ['manager', 'normal']) {
    const response = await post('/api/tasks', { name: role, taskType: role, agent: 'omp', chatName: '{chat.id}', model: 'default', thinking: 'default', skill: null, prompt: 'FIXED ROLE', extensions: [] })
    expect(response.status()).toBe(200)
    ids[role] = (await response.json()).tasks.find((task: { name: string }) => task.name === role).id
  }
  const bots = path.join(server!.cwd, 'meta-data/bots')
  fs.writeFileSync(path.join(bots, 'shared.json'), JSON.stringify({ id: 'shared', name: '共享', when: '', plan_guide: '', node_guide: '', task_id: ids.normal }))
  for (const [id, title, policy] of [['A', '旧节点', 'auto'], ['B', '另一个节点', 'manual'], ['C', '无关节点', 'auto']] as const) {
    await edit(page, id, title)
    await page.getByLabel('node 内容 JSON', { exact: true }).fill(JSON.stringify({ ...legacyNode, title, zone, auto_close: false }))
    await page.getByRole('radio', { name: policy, exact: true }).check()
    await save(page)
  }
  server!.setHerdrBehavior({ native_session: { source: 'herdr:omp', agent: 'omp', kind: 'id', value: 'settings-manager' } })
  const started = await post('/api/scheduler/maps/settings.json/runs', { managerTaskId: ids.manager, zone, userInput: '验证节点策略派发' })
  expect(started.status(), await started.text()).toBe(201)
  const { id: runId } = await started.json()
  const prompts: Record<string, string> = {}
  for (const id of ['A', 'B']) {
    server!.setHerdrBehavior({ native_session: { source: 'herdr:omp', agent: 'omp', kind: 'id', value: `settings-node-${id}` } })
    const dispatched = await post('/api/workflow/manager/dispatch', { sessionId: 'settings-manager', nodes: [id] })
    expect(dispatched.status()).toBe(200)
    expect((await dispatched.json()).results[0].status).toBe('running')
    const prompt = server!.herdrCalls().find(call => call.kind === 'agent_prompt' && String(call.text).includes(`Node ID: ${id}\n`))
    expect(prompt).toBeDefined()
    prompts[id] = String(prompt!.text)
  }
  expect(prompts.A).toContain('report_policy: auto')
  expect(prompts.A).toContain('Report handoff-ready results without waiting for user confirmation')
  expect(prompts.B).toContain('report_policy: manual')
  expect(prompts.B).toContain('show the user a summary, result locations and limitations in this node Chat')
  expect(prompts.B).toContain('If changes are requested, make them first, show the revised result and ask again')
  expect(prompts.B).toContain('stop the turn and wait')
  expect(prompts.B).toContain('only after the user confirms in natural language here')
  expect(prompts.B).toContain('Supplements and corrections also require this confirmation')
  for (const text of Object.values(prompts)) expect(text).toContain('the manager still decides complete')
  for (const [node, body] of [['A', '自动结果 result-a.md'], ['B', '人工策略结果 result-b.md'], ['B', '补充 result-b-extra.md'], ['B', '更正 result-b-corrected.md']]) {
    const report = await post('/api/workflow/node/report', { sessionId: `settings-node-${node}`, body })
    expect(report.status()).toBe(200)
  }
  const unauthorized = await post('/api/workflow/manager/complete', { sessionId: 'settings-node-B', nodeId: 'B', reason: '不能自行完成' })
  expect(unauthorized.status()).toBe(403)
  expect((await unauthorized.json()).code).toBe('manager-unassociated')
  const status = await post('/api/workflow/manager/status', { sessionId: 'settings-manager' })
  const running = await status.json()
  expect(running.snapshot.nodes.A.status).toBe('running')
  expect(running.snapshot.nodes.B.status).toBe('running')
  for (const nodeId of ['A', 'B']) {
    const complete = await post('/api/workflow/manager/complete', { sessionId: 'settings-manager', nodeId, reason: '管理者核实结果' })
    expect(complete.status()).toBe(200)
    expect((await complete.json()).snapshot.nodes[nodeId].status).toBe('completed')
  }
  await page.goto(`${server!.baseURL}/#workflow/${runId}`)
  const navigation = page.getByRole('navigation', { name: '已保存节点状态' })
  await expect(navigation.locator('[data-node-id="B"]')).toHaveAttribute('data-node-state', 'completed')
  await expect(page.getByRole('button', { name: /审批|approve/i })).toHaveCount(0)
  const detail = await (await page.request.get(`${server!.baseURL}/api/workflow/runs/${runId}`)).json()
  expect(detail.reports.map((report: { body: string }) => report.body)).toEqual(['自动结果 result-a.md', '人工策略结果 result-b.md', '补充 result-b-extra.md', '更正 result-b-corrected.md'])
  fs.writeFileSync(info.outputPath('report-policy-dispatch.json'), JSON.stringify({ prompts, running, detail }, null, 2))
})

// Approved S1 delivered browser + S2 HTTP. P2's independent expectation is false
// for both missing legacy configuration and new drafts. Violation: absence enables
// auto-close, or save invents true. Observe controls AND authoritative readback.
test('P2 new drafts and legacy nodes keep auto-close off when configuration is omitted', async ({ page }) => {
  await setup(page)
  await edit(page, 'A', '旧节点')
  await expect(page.getByRole('checkbox', { name: '完成后自动关闭', exact: true })).not.toBeChecked()
  await save(page)
  expect((await readMap(page)).nodes.A.auto_close).toBe(false)
  await edit(page, 'A', '旧节点')
  await expect(page.getByRole('checkbox', { name: '完成后自动关闭', exact: true })).not.toBeChecked()
  await page.getByRole('button', { name: '放弃编辑', exact: true }).click()
  await page.getByRole('button', { name: '新增 node', exact: true }).click()
  await expect(page.getByRole('checkbox', { name: '完成后自动关闭', exact: true })).not.toBeChecked()
  await page.getByLabel('node ID', { exact: true }).fill('NEW')
  await save(page)
  expect((await readMap(page)).nodes.NEW.auto_close).toBe(false)
  await page.getByRole('button', { name: '修改 node', exact: true }).click()
  await expect(page.getByRole('checkbox', { name: '完成后自动关闭', exact: true })).not.toBeChecked()
})
