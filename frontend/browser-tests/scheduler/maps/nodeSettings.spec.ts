import fs from 'node:fs'
import { openNodeJson } from '../../support/ui'
import path from 'node:path'
import { expect, test, type Page } from '@playwright/test'
import { startServer, type E2EServer } from '../../support/support'

let server: E2EServer | null = null
test.afterEach(async () => { await server?.cleanup(); server = null })
const legacyNode = { title: '旧节点', bot_id: 'shared', input: '保留原文 {"nested":[1,null]}', zone: 'unregistered-zone', blocked_by: [] as string[] }
const original = { title: '策略编辑', description: '保留说明', nodes: { A: legacyNode, B: { ...legacyNode, title: '另一个节点', output_rule: [{ id: 'result' }] }, C: { ...legacyNode, title: '无关节点', blocked_by: ['B'] } } }
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
  await openNodeJson(page)
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
  const autoClose = page.getByRole('checkbox', { name: 'auto_close', exact: true })
  const auto = page.getByRole('radio', { name: 'auto', exact: true })
  const manual = page.getByRole('radio', { name: 'manual', exact: true })
  // P1 defines omitted as true, not an independently persisted third mode.
  // The source fixture omits the field; public normalization/readback is true.
  await expect(auto).toBeChecked()
  await save(page)
  expect((await readMap(page)).nodes.A.auto_approved).toBe(true)
  await edit(page, 'A', '旧节点')
  await expect(auto).toBeChecked()
  await expect(manual).not.toBeChecked()
  await expect(autoClose).toHaveAccessibleDescription(/关闭 Tab.*终止.*终端.*进程/)
  await expect(page.getByText('关闭 Tab 会终止 Tab 内终端及相关进程。', { exact: true })).toBeVisible()
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
    A: { ...before.nodes.A, auto_close: true, auto_approved: false },
    B: { ...before.nodes.B, auto_close: false, auto_approved: true },
  } })
  expect(fs.readFileSync(path.join(root, 'untouched.json'), 'utf8')).toBe(untouched)
  for (const [id, title, closes, policy] of [['A', '旧节点', true, 'manual'], ['B', '另一个节点', false, 'auto']] as const) {
    await edit(page, id, title)
    await expect(autoClose).toBeChecked({ checked: closes })
    await expect(page.getByRole('radio', { name: policy, exact: true })).toBeChecked()
    const calls = server!.herdrCalls()
    const definition = await readMap(page)
    await page.screenshot({ path: info.outputPath(`saved-${id}-${policy}.png`), fullPage: true })
    expect(server!.herdrCalls()).toEqual(calls)
    fs.writeFileSync(info.outputPath(`saved-${id}-${policy}.json`), JSON.stringify({ definition, callsBefore: calls, callsAfter: server!.herdrCalls() }, null, 2))
    await page.getByRole('button', { name: '放弃编辑', exact: true }).click()
  }
  fs.writeFileSync(info.outputPath('independent-node-settings.json'), JSON.stringify({ before, after }, null, 2))
})

// Independent settings never rewrite raw content; saving merges every content field.
test('P4 independent settings merge content and retain invalid raw drafts', async ({ page }) => {
  await setup(page)
  const before = await readMap(page)
  const json = page.getByLabel('node 内容 JSON', { exact: true })
  const zone = page.getByRole('textbox', { name: 'zone', exact: true })
  const closes = page.getByRole('checkbox', { name: 'auto_close', exact: true })
  const manual = page.getByRole('radio', { name: 'manual', exact: true })
  await edit(page, 'A', '旧节点')
  const content = { title: legacyNode.title, bot_id: legacyNode.bot_id, input: legacyNode.input, blocked_by: [], input_rule: [{ id: 'source', type: 'node', node_id: 'B', output_id: 'result' }], output_rule: [] }
  expect(JSON.parse(await json.inputValue())).toEqual({ title: legacyNode.title, bot_id: legacyNode.bot_id, input: legacyNode.input, blocked_by: [], input_rule: [], output_rule: [] })
  const raw = '  ' + JSON.stringify(content, null, 3) + '\n'
  await json.fill(raw)
  await zone.fill('another-unregistered-zone')
  await closes.check()
  await manual.check()
  await expect(json).toHaveValue(raw)
  await save(page)
  const expected = { ...before, nodes: { ...before.nodes, A: { ...content, zone: 'another-unregistered-zone', auto_close: true, auto_approved: false, end_node: false } } }
  expect(await readMap(page)).toEqual(expected)
  await edit(page, 'A', '旧节点')
  await expect(zone).toHaveValue('another-unregistered-zone')
  await expect(closes).toBeChecked()
  await expect(manual).toBeChecked()
  for (const invalid of ['{ "input": ', '[true]', JSON.stringify({ ...content, zone: 'conflicting' }), JSON.stringify({ ...content, auto_close: false }), JSON.stringify({ ...content, auto_approved: true }), JSON.stringify({ ...content, node_id: 'B' })]) {
    await json.fill(invalid)
    await expect(closes).toBeEnabled()
    await closes.uncheck()
    await expect(json).toHaveValue(invalid)
    await page.getByRole('button', { name: '保存 node', exact: true }).click()
    await expect(page.getByRole('form', { name: 'node 编辑', exact: true })).toBeVisible()
    await expect(page.getByRole('alert').filter({ hasText: /JSON|对象|移除/ })).toBeVisible()
    await expect(json).toHaveValue(invalid)
    expect(await readMap(page)).toEqual(expected)
  }
  await json.fill(raw)
  await closes.check()
  await save(page)
  expect(await readMap(page)).toEqual(expected)
  await edit(page, 'A', '旧节点')
  await zone.fill('discard-this-zone')
  page.once('dialog', dialog => dialog.accept())
  await page.getByRole('button', { name: '放弃编辑', exact: true }).click()
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

// P3/P36, approved seam S3: current built page plus independent public HTTP.
// A plausible violation paints the edited Map's auto mode onto the frozen manual
// execution, or carries its approval into retry. Literal expected facts below
// distinguish both; only external Herdr is controlled, never Run persistence.
// HTTP manual submission proves display of the recorded source, not native Yes.
test('P3 P36 current definition frozen mode and retry approval facts remain distinct', async ({ page }, info) => {
  await setup(page)
  const zone = path.join(server!.cwd, 'bot-playground')
  fs.mkdirSync(zone)
  const post = async (route: string, data: object, status = 200) => {
    const response = await page.request.post(server!.baseURL + route, { data })
    expect(response.status(), await response.text()).toBe(status)
    return response.json()
  }
  await post('/api/zones', { id: 'test/zone', path: zone })
  const tasks: Record<string, string> = {}
  for (const role of ['manager', 'normal']) {
    const result = await post('/api/tasks', { name: role, taskType: role, agent: 'omp', chatName: '{chat.id}', model: 'default', thinking: 'default', skill: null, prompt: 'APPROVAL PAGE FIXTURE' })
    tasks[role] = result.tasks.find((task: { name: string }) => task.name === role).id
  }
  fs.writeFileSync(path.join(server!.cwd, 'meta-data/bots/shared.json'), JSON.stringify({ id: 'shared', name: '共享', when: '', plan_guide: '', node_guide: '', task_id: tasks.normal }))
  for (const [id, title, mode] of [['A', '旧节点', 'manual'], ['B', '另一个节点', 'auto'], ['C', '无关节点', 'auto']] as const) {
    await edit(page, id, title)
    await (await openNodeJson(page)).fill(JSON.stringify({ title, bot_id: 'shared', input: 'APPROVAL BUSINESS', blocked_by: id === 'B' ? ['A'] : [], input_rule: [], output_rule: [] }))
    await page.getByRole('textbox', { name: 'zone', exact: true }).fill('test/zone')
    await page.getByRole('radio', { name: mode, exact: true }).check()
    await save(page)
  }
  const native = (sessionId: string) => server!.setHerdrBehavior({ native_session: { source: 'herdr:omp', agent: 'omp', kind: 'id', value: sessionId }, native_cwd: zone })
  native('settings-manager')
  const started = await post('/api/scheduler/maps/settings.json/runs', { managerTaskId: tasks.manager, zone: 'test/zone', userInput: 'Inspect approval facts without business redispatch' }, 201)
  await post('/api/workflow/manager/prepare_inputs', { sessionId: 'settings-manager', fileInputs: [], confirmed: true, reason: 'No named materials required' })
  native('settings-manual')
  const dispatched = await post('/api/workflow/manager/dispatch', { sessionId: 'settings-manager', nodes: ['A'] })
  expect(dispatched.results[0].status).toBe('running')
  const oldId = dispatched.snapshot.nodes.A.execution.id
  const url = server!.baseURL + '/#workflow/' + started.id
  const facts = page.getByRole('region', { name: '当前执行事实', exact: true })
  const field = (name: string) => facts.locator('dt').filter({ hasText: new RegExp('^' + name + '$') }).locator('+ dd')
  const read = async () => (await page.request.get(server!.baseURL + '/api/workflow/runs/' + started.id)).json()
  const capture = async (name: string) => {
    const before = server!.herdrCalls()
    const detail = await read(), definition = await readMap(page)
    await page.getByRole('button', { name: '刷新 Run', exact: true }).click()
    await expect(field('id')).toContainText(detail.run.nodes.A.execution.id)
    const image = await page.screenshot({ path: info.outputPath(name + '.png'), fullPage: true })
    await info.attach(name, { body: image, contentType: 'image/png' })
    expect(server!.herdrCalls()).toEqual(before)
    const json = JSON.stringify({ definition, detail, callsBefore: before, callsAfter: server!.herdrCalls() }, null, 2)
    fs.writeFileSync(info.outputPath(name + '.json'), json)
    await info.attach(name + '-public-facts', { body: json, contentType: 'application/json' })
    return detail
  }
  await page.goto(url)
  await expect(field('autoApproved')).toHaveText('false')
  await expect(field('approved')).toHaveText('false · 尚未批准')
  await expect(field('approvalSource')).toHaveText('无')
  await expect(page.getByText(/report_policy|reportPolicy/)).toHaveCount(0)
  await capture('manual-before-approval')
  await page.goto(server!.baseURL + '?layout=expanded')
  await page.getByTestId('tab-scheduler').click()
  await page.locator('[data-map-file="settings.json"]').click()
  await edit(page, 'A', '旧节点')
  await page.getByRole('radio', { name: 'auto', exact: true }).check()
  await save(page)
  expect((await readMap(page)).nodes.A.auto_approved).toBe(true)
  await page.goto(url)
  await expect(field('autoApproved')).toHaveText('false')
  const definition = page.getByRole('group', { name: '当前节点定义', exact: true })
  await definition.locator('summary').click()
  await expect(definition.locator('dt').filter({ hasText: /^auto_approved$/ }).locator('+ dd')).toHaveText('true')
  await capture('changed-map-frozen-manual')
  await post('/api/workflow/node/approve', { sessionId: 'settings-manual', executionId: oldId, source: 'manual' })
  await page.getByRole('button', { name: '刷新 Run', exact: true }).click()
  await expect(field('approved')).toHaveText('true · 已批准')
  await expect(field('approvalSource')).toHaveText('manual · 人工批准')
  const approved = await capture('manual-approved')
  expect(approved.run.nodes.A.execution).toMatchObject({ autoApproved: false, approved: true, approvalSource: 'manual' })
  expect(approved.reports).toEqual([])
  await post('/api/workflow/manager/fail', { sessionId: 'settings-manager', nodeId: 'A', reason: 'Independent new attempt needed' })
  native('settings-auto-retry')
  const retry = await post('/api/workflow/manager/retry', { sessionId: 'settings-manager', nodeId: 'A', confirmed: true, reason: 'Old effects inspected' })
  const newId = retry.snapshot.nodes.A.execution.id
  expect(newId).not.toBe(oldId)
  await page.getByRole('button', { name: '刷新 Run', exact: true }).click()
  await expect(field('autoApproved')).toHaveText('true')
  await expect(field('approved')).toHaveText('false · 尚未批准')
  await expect(field('approvalSource')).toHaveText('无')
  const history = page.getByRole('group', { name: '已保存旧执行事实', exact: true })
  await history.locator(':scope > summary').click()
  await expect(history).toContainText(oldId)
  await expect(history).toContainText('manual · 人工批准')
  const pending = await capture('retry-unapproved-and-old-manual')
  expect(pending.run.nodes.A.previousExecutions[0].execution).toMatchObject({ id: oldId, autoApproved: false, approved: true, approvalSource: 'manual' })
  expect(pending.run.nodes.A.execution).toMatchObject({ id: newId, autoApproved: true, approved: false, approvalSource: null })
  await post('/api/workflow/node/approve', { sessionId: 'settings-auto-retry', executionId: newId, source: 'auto' })
  await page.getByRole('button', { name: '刷新 Run', exact: true }).click()
  await expect(field('approvalSource')).toHaveText('auto · 自动批准（非用户 Yes）')
  const automatic = await capture('retry-auto-approved')
  expect(automatic.run.nodes.A.status).toBe('running')
  expect(automatic.run.nodes.B.status).toBe('pending')
  expect(automatic.reports).toEqual([])
  await post('/api/workflow/node/report', { sessionId: 'settings-auto-retry', body: 'Progress without sending confirmation', canComplete: false })
  await post('/api/workflow/manager/complete', { sessionId: 'settings-manager', nodeId: 'A', reason: 'Not ready' }, 409)
  await post('/api/workflow/node/report', { sessionId: 'settings-auto-retry', body: 'Ready result', canComplete: true })
  await post('/api/workflow/manager/complete', { sessionId: 'settings-auto-retry', nodeId: 'A', reason: 'Wrong actor' }, 403)
  await post('/api/workflow/manager/complete', { sessionId: 'settings-manager', nodeId: 'A', reason: 'Reviewed latest ready result' })
  await capture('completed-with-reports')
  await expect(page.getByRole('region', { name: '当前执行适用报告', exact: true })).toContainText('Ready result')
  await expect(page.getByRole('button', { name: /审批|approve/i })).toHaveCount(0)
})

// Approved S1 delivered browser + S2 HTTP. P2's independent expectation is false
// for both missing legacy configuration and new drafts. Violation: absence enables
// auto-close, or save invents true. Observe controls AND authoritative readback.
test('P2 new drafts and legacy nodes keep auto-close off when configuration is omitted', async ({ page }) => {
  await setup(page)
  await edit(page, 'A', '旧节点')
  await expect(page.getByRole('checkbox', { name: 'auto_close', exact: true })).not.toBeChecked()
  await save(page)
  expect((await readMap(page)).nodes.A.auto_close).toBe(false)
  await edit(page, 'A', '旧节点')
  await expect(page.getByRole('checkbox', { name: 'auto_close', exact: true })).not.toBeChecked()
  await page.getByRole('button', { name: '放弃编辑', exact: true }).click()
  await page.getByRole('button', { name: '新增 node', exact: true }).click()
  await expect(page.getByRole('checkbox', { name: 'auto_close', exact: true })).not.toBeChecked()
  await page.getByLabel('node_id', { exact: true }).fill('NEW')
  await save(page)
  expect((await readMap(page)).nodes.NEW.auto_close).toBe(false)
  await page.getByRole('button', { name: '修改 node', exact: true }).click()
  await expect(page.getByRole('checkbox', { name: 'auto_close', exact: true })).not.toBeChecked()
})
