import fs from 'node:fs'
import path from 'node:path'
import { test, expect } from '@playwright/test'
import { configureScheduler, startServer, type E2EServer } from './support'
import { expectNoHorizontalOverflow } from './ui'
let server: E2EServer | null = null
test.afterEach(async () => { await server?.cleanup(); server = null })
// S1 actual delivered page, S2 HTTP, S3 controlled external recorder, S4 files.
// P12/P68 violation: refresh replays POST/start. Observe 750ms after each
// refresh/reentry and retain external calls; no infinite absence claim.
async function fixture() {
  server = await startServer({ separateCwd: true, herdr: { behavior: { native_session: { source: 'herdr:omp', agent: 'omp', kind: 'id', value: 'browser-manager-04' } } } })
  const zonePath = path.join(path.dirname(server.configPath), 'bot-playground')
  fs.mkdirSync(zonePath)
  let response = await fetch(`${server.baseURL}/api/zones`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ path: zonePath }) })
  expect(response.status).toBe(200)
  const zone = (await response.json()).zones[0].path
  const ids: Record<string, string> = {}
  for (const role of ['manager', 'normal']) {
    response = await fetch(`${server.baseURL}/api/tasks`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ name: role, taskType: role, agent: 'omp', chatName: '{chat.id}', model: 'default', thinking: 'default', skill: null, prompt: 'FIXED ROLE', extensions: [] }) })
    expect(response.status).toBe(200)
    ids[role] = (await response.json()).tasks.find((task: { name: string }) => task.name === role).id
  }
  const maps = path.join(server.cwd, 'meta-data/workflow-maps'), bots = path.join(server.cwd, 'meta-data/bots')
  fs.mkdirSync(maps, { recursive: true }); fs.mkdirSync(bots, { recursive: true })
  fs.writeFileSync(path.join(maps, 'launch.json'), JSON.stringify({ title: '启动图', description: '', nodes: { A: { title: 'A', bot_id: 'worker', input: 'NODE ONLY', zone, blocked_by: [] } } }))
  fs.writeFileSync(path.join(bots, 'worker.json'), JSON.stringify({ id: 'worker', name: '', when: '', plan_guide: '', node_guide: '', task_id: ids.normal }))
  return { server, zone, ids }
}

// Approved seams: delivered page + HTTP, process restart, external Herdr recorder
// and the three real record files. P28.04/P49.01/P68.08 source: ticket 16.
// Plausible violation: startup scans saved identities and re-enables management
// or lets a pending report bypass the process-local ownership restriction.
// Keep report/collect and every mutation real; only the external host is controlled.
test('P28.04 P49.01 P68.08 real restart preserves facts but rejects old Run tools @time-contract', async ({ page }, info) => {
  const { server, zone, ids } = await fixture()
  const mapPath = path.join(server.cwd, 'meta-data/workflow-maps/launch.json')
  const map = JSON.parse(fs.readFileSync(mapPath, 'utf8'))
  map.nodes.B = { ...map.nodes.A, title: 'B' }
  map.nodes.C = { ...map.nodes.A, title: 'C' }
  fs.writeFileSync(mapPath, JSON.stringify(map))
  const post = (route: string, data: object) => page.request.post(`${server.baseURL}${route}`, { data })
  const started = await post('/api/scheduler/maps/launch.json/runs', { managerTaskId: ids.manager, zone, userInput: '保存原始现场' })
  expect(started.status()).toBe(201)
  const { id } = await started.json()
  for (const nodeId of ['A', 'B']) {
    server.setHerdrBehavior({ native_session: { source: 'herdr:omp', agent: 'omp', kind: 'id', value: `restart-node-${nodeId}` } })
    const response = await post('/api/workflow/manager/dispatch', { sessionId: 'browser-manager-04', nodes: [nodeId] })
    expect(response.status()).toBe(200)
    expect((await response.json()).results[0].status).toBe('running')
  }
  const reported = await post('/api/workflow/node/report', { sessionId: 'restart-node-A', body: '重启前报告：结果地址与限制仍需管理者核实' })
  expect(reported.status()).toBe(200)
  const failed = await post('/api/workflow/manager/fail', { sessionId: 'browser-manager-04', nodeId: 'B', reason: '已核实的节点执行故障' })
  expect(failed.status()).toBe(200)
  const before = await (await page.request.get(`${server.baseURL}/api/workflow/runs/${id}`)).json()
  expect(before.managed).toBe(true)
  expect(before.run.status).toBe('running')
  expect(before.run.nodes.A.status).toBe('running')
  expect(before.run.nodes.B.status).toBe('failed')
  expect(before.run.nodes.B.failure.stage).toBe('execution')
  expect(before.pendingReports.A).toBeDefined()
  const root = path.join(server.cwd, '.data/workflow-runs', id)
  const records = () => Object.fromEntries(['run.json', 'history.jsonl', 'messages.jsonl'].map(file => [file, fs.readFileSync(path.join(root, file), 'utf8')]))
  const saved = records(), calls = server.herdrCalls()
  await server.restart()
  const restarted = await (await page.request.get(`${server.baseURL}/api/workflow/runs/${id}`)).json()
  expect(restarted).toEqual({ ...before, managed: false })
  await page.goto(`${server.baseURL}/#workflow/${id}`)
  await page.locator('.workflow-run-background > summary').click()
  await expect(page.getByText('browser-manager-04', { exact: true })).toBeVisible()
  const navigation = page.getByRole('navigation', { name: '已保存节点状态' })
  await expect(navigation.locator('[data-node-id="A"]')).toHaveAttribute('data-node-state', 'running')
  await expect(navigation.locator('[data-node-id="B"]')).toHaveAttribute('data-node-state', 'failed')
  const outcomes = []
  for (const [action, fields] of [
    ['status', {}], ['collect', {}], ['dispatch', { nodes: ['C'] }],
    ['complete', { nodeId: 'A', reason: '确认可交接' }], ['finish', {}],
    ['pause', {}], ['resume', {}], ['fail', { nodeId: 'A', reason: '核实失败' }],
    ['recover', { nodeId: 'B', confirmed: true, reason: '已修复' }],
    ['retry', { nodeId: 'B', confirmed: true, reason: '人工确认另起执行' }],
    ['reconcile_dispatch', { nodeId: 'B', finding: 'created', reason: '人工核实' }],
  ] as const) {
    const response = await post(`/api/workflow/manager/${action}`, { sessionId: 'browser-manager-04', ...fields })
    const body = await response.json()
    outcomes.push({ action, status: response.status(), body })
    expect(response.status(), action).toBe(403)
    expect(body.code, action).toBe('manager-unassociated')
  }
  for (const sessionId of ['restart-node-A', 'restart-node-B']) {
    const response = await post('/api/workflow/node/report', { sessionId, body: '不能在重启后追加' })
    const body = await response.json()
    outcomes.push({ action: 'report', sessionId, status: response.status(), body })
    expect(response.status()).toBe(403)
    expect(body.code).toBe('node-unassociated')
  }
  const runURLBeforeReload1 = page.url()

  await page.reload()

  await expect(page.getByRole('tab', { name: 'chat', exact: true })).toHaveAttribute('aria-selected', 'true')

  await page.goto(runURLBeforeReload1)
  await expect(page.getByRole('heading', { name: 'Run 详情' })).toBeVisible()
  // Bounded absence evidence: 1000ms after all rejected tools and page reload.
  await page.waitForTimeout(1000)
  expect(server.herdrCalls()).toEqual(calls)
  expect(records()).toEqual(saved)
  expect(await (await page.request.get(`${server.baseURL}/api/workflow/runs/${id}`)).json()).toEqual(restarted)
  fs.writeFileSync(info.outputPath('restart-read-only-evidence.json'), JSON.stringify({ before, restarted, recordsBefore: saved, recordsAfter: records(), outcomes, callsBefore: calls, callsAfter: server.herdrCalls(), observationMs: 1000 }, null, 2))
  await page.screenshot({ path: info.outputPath('restart-read-only.png') })
})

// P54.02 source: ticket 16. Violation: reconstructing process-local startup
// errors from persisted business history on restart. Observe the actual page
// and byte-identical records; diagnostics are captured separately, not replayed.
// The preceding restart test independently proves persistent node failure.
test('P54.02 real restart drops manager startup errors without rewriting business facts @time-contract', async ({ page }, info) => {
  const { server, zone, ids } = await fixture()
  server.setHerdrBehavior({ native_session: { source: 'herdr:omp', agent: 'omp', kind: 'id', value: 'failed-manager-restart' }, fail_prompt: true })
  const response = await page.request.post(`${server.baseURL}/api/scheduler/maps/launch.json/runs`, { data: { managerTaskId: ids.manager, zone, userInput: '不应出现在诊断里的本次要求' } })
  expect(response.status()).toBe(500)
  const before = await response.json()
  expect(before.startupError.stage).toBe('submit')
  expect(before.run.manager.sessionId).toBe('failed-manager-restart')
  await page.goto(`${server.baseURL}/#workflow/${before.id}`)
  await expect(page.getByRole('alert', { name: '管理者启动错误' })).toBeVisible()
  const diagnostics = server.diagnostics()
  expect(diagnostics).toContain(`run=${before.id} manager-start stage=submit`)
  expect(diagnostics).not.toContain('不应出现在诊断里的本次要求')
  // Preserve process diagnostics outside the three business files before exit.
  fs.writeFileSync(info.outputPath('pre-restart-diagnostics.log'), diagnostics)
  const root = path.join(server.cwd, '.data/workflow-runs', before.id)
  const saved = Object.fromEntries(['run.json', 'history.jsonl', 'messages.jsonl'].map(file => [file, fs.readFileSync(path.join(root, file), 'utf8')]))
  expect(saved['history.jsonl']).toBe('')
  expect(saved['messages.jsonl']).toBe('')
  expect(JSON.parse(saved['run.json'])).not.toHaveProperty('startupError')
  const calls = server.herdrCalls()
  await server.restart()
  const after = await (await page.request.get(`${server.baseURL}/api/workflow/runs/${before.id}`)).json()
  expect(after).toEqual({ ...before, managed: false, startupError: null })
  // The helper reuses its allocated port; a hash-only goto is not a refresh.
  const runURLBeforeReload2 = page.url()

  await page.reload()

  await expect(page.getByRole('tab', { name: 'chat', exact: true })).toHaveAttribute('aria-selected', 'true')

  await page.goto(runURLBeforeReload2)
  await expect(page.getByRole('heading', { name: 'Run 详情' })).toBeVisible()
  await page.locator('.workflow-run-background > summary').click()
  await expect(page.getByText('failed-manager-restart', { exact: true })).toBeVisible()
  await expect(page.getByText(/^历史现场 \/ 未接管：/)).toBeVisible()
  await expect(page.getByRole('alert', { name: '管理者启动错误' })).toHaveCount(0)
  const runURLBeforeReload3 = page.url()

  await page.reload()

  await expect(page.getByRole('tab', { name: 'chat', exact: true })).toHaveAttribute('aria-selected', 'true')

  await page.goto(runURLBeforeReload3)
  await page.locator('.workflow-run-background > summary').click()
  await expect(page.getByText('failed-manager-restart', { exact: true })).toBeVisible()
  await page.waitForTimeout(1000)
  await expect(page.getByRole('alert', { name: '管理者启动错误' })).toHaveCount(0)
  expect(server.diagnostics()).not.toContain('manager-start')
  expect(server.herdrCalls()).toEqual(calls)
  const recordsAfter = Object.fromEntries(['run.json', 'history.jsonl', 'messages.jsonl'].map(file => [file, fs.readFileSync(path.join(root, file), 'utf8')]))
  expect(recordsAfter).toEqual(saved)
  expect(fs.readFileSync(info.outputPath('pre-restart-diagnostics.log'), 'utf8')).toBe(diagnostics)
  fs.writeFileSync(info.outputPath('restart-manager-error-evidence.json'), JSON.stringify({ before, after, recordsBefore: saved, recordsAfter, callsBefore: calls, callsAfter: server.herdrCalls(), observationMs: 1000 }, null, 2))
  await page.screenshot({ path: info.outputPath('restart-no-manager-error.png') })
})

test('P12 invalid start stays on the launch form without external effects', async ({ page }, info) => {
  const { server } = await fixture()
  await page.goto(`${server.baseURL}?layout=expanded`)
  await page.getByRole('tab', { name: 'scheduler', exact: true }).click()
  await page.getByRole('button', { name: '启动图', exact: true }).click()
  await page.getByRole('button', { name: '+ 发起 Run', exact: true }).click()
  await page.getByRole('button', { name: '启动 Run', exact: true }).click()
  await expect(page.getByRole('form', { name: '发起 Run' })).toContainText('启动预检未通过')
  expect(page.url()).not.toContain('#workflow/')
  expect(server.herdrCalls()).toEqual([])
  expect(fs.existsSync(path.join(server.cwd, '.data/workflow-runs'))).toBe(false)
  fs.writeFileSync(info.outputPath('rejected-start-external-calls.json'), JSON.stringify(server.herdrCalls()))
})

// P45/P54 violation: showing startup success, hiding the real Chat, or losing
// process-local error on page refresh. Controlled failure is not native-host proof.
test('P45 P54 startup failure and real partial effects survive browser refresh @time-contract', async ({ page }, info) => {
  const { server, zone, ids } = await fixture()
  server.setHerdrBehavior({ native_session: { source: 'herdr:omp', agent: 'omp', kind: 'id', value: 'failed-manager-04' }, fail_prompt: true })
  const response = await page.request.post(`${server.baseURL}/api/scheduler/maps/launch.json/runs`, { data: { managerTaskId: ids.manager, zone, userInput: '' } })
  expect(response.status()).toBe(500)
  const detail = await response.json()
  expect(detail.startupError.stage).toBe('submit')
  await page.goto(`${server.baseURL}/#workflow/${detail.id}`)
  await expect(page.getByRole('alert', { name: '管理者启动错误' })).toContainText('已发生的外部效果未回滚')
  await expect(page.getByText('failed-manager-04', { exact: true })).toHaveCount(2)
  const calls = server.herdrCalls()
  const runURLBeforeReload4 = page.url()

  await page.reload()

  await expect(page.getByRole('tab', { name: 'chat', exact: true })).toHaveAttribute('aria-selected', 'true')

  await page.goto(runURLBeforeReload4)
  await expect(page.getByRole('heading', { name: '管理者启动失败 · submit' })).toBeVisible()
  await page.waitForTimeout(750)
  expect(server.herdrCalls()).toEqual(calls)
  expect(server.diagnostics()).toContain(`run=${detail.id}`)
  expect(server.diagnostics()).toContain('stage=submit')
  fs.writeFileSync(info.outputPath('failure-effects-and-calls.json'), JSON.stringify({ detail, calls, diagnostics: server.diagnostics() }, null, 2))
  await page.screenshot({ path: info.outputPath('startup-error-refresh.png') })
})

test('P9 P10 P12 P67 P68 manager Run opens refreshable read-only details @time-contract', async ({ page }, info) => {
  const { server, zone, ids } = await fixture()
  const runRoot = path.join(path.dirname(server.configPath), 'nondefault-run-records')
  configureScheduler(server, { run_directory: runRoot })
  await server.restart()
  await page.goto(`${server.baseURL}?layout=expanded`)
  await page.getByRole('tab', { name: 'scheduler', exact: true }).click()
  await page.getByRole('button', { name: '启动图', exact: true }).click()
  await page.getByRole('button', { name: '+ 发起 Run', exact: true }).click()
  await page.getByRole('combobox', { name: '管理者 Task' }).selectOption(ids.manager)
  await page.getByRole('combobox', { name: '管理者 zone' }).selectOption(zone)
  const requirements = '# 浏览器原文\n\n- 不可改写  空格\n末尾\n'
  await page.getByRole('textbox', { name: '本次要求' }).fill(requirements)
  await page.getByRole('button', { name: '启动 Run', exact: true }).click()
  await expect(page.getByRole('heading', { name: 'Run 详情' })).toBeVisible()
  const url = page.url()
  expect(url).toContain('#workflow/')
  const runId = url.split('#workflow/')[1]
  expect(fs.existsSync(path.join(runRoot, runId, 'run.json'))).toBe(true)
  expect(fs.existsSync(path.join(server.cwd, '.data/workflow-runs'))).toBe(false)
  expect(server.herdrCalls().find(call => call.kind === 'agent_prompt')?.text).toContain(path.join(runRoot, runId, 'run.json'))
  await expect(page.getByTestId('run-user-input')).toHaveText(requirements)
  await expect(page.getByRole('textbox', { name: '本次要求' })).toHaveCount(0)
  await page.locator('.workflow-run-background > summary').click()
  await expect(page.getByText('browser-manager-04', { exact: true })).toBeVisible()
  const count = server.herdrCalls().filter(call => call.kind === 'agent_start').length
  expect(count).toBe(1)
  const runURLBeforeReload5 = page.url()

  await page.reload()

  await expect(page.getByRole('tab', { name: 'chat', exact: true })).toHaveAttribute('aria-selected', 'true')

  await page.goto(runURLBeforeReload5)
  await expect(page.getByRole('heading', { name: 'Run 详情' })).toBeVisible()
  await page.waitForTimeout(750)
  await page.getByRole('tab', { name: 'chat', exact: true }).click()
  await page.getByRole('tab', { name: 'workflow', exact: true }).click()
  await expect(page.getByRole('heading', { name: 'Run 详情' })).toBeVisible()
  await page.waitForTimeout(750)
  expect(server.herdrCalls().filter(call => call.kind === 'agent_start')).toHaveLength(count)
  fs.writeFileSync(info.outputPath('external-calls.json'), JSON.stringify(server.herdrCalls(), null, 2))
  fs.writeFileSync(info.outputPath('run-records.json'), JSON.stringify({
    root: runRoot, serviceCwd: server.cwd,
    run: JSON.parse(fs.readFileSync(path.join(runRoot, runId, 'run.json'), 'utf8')),
    history: fs.readFileSync(path.join(runRoot, runId, 'history.jsonl'), 'utf8'),
    messages: fs.readFileSync(path.join(runRoot, runId, 'messages.jsonl'), 'utf8'),
  }, null, 2))
  await page.screenshot({ path: info.outputPath('run-details.png') })
  await page.setViewportSize({ width: 390, height: 844 })
  await expectNoHorizontalOverflow(page)
  await page.screenshot({ path: info.outputPath('run-details-mobile.png') })
  await page.getByRole('link', { name: '返回全部 Run' }).click()
  await expect(page.getByRole('searchbox')).toBeVisible()
  await expect(page.getByRole('heading', { name: 'Run 详情' })).toHaveCount(0)
  await expect(page).toHaveURL(/#workflow$/)
})
