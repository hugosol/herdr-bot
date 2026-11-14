import fs from 'node:fs'
import path from 'node:path'
import { expect, test, type Page, type TestInfo } from '@playwright/test'
import { startServer, type E2EServer } from '../support/support'

let server: E2EServer | null = null
test.afterEach(async () => { await server?.cleanup(); server = null })

async function saveEvidence(info: TestInfo, name: string, value: unknown) {
  const file = info.outputPath(`${name}.json`)
  fs.writeFileSync(file, JSON.stringify(value, null, 2))
  await info.attach(name, { path: file, contentType: 'application/json' })
}

async function nestedFixture(page: Page) {
  const host = server = await startServer({ separateCwd: true, herdr: { workspaceName: 'navigation-zone', workspaces: [{ workspace_id: 'navigation-workspace', label: 'navigation-zone' }] } })
  const zone = path.join(path.dirname(host.configPath), 'navigation-zone')
  fs.mkdirSync(zone)
  const post = async (route: string, data: unknown, status = 200) => {
    const response = await page.request.post(`${host.baseURL}${route}`, { data })
    expect(response.status(), await response.text()).toBe(status)
    return response.json()
  }
  await post('/api/zones', { id: 'zone', path: zone })
  const task = async (name: string, taskType: string) => {
    const value = await post('/api/tasks', { name, taskType, agent: 'omp', chatName: '{chat.id}', model: 'default', thinking: 'default', prompt: '' })
    return value.tasks.find((item: { name: string }) => item.name === name).id as string
  }
  const manager = await task('Navigation manager', 'manager'), worker = await task('Navigation worker', 'normal')
  const maps = path.join(host.cwd, 'meta-data/workflow-maps'), bots = path.join(host.cwd, 'meta-data/bots')
  fs.mkdirSync(maps, { recursive: true }); fs.mkdirSync(bots, { recursive: true })
  fs.writeFileSync(path.join(bots, 'nested.json'), JSON.stringify({ id: 'nested', name: 'Same name', when: '', plan_guide: '', node_guide: '', map_file: 'child.json' }))
  fs.writeFileSync(path.join(bots, 'worker.json'), JSON.stringify({ id: 'worker', name: 'Same name', when: '', plan_guide: '', node_guide: '', task_id: worker }))
  fs.writeFileSync(path.join(maps, 'child.json'), JSON.stringify({ title: 'Same name', description: '', nodes: { A: { title: 'Same name', bot_id: 'worker', input: 'CHILD INTERNAL INPUT', zone: 'zone', blocked_by: [] } } }))
  fs.writeFileSync(path.join(maps, 'root.json'), JSON.stringify({ title: 'Same name', description: '', nodes: { A: { title: 'Same name', bot_id: 'nested', manager_task_id: manager, input: 'CHILD NAVIGATION INPUT', zone: 'zone', blocked_by: [] } } }))
  const native = (sessionId: string) => host.setHerdrBehavior({ native_session: { source: 'herdr:omp', agent: 'omp', kind: 'id', value: sessionId }, native_cwd: zone })
  native('navigation-root')
  const stream = page.waitForResponse(response => response.url() === `${host.baseURL}/api/sessions/events` && response.request().method() === 'GET')
  await page.goto(`${host.baseURL}?layout=expanded`)
  const streamResponse = await stream
  expect(streamResponse.status()).toBe(200)
  expect(streamResponse.headers()['content-type']).toContain('text/event-stream')
  await page.getByTestId('tab-scheduler').click()
  await page.locator('button[data-map-file="root.json"]').click()
  await page.getByRole('button', { name: '+ 发起 Run', exact: true }).click()
  await page.getByRole('combobox', { name: '管理者 Task' }).selectOption(manager)
  await page.getByRole('combobox', { name: '管理者 zone' }).selectOption('zone')
  await page.getByRole('textbox', { name: '本次要求', exact: true }).fill('ROOT NAVIGATION INPUT')
  await page.getByRole('button', { name: '启动 Run', exact: true }).click()
  await expect(page.getByRole('heading', { name: 'Run 详情', exact: true })).toBeVisible()
  const rootId = page.url().split('#workflow/')[1]
  await post('/api/workflow/manager/prepare_inputs', { sessionId: 'navigation-root', fileInputs: [], confirmed: true, reason: 'Reviewed navigation fixture inputs' })
  native('navigation-old-child')
  const dispatch = await post('/api/workflow/manager/dispatch', { sessionId: 'navigation-root', nodes: ['A'] })
  expect(dispatch.results[0].status).toBe('running')
  return { host, rootId, post, native, old: dispatch.snapshot.nodes.A.execution }
}

// P36 approved seams: delivered page, public HTTP, owned persisted files and
// recording-safe Herdr. Literal input/identity expectations distinguish real
// navigation from a static link; retry makes a latest-child-only link wrong.
test('P36 root and child Runs remain peers and current and retained executions navigate to their own persisted child', async ({ page }, info) => {
  const { host, rootId, post, native, old } = await nestedFixture(page)
  await post('/api/workflow/manager/fail', { sessionId: 'navigation-root', nodeId: 'A', reason: 'Inspect old effects before retry' })
  native('navigation-current-child')
  const retried = await post('/api/workflow/manager/retry', { sessionId: 'navigation-root', nodeId: 'A', confirmed: true, reason: 'Explicit new attempt' })
  const current = retried.snapshot.nodes.A.execution
  expect(current.childRunId).not.toBe(old.childRunId)
  expect(retried.snapshot.nodes.A.previousExecutions[0].execution.childRunId).toBe(old.childRunId)
  for (const id of [rootId, old.childRunId, current.childRunId]) {
    const response = await page.request.patch(`${host.baseURL}/api/workflow/runs/${id}`, { data: { name: 'Same name' } })
    expect(response.ok()).toBe(true)
  }
  const runRoot = path.join(host.cwd, '.data/workflow-runs')
  const ids = fs.readdirSync(runRoot).sort()
  const before = ids.map(id => ['run.json', 'history.jsonl', 'messages.jsonl'].map(file => fs.readFileSync(path.join(runRoot, id, file), 'utf8')))
  const calls = host.herdrCalls()
  await page.getByRole('button', { name: '刷新 Run', exact: true }).click()
  const currentFacts = page.getByRole('region', { name: '当前执行事实', exact: true })
  await expect(currentFacts).toContainText(current.id)
  await currentFacts.getByRole('link', { name: '查看子 Run', exact: true }).click()
  await expect(page).toHaveURL(url => url.origin === host.baseURL && url.hash === `#workflow/${current.childRunId}`)
  await expect(page.getByTestId('run-user-input')).toHaveText('CHILD NAVIGATION INPUT')
  const currentRead = await (await page.request.get(`${host.baseURL}/api/workflow/runs/${current.childRunId}`)).json()
  expect(currentRead.run.parent).toEqual({ runId: rootId, nodeId: 'A', executionId: current.id })
  const childRefresh = page.waitForResponse(response => response.url() === `${host.baseURL}/api/workflow/runs/${current.childRunId}` && response.request().method() === 'GET')
  await page.getByRole('button', { name: '刷新 Run', exact: true }).click()
  expect((await childRefresh).status()).toBe(200)
  await expect(page.getByTestId('run-user-input')).toHaveText('CHILD NAVIGATION INPUT')
  const childScreenshot = info.outputPath('current-child-page.png')
  await page.screenshot({ fullPage: true, path: childScreenshot })
  await info.attach('current-child-page', { path: childScreenshot, contentType: 'image/png' })
  // Whole-page reload intentionally starts in chat (the existing navigation
  // contract); returning through workflow must still find the same child and
  // must not create/re-dispatch anything.
  await page.reload()
  await expect(page.getByRole('tab', { name: 'chat', exact: true })).toHaveAttribute('aria-selected', 'true')
  await page.getByTestId('tab-workflow').click()
  await page.getByRole('region', { name: 'Run 列表', exact: true }).getByRole('row').filter({ hasText: current.childRunId }).getByRole('link').click()
  await expect(page.getByTestId('run-user-input')).toHaveText('CHILD NAVIGATION INPUT')
  await page.goto(`${host.baseURL}/#workflow/${rootId}`)
  const history = page.getByLabel('已保存旧执行事实', { exact: true })
  await history.locator(':scope > summary').click()
  await expect(history).toContainText(old.id)
  await history.getByRole('link', { name: '查看子 Run', exact: true }).click()
  await expect(page).toHaveURL(url => url.origin === host.baseURL && url.hash === `#workflow/${old.childRunId}`)
  await expect(page.getByTestId('run-user-input')).toHaveText('CHILD NAVIGATION INPUT')
  const oldRead = await (await page.request.get(`${host.baseURL}/api/workflow/runs/${old.childRunId}`)).json()
  expect(oldRead.run.parent).toEqual({ runId: rootId, nodeId: 'A', executionId: old.id })
  await page.getByRole('link', { name: '返回全部 Run', exact: true }).click()
  const list = page.getByRole('region', { name: 'Run 列表', exact: true })
  await expect(list.getByRole('row')).toHaveCount(3)
  const listScreenshot = info.outputPath('parallel-run-list.png')
  await page.screenshot({ fullPage: true, path: listScreenshot })
  await info.attach('parallel-run-list', { path: listScreenshot, contentType: 'image/png' })
  for (const id of ids) {
    const row = list.getByRole('row').filter({ hasText: id })
    await expect(row).toHaveCount(1)
    await row.getByRole('link').click()
    await expect(page).toHaveURL(url => url.origin === host.baseURL && url.hash === `#workflow/${id}`)
    await expect(page.getByTestId('run-user-input')).toHaveText(id === rootId ? 'ROOT NAVIGATION INPUT' : 'CHILD NAVIGATION INPUT')
    await page.getByRole('link', { name: '返回全部 Run', exact: true }).click()
  }
  expect(fs.readdirSync(runRoot).sort()).toEqual(ids)
  expect(ids.map(id => ['run.json', 'history.jsonl', 'messages.jsonl'].map(file => fs.readFileSync(path.join(runRoot, id, file), 'utf8')))).toEqual(before)
  expect(host.herdrCalls()).toEqual(calls)
  await saveEvidence(info, 'navigation-facts', { rootId, old, current, currentRead, oldRead, ids, externalCalls: calls })
})

// Missing persisted association must be reported, not reconstructed from the
// same-named existing child, Bot, Map or Chat. Restart removes process ownership
// so the page must read the deliberately incomplete, real historical records.
test('P36 missing current and retained child associations stay explicit without guessing existing children', async ({ page }, info) => {
  const { host, rootId, post, native, old } = await nestedFixture(page)
  await post('/api/workflow/manager/fail', { sessionId: 'navigation-root', nodeId: 'A', reason: 'Keep old attempt' })
  native('navigation-current-child')
  await post('/api/workflow/manager/retry', { sessionId: 'navigation-root', nodeId: 'A', confirmed: true, reason: 'New attempt' })
  await host.stop()
  const runRoot = path.join(host.cwd, '.data/workflow-runs')
  const parentPath = path.join(runRoot, rootId, 'run.json')
  const parent = JSON.parse(fs.readFileSync(parentPath, 'utf8'))
  delete parent.nodes.A.execution.childRunId
  delete parent.nodes.A.previousExecutions[0].execution.childRunId
  fs.writeFileSync(parentPath, JSON.stringify(parent))
  const before = fs.readFileSync(parentPath, 'utf8'), calls = host.herdrCalls()
  await host.restart()
  await page.goto(`${host.baseURL}/#workflow/${rootId}`)
  const current = page.getByRole('region', { name: '当前执行事实', exact: true })
  await expect(current).toContainText(/childRunId.*未保存关联/)
  await expect(current.getByRole('link', { name: '查看子 Run', exact: true })).toHaveCount(0)
  const history = page.getByLabel('已保存旧执行事实', { exact: true })
  await history.locator(':scope > summary').click()
  await expect(history).toContainText(old.id)
  await expect(history).toContainText(/childRunId.*未保存关联/)
  await expect(history.getByRole('link', { name: '查看子 Run', exact: true })).toHaveCount(0)
  await page.getByRole('button', { name: '刷新 Run', exact: true }).click()
  await expect(current).toContainText(/childRunId.*未保存关联/)
  const detail = await (await page.request.get(`${host.baseURL}/api/workflow/runs/${rootId}`)).json()
  expect(detail.run).toEqual(parent)
  expect((await (await page.request.get(`${host.baseURL}/api/workflow/runs/${old.childRunId}`)).json()).run.parent).toEqual({ runId: rootId, nodeId: 'A', executionId: old.id })
  expect(fs.readFileSync(parentPath, 'utf8')).toBe(before)
  expect(fs.readdirSync(runRoot)).toHaveLength(3)
  expect(host.herdrCalls()).toEqual(calls)
  await saveEvidence(info, 'missing-association-facts', { detail, existingOldChild: old.childRunId })
})

// Existing public read errors must survive the new navigation. The child is
// really missing or an OS directory at the required file path, not a mocked
// HTTP failure. A same-named independent Run remains readable as a decoy.
for (const condition of ['missing', 'unreadable'] as const) {
  test(`P36 ${condition} child target reports its real read failure rather than opening a same-named Run`, async ({ page }, info) => {
    const { host, rootId, post, native, old } = await nestedFixture(page)
    native('navigation-decoy')
    const decoy = await post('/api/scheduler/maps/root.json/runs', { managerTaskId: old.taskId, zone: 'zone', userInput: 'DECOY MUST NOT OPEN' }, 201)
    await host.stop()
    const runRoot = path.join(host.cwd, '.data/workflow-runs')
    const target = path.join(runRoot, old.childRunId, 'run.json')
    fs.renameSync(target, `${target}.saved`)
    if (condition === 'unreadable') fs.mkdirSync(target)
    const parentPath = path.join(runRoot, rootId, 'run.json')
    const before = fs.readFileSync(parentPath, 'utf8'), calls = host.herdrCalls()
    const ids = fs.readdirSync(runRoot).sort()
    await host.restart()
    await page.goto(`${host.baseURL}/#workflow/${rootId}`)
    const current = page.getByRole('region', { name: '当前执行事实', exact: true })
    await expect(current).toContainText(old.childRunId)
    const failedRead = page.waitForResponse(response => response.url() === `${host.baseURL}/api/workflow/runs/${old.childRunId}` && response.request().method() === 'GET')
    await current.getByRole('link', { name: '查看子 Run', exact: true }).click()
    const response = await failedRead
    expect(response.status()).toBe(500)
    const failure = await response.json()
    expect(failure.code).toBe('run-read-failed')
    await expect(page).toHaveURL(url => url.origin === host.baseURL && url.hash === `#workflow/${old.childRunId}`)
    await expect(page.getByRole('alert').filter({ hasText: '更新失败' })).toBeVisible()
    await expect(page.getByRole('alert').filter({ hasText: '更新失败' })).toContainText(failure.message)
    await expect(page.getByTestId('run-user-input')).toHaveCount(0)
    const secondRead = page.waitForResponse(response => response.url() === `${host.baseURL}/api/workflow/runs/${old.childRunId}` && response.request().method() === 'GET')
    await page.getByRole('button', { name: '刷新 Run', exact: true }).click()
    expect((await secondRead).status()).toBe(500)
    expect((await (await page.request.get(`${host.baseURL}/api/workflow/runs/${decoy.id}`)).json()).run.userInput).toBe('DECOY MUST NOT OPEN')
    expect(fs.readFileSync(parentPath, 'utf8')).toBe(before)
    expect(fs.readdirSync(runRoot).sort()).toEqual(ids)
    expect(host.herdrCalls()).toEqual(calls)
    await saveEvidence(info, `${condition}-child-facts`, { rootId, childRunId: old.childRunId, decoyId: decoy.id, failure })
  })
}
