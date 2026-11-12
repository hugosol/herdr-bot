import fs from 'node:fs'
import path from 'node:path'
import { test, expect, type Page, type TestInfo } from '@playwright/test'
import type { Request } from '@playwright/test'
import { startServer, type E2EServer } from './support'
import { scriptFileGate } from './scriptFileGate'

// Approved seam: delivered workflow page + public S1 actions/readback; only the
// external Herdr process is controlled. No fabricated execution/report records.
let server: E2EServer | null = null
test.afterEach(async () => { await server?.cleanup(); server = null })
const manager = 'script-page-manager'
async function post(page: Page, route: string, data: object, status = 200) {
  const response = await page.request.post(`${server!.baseURL}${route}`, { data })
  expect(response.status(), await response.text()).toBe(status)
  return response.json()
}
async function read(page: Page, id: string) {
  const response = await page.request.get(`${server!.baseURL}/api/workflow/runs/${id}`)
  expect(response.status()).toBe(200)
  return response.json()
}
async function setup(page: Page) {
  const workers = process.env.TOKIO_WORKER_THREADS
  process.env.TOKIO_WORKER_THREADS = '2'
  try { server = await startServer({ separateCwd: true, herdr: {} }) }
  finally { if (workers === undefined) delete process.env.TOKIO_WORKER_THREADS; else process.env.TOKIO_WORKER_THREADS = workers }
  const zone = path.join(server.cwd, 'bot-playground')
  fs.mkdirSync(zone)
  await post(page, '/api/zones', { id: 'script-zone', path: zone })
  const tasks: Record<string, string> = {}
  for (const taskType of ['manager', 'normal']) {
    const result = await post(page, '/api/tasks', { name: taskType, taskType, agent: 'omp', chatName: '{chat.id}', model: 'default', thinking: 'default', prompt: 'ISOLATED PAGE ACCEPTANCE' })
    tasks[taskType] = result.tasks.find((task: { name: string }) => task.name === taskType).id
  }
  const target = await post(page, '/api/tasks', { name: 'Unbound target', taskType: 'normal', agent: 'omp', chatName: '{chat.id}', model: 'default', thinking: 'default', prompt: 'READ ONLY TARGET' })
  const targetId = target.tasks.find((task: { name: string }) => task.name === 'Unbound target').id
  const request = { schema_version: 1, kind: 'request', target: { type: 'task', task_id: targetId }, bot: { id: 'page-product', name: 'Page product' }, requirements: 'Read only', constraints: [], non_goals: [], resource_limits: [], authorization: { create_blank: true, write_materials: true, save_guides: false }, confirmation: { scope: 'create_blank', evidence: 'Explicit isolated authorization' } }
  fs.writeFileSync(path.join(zone, 'request.json'), JSON.stringify(request))
  await post(page, '/api/scheduler/bots/save', { id: 'script', revision: null, content: JSON.stringify({ name: 'Script', when: '', plan_guide: '', node_guide: '', rust_script: 'create_blank_bot' }) })
  await post(page, '/api/scheduler/bots/save', { id: 'worker', revision: null, content: JSON.stringify({ name: 'Worker', when: '', plan_guide: '', node_guide: '', task_id: tasks.normal }) })
  const map = { title: 'Script workflow', description: '', nodes: {
    create: { title: 'Create blank', bot_id: 'script', input: 'Not script parameters', input_rule: [{ id: 'request', type: 'file', zone_id: 'script-zone', path: 'request.json' }], output_rule: [{ id: 'baseline' }, { id: 'handoff' }], zone: 'script-zone', blocked_by: [] },
    consume: { title: 'Consumer', bot_id: 'worker', input: 'Read result', zone: 'script-zone', blocked_by: ['create'] },
  } }
  const maps = path.join(server.cwd, 'meta-data/workflow-maps')
  fs.mkdirSync(maps, { recursive: true })
  const mapPath = path.join(maps, 'scripts.json')
  fs.writeFileSync(mapPath, JSON.stringify(map))
  expect((await page.request.get(`${server.baseURL}/api/scheduler/maps/scripts.json`)).status()).toBe(200)
  server.setHerdrBehavior({ native_session: { source: 'herdr:omp', agent: 'omp', kind: 'id', value: manager }, native_cwd: zone })
  const run = await post(page, '/api/scheduler/maps/scripts.json/runs', { managerTaskId: tasks.manager, zone: 'script-zone', userInput: 'Only isolated blank creation' }, 201)
  await post(page, '/api/workflow/manager/prepare_inputs', { sessionId: manager, fileInputs: [], confirmed: true, reason: 'Reviewed structured request' })
  return { id: run.id as string, zone, request, map, mapPath }
}
async function evidence(page: Page, info: TestInfo, id: string, name: string, definitions = true) {
  const fact = await read(page, id)
  // During the real Map read gate, its shared definition scope is intentionally
  // occupied. Runtime S1 facts remain independently readable; do not unblock work
  // merely to manufacture a screenshot. Later evidence includes public catalogs.
  const map = definitions ? await (await page.request.get(`${server!.baseURL}/api/scheduler/maps/${fact.run.map}`)).json() : null
  const bots = definitions ? await (await page.request.get(`${server!.baseURL}/api/scheduler/bots`)).json() : null
  const materials = fact.reports.flatMap((report: { executionId: string; outputs: { id: string; path: string }[] }) => report.outputs.map(output => ({ executionId: report.executionId, ...output, content: fs.readFileSync(output.path, 'utf8') })))
  const record = JSON.stringify({ fact, map, bots, materials, calls: server!.herdrCalls(), browser: page.context().browser()?.version() }, null, 2)
  fs.writeFileSync(info.outputPath(`${name}-S1.json`), record)
  await info.attach(`${name}-S1`, { body: record, contentType: 'application/json' })
  const screenshot = await page.screenshot({ path: info.outputPath(`${name}.png`), fullPage: true })
  await info.attach(`${name}-page`, { body: screenshot, contentType: 'image/png' })
  return fact
}

// P31/P5/P32: a plausible violation treats a successful automatic report as
// completed or renders an empty Task/Chat fallback. Literal public states and
// real dependency eligibility distinguish business success from manager handoff.
test('P31 script result remains pending handoff until explicit complete @time-contract', async ({ page }, info) => {
  const s = await setup(page)
  const before = server!.herdrCalls()
  await page.goto(`${server!.baseURL}/#workflow/${s.id}`)
  await expect(page.getByRole('group', { name: 'Map 依赖图', exact: true })).toBeVisible()
  const gate = await scriptFileGate(server!.storageFile)
  const catalogReads: Promise<void>[] = []
  try {
    await post(page, '/api/workflow/manager/dispatch', { sessionId: manager, nodes: ['create'] })
    await expect.poll(gate.reached, { timeout: 5000 }).toBe(true)
    // Two real reads compete with the two async HTTP workers. Blocking file IO
    // on those workers would prevent both the page refresh and independent S1
    // readback while business is still held (the original 90s regression).
    for (let index = 0; index < 2; index++) {
      const pending = page.request.get(`${server!.baseURL}/api/tasks`).then(response => { expect(response.status()).toBe(200) })
      void pending.catch(() => {})
      catalogReads.push(pending)
    }
    await page.getByRole('button', { name: '刷新 Run', exact: true }).click()
    await expect(page.getByRole('region', { name: '当前执行事实', exact: true })).toContainText('业务执行在途')
    const during = await page.request.get(`${server!.baseURL}/api/workflow/runs/${s.id}`, { timeout: 2000 })
    expect(during.status()).toBe(200)
    expect((await during.json()).reports).toEqual([])
    await evidence(page, info, s.id, 'success-in-flight')
    expect(server!.herdrCalls()).toEqual(before)
  } finally { await gate.release(); await Promise.all(catalogReads) }
  await expect.poll(async () => (await read(page, s.id)).reports.length).toBe(1)
  const saved = await read(page, s.id)
  expect(saved.run.nodes.create.status).toBe('running')
  expect(saved.run.nodes.create.execution.lifecycle.status, JSON.stringify(saved)).toBe('succeeded')
  expect(saved.reports[0].canComplete).toBe(true)
  await page.goto(`${server!.baseURL}/#workflow/${s.id}`)
  const execution = page.getByRole('region', { name: '当前执行事实', exact: true })
  await expect(execution).toContainText('rust_script')
  await expect(execution).toContainText('create_blank_bot')
  await expect(execution).toContainText('业务执行已成功结束')
  await expect(execution).not.toContainText('taskId')
  await expect(execution.getByRole('button', { name: /恢复|Resume/ })).toHaveCount(0)
  await expect(page.getByRole('region', { name: '当前执行交接事实', exact: true })).toContainText('最新 canComplete: true')
  const graph = page.getByRole('group', { name: 'Map 依赖图', exact: true })
  await expect(graph.getByRole('button', { name: 'Consumer consume', exact: true })).toHaveAttribute('data-node-state', 'blocked')
  await evidence(page, info, s.id, 'saved-before-complete')
  await post(page, '/api/workflow/manager/complete', { sessionId: manager, nodeId: 'create', reason: 'Accepted actual saved materials' })
  await page.getByRole('button', { name: '刷新 Run', exact: true }).click()
  await expect(graph.getByRole('button', { name: 'Create blank create', exact: true })).toHaveAttribute('data-node-state', 'completed')
  await expect(graph.getByRole('button', { name: 'Consumer consume', exact: true })).toHaveAttribute('data-node-state', 'ready')
  await expect(execution).toContainText('节点已由 manager 完成交接')
  await evidence(page, info, s.id, 'explicit-complete')
  expect(server!.herdrCalls().slice(before.length).every(call => call.kind === 'agent_prompt')).toBe(true)
  for (const output of saved.reports[0].outputs) expect(fs.existsSync(output.path)).toBe(true)
  await expect.poll(() => server!.herdrCalls().some(call => call.kind === 'agent_prompt' && String(call.text).includes(saved.reports[0].id))).toBe(true)
  const stable = await read(page, s.id)
  const calls = server!.herdrCalls()
  const writes: string[] = []
  const observe = (request: Request) => {
    if (request.method() !== 'GET' && /\/api\/(workflow|chats)(\/|$)/.test(new URL(request.url()).pathname)) writes.push(`${request.method()} ${request.url()}`)
  }
  page.on('request', observe)
  try {
    await page.getByRole('button', { name: '刷新 Run', exact: true }).click()
    await page.getByRole('link', { name: '返回全部 Run', exact: true }).click()
    await page.locator(`a[href="#workflow/${s.id}"]`).click()
    await expect(page.getByRole('region', { name: '当前执行事实', exact: true })).toContainText(saved.run.nodes.create.execution.id)
    // Wait for a real production poll, not a hand-issued imitation of polling.
    await page.waitForResponse(response => response.url().endsWith(`/api/workflow/runs/${s.id}`) && response.request().method() === 'GET')
    await page.reload()
    await expect(page.getByRole('tab', { name: 'chat', exact: true })).toHaveAttribute('aria-selected', 'true')
    await page.goto(`${server!.baseURL}/#workflow/${s.id}`)
    await expect(page.getByRole('region', { name: '当前执行事实', exact: true })).toContainText('节点已由 manager 完成交接')
    expect(await read(page, s.id)).toEqual(stable)
    expect(server!.herdrCalls()).toEqual(calls)
    expect(writes).toEqual([])
  } finally { page.off('request', observe) }
  await page.getByText('运行背景 · 本次要求与管理者', { exact: true }).click()
  await expect(page.getByRole('region', { name: '管理者 Chat', exact: true }).getByRole('button', { name: 'Resume', exact: true })).toBeVisible()
  server!.setHerdrBehavior({ native_session: { source: 'herdr:omp', agent: 'omp', kind: 'id', value: 'script-page-consumer' }, native_cwd: s.zone })
  await post(page, '/api/workflow/manager/dispatch', { sessionId: manager, nodes: ['consume'] })
  await page.getByRole('button', { name: '刷新 Run', exact: true }).click()
  await page.getByRole('group', { name: 'Map 依赖图', exact: true }).getByRole('button', { name: 'Consumer consume', exact: true }).click()
  await expect(page.getByRole('region', { name: '当前执行 Chat', exact: true })).toContainText('script-page-consumer')
  await expect(page.getByRole('region', { name: '当前执行 Chat', exact: true }).getByRole('button', { name: 'Resume', exact: true })).toBeVisible()
})

// P31/P5/P32: an old true leaking into a new attempt, current Bot replacing the
// saved script identity, or report.to exposing Resume are plausible violations.
// A real success -> explicit fail/retry -> held business read -> actual partial
// material failure exercises each condition through public actions and the page.
test('P31 retained success never grants retry eligibility or invents a Chat @time-contract', async ({ page }, info) => {
  const s = await setup(page)
  await post(page, '/api/workflow/manager/dispatch', { sessionId: manager, nodes: ['create'] })
  await expect.poll(async () => (await read(page, s.id)).reports.length).toBe(1)
  const first = await read(page, s.id)
  expect(first.reports[0].canComplete).toBe(true)
  await post(page, '/api/workflow/manager/fail', { sessionId: manager, nodeId: 'create', reason: 'Explicit new authorized attempt after inspecting old materials' })
  const afterFail = await read(page, s.id)
  expect(afterFail.run.nodes.create.status).toBe('failed')
  await page.goto(`${server!.baseURL}/#workflow/${s.id}`)
  await expect(page.getByRole('region', { name: '当前执行事实', exact: true })).toContainText(first.run.nodes.create.execution.id)
  await evidence(page, info, s.id, 'explicit-fail')
  const targetMap = path.join(server!.cwd, 'meta-data/workflow-maps/business-target.json')
  fs.writeFileSync(targetMap, JSON.stringify({ title: 'Unbound business target', description: '', nodes: {} }))
  const worker = await post(page, '/api/scheduler/bots/edit', { id: 'worker' })
  const script = await post(page, '/api/scheduler/bots/edit', { id: 'script' })
  fs.writeFileSync(path.join(s.zone, 'request.json'), JSON.stringify({ ...s.request, target: { type: 'map', map_file: 'business-target.json' }, bot: { id: 'page-second-product', name: 'Second product' } }))
  const gate = await scriptFileGate(targetMap)
  let executionId = ''
  try {
    const retry = await post(page, '/api/workflow/manager/retry', { sessionId: manager, nodeId: 'create', confirmed: true, reason: 'User authorized a second distinct blank product' })
    executionId = retry.snapshot.nodes.create.execution.id
    expect(executionId).not.toBe(first.run.nodes.create.execution.id)
    await expect.poll(gate.reached, { timeout: 5000 }).toBe(true)
    const during = await read(page, s.id)
    expect(during.run.nodes.create.execution.lifecycle.status).toBe('running')
    expect(during.reports).toHaveLength(1)
    const collision = path.join(s.zone, '.manager-zone', '.workflow', s.id, executionId, 'handoff.json')
    fs.mkdirSync(path.dirname(collision), { recursive: true })
    fs.writeFileSync(collision, 'UNOWNED HANDOFF')
    // Change the real current definition only; never write a runtime record.
    s.map.nodes.create.input_rule = []
    s.map.nodes.create.output_rule = []
    fs.writeFileSync(s.mapPath, JSON.stringify(s.map))
    fs.writeFileSync(path.join(server!.cwd, 'meta-data/bots', script.revision.file), JSON.stringify({ ...worker.content, id: 'script', name: 'Now a Task target' }))
    await page.getByRole('button', { name: '刷新 Run', exact: true }).click()
    const execution = page.getByRole('region', { name: '当前执行事实', exact: true })
    await expect(execution).toContainText('业务执行在途')
    await expect(execution).toContainText(executionId)
    await expect(execution).toContainText('request.json')
    await expect(execution).toContainText('baseline')
    await expect(page.getByRole('region', { name: '当前执行交接事实', exact: true })).toContainText('最新 canComplete: 无报告')
    await expect(page.getByRole('region', { name: '当前执行交接事实', exact: true })).not.toContainText(first.reports[0].outputs[0].path)
    await expect(page.getByRole('navigation', { name: '已保存节点状态' }).getByRole('button', { name: /Create blank/ })).toContainText('脚本 · create_blank_bot')
    await evidence(page, info, s.id, 'retry-in-flight', false)
  } finally { await gate.release() }
  await expect.poll(async () => (await read(page, s.id)).reports.length).toBe(2)
  const failed = await read(page, s.id)
  expect(failed.run.nodes.create.execution.lifecycle.status).toBe('failed')
  expect(failed.reports[1].canComplete).toBe(false)
  expect(failed.reports[1].outputs.map((output: { id: string }) => output.id)).toEqual(['baseline'])
  expect(failed.run.nodes.create.status).toBe('running')
  await post(page, '/api/workflow/manager/complete', { sessionId: manager, nodeId: 'create', reason: 'False report cannot complete' }, 409)
  await page.getByRole('button', { name: '刷新 Run', exact: true }).click()
  const execution = page.getByRole('region', { name: '当前执行事实', exact: true })
  await expect(execution).toContainText('业务执行已失败结束')
  await expect(execution).toContainText('material-write-failed')
  await expect(execution).toContainText('succeeded')
  const reports = page.getByRole('region', { name: '当前执行适用报告', exact: true })
  await expect(reports).toContainText('create_blank_bot')
  await expect(reports.getByRole('button', { name: 'Resume', exact: true })).toHaveCount(0)
  const history = page.getByRole('group', { name: '已保存旧执行事实', exact: true })
  await history.getByText(/旧执行历史/, { exact: false }).click()
  await expect(history).toContainText(first.run.nodes.create.execution.id)
  await expect(history).toContainText(first.reports[0].outputs[0].path)
  await expect(history).toContainText('业务执行已成功结束')
  await expect(history.getByRole('button', { name: 'Resume', exact: true })).toHaveCount(0)
  await evidence(page, info, s.id, 'failed-partial-and-old-success')
})
