import fs from 'node:fs'
import path from 'node:path'
import { expect, test, type Page } from '@playwright/test'
import { startServer, type E2EServer } from './support'

// Approved seams: S1 delivered page, S2 scheduler/workflow/Chat HTTP and real
// process restart, S3 production Herdr CLI with a test-owned command recorder.
// No application collaborators are replaced. A recorded close proves an attempt,
// not real Herdr Tab removal or terminal termination (that belongs to P22).
let server: E2EServer | null = null
test.afterEach(async () => { await server?.cleanup(); server = null })
const manager = 'lifecycle-manager'
const observationMs = 300

async function post(page: Page, route: string, data: object, status = 200) {
  const response = await page.request.post(`${server!.baseURL}${route}`, { data })
  expect(response.status(), await response.text()).toBe(status)
  return response.json()
}
async function readRun(page: Page, id: string) {
  const response = await page.request.get(`${server!.baseURL}/api/workflow/runs/${id}`)
  expect(response.status()).toBe(200)
  return response.json()
}
function host(sessionId: string, cwd: string) {
  server!.setHerdrBehavior({ native_session: { source: 'herdr:omp', agent: 'omp', kind: 'id', value: sessionId }, native_cwd: cwd })
}
function closes() { return server!.herdrCalls().filter(call => call.kind === 'tab_close') }
async function noMoreCloses(page: Page, expected: unknown[]) {
  // This is deliberately a bounded negative observation, not an assertion about
  // all future time. Check the command record throughout and at the deadline.
  const end = Date.now() + observationMs
  do {
    expect(closes().map(call => call.argv)).toEqual(expected)
    await page.waitForTimeout(20)
  } while (Date.now() < end)
  expect(closes().map(call => call.argv)).toEqual(expected)
}
async function setChoice(page: Page, nodeId: string, enabled: boolean) {
  const draft = await post(page, '/api/scheduler/maps/lifecycle.json/nodes/edit', { id: nodeId })
  await post(page, '/api/scheduler/maps/lifecycle.json/nodes/save', { id: nodeId, content: JSON.stringify({ ...draft.content, auto_close: enabled }), revision: draft.revision, create: false })
  const response = await page.request.get(`${server!.baseURL}/api/scheduler/maps/lifecycle.json`)
  expect(response.status()).toBe(200)
  expect((await response.json()).nodes[nodeId].auto_close).toBe(enabled)
}
async function setup(page: Page, a = false, b = true) {
  server = await startServer({ separateCwd: true, herdr: {} })
  const zone = path.join(server.cwd, 'bot-playground')
  fs.mkdirSync(zone)
  await post(page, '/api/zones', { path: zone })
  const ids: Record<string, string> = {}
  for (const role of ['manager', 'normal']) {
    const result = await post(page, '/api/tasks', { name: role, taskType: role, agent: 'omp', chatName: '{chat.id}', model: 'default', thinking: 'default', skill: null, prompt: 'LAUNCH-ONLY-LIFECYCLE', extensions: [] })
    ids[role] = result.tasks.find((task: { name: string }) => task.name === role).id
  }
  const botRoot = path.join(server.cwd, 'meta-data/bots')
  fs.mkdirSync(botRoot, { recursive: true })
  fs.writeFileSync(path.join(botRoot, 'worker.json'), JSON.stringify({ id: 'worker', name: 'worker', when: '', plan_guide: '', node_guide: '', task_id: ids.normal }))
  const mapRoot = path.join(server.cwd, 'meta-data/workflow-maps')
  fs.mkdirSync(mapRoot, { recursive: true })
  const node = { title: 'Worker', bot_id: 'worker', input: 'NODE-LIFECYCLE-INPUT', zone, blocked_by: [] }
  fs.writeFileSync(path.join(mapRoot, 'lifecycle.json'), JSON.stringify({ title: 'Lifecycle', description: 'Isolated auto-close lifecycle', nodes: { A: { ...node, auto_close: a }, B: { ...node, auto_close: b } } }))
  host(manager, zone)
  const started = await post(page, '/api/scheduler/maps/lifecycle.json/runs', { managerTaskId: ids.manager, zone, userInput: 'LIFECYCLE-MANAGER-INPUT' }, 201)
  return { zone, runId: started.id as string }
}
async function dispatch(page: Page, nodeId: string, sessionId: string, zone: string) {
  host(sessionId, zone)
  const result = await post(page, '/api/workflow/manager/dispatch', { sessionId: manager, nodes: [nodeId] })
  expect(result.results[0].status).toBe('running')
  return result.snapshot.nodes[nodeId].execution
}
async function complete(page: Page, nodeId: string) {
  const result = await post(page, '/api/workflow/manager/complete', { sessionId: manager, nodeId, reason: 'Manager accepted the delivered work' })
  expect(result.changed).toBe(true)
  expect(result.snapshot.nodes[nodeId].status).toBe('completed')
  return result.snapshot
}
async function expectClosed(tabId: string, count: number) {
  await expect.poll(() => closes().length).toBe(count)
  expect(closes()[count - 1].argv).toEqual(['tab', 'close', tabId])
}

// P8: Run creation must not freeze node choices. The independent expected
// values are true/false after opposite Map edits, not values read from execution.
// A stale Run snapshot would either miss A's close or incorrectly close B.
test('P8 new dispatch uses current Map choices in both directions @time-contract', async ({ page }) => {
  const { zone } = await setup(page)
  await setChoice(page, 'A', true)
  await setChoice(page, 'B', false)
  const a = await dispatch(page, 'A', 'lifecycle-new-A', zone)
  const b = await dispatch(page, 'B', 'lifecycle-new-B', zone)
  expect(a.autoClose).toBe(true)
  expect(b.autoClose).toBe(false)
  expect(a.id).not.toBe(b.id)
  expect(a.chat.sessionId).toBe('lifecycle-new-A')
  expect(b.chat.sessionId).toBe('lifecycle-new-B')
  await complete(page, 'A')
  await expectClosed(a.chat.tabId, 1)
  await complete(page, 'B')
  await noMoreCloses(page, [['tab', 'close', a.chat.tabId]])
})

// P18: after a real eligible completion, GET, delivered-page reload and an
// isolated service-process restart must not repeat the external close. A read
// hook or startup replay would add a command; byte checks also reject rewriting
// the saved completion into a new execution/completion or a close-result field.
test('P18 completed execution is not closed again by reading refreshing or service restart @time-contract', async ({ page }, info) => {
  const { zone, runId } = await setup(page, true)
  const execution = await dispatch(page, 'A', 'lifecycle-read-completed', zone)
  const completed = await complete(page, 'A')
  await expectClosed(execution.chat.tabId, 1)
  const expected = [['tab', 'close', execution.chat.tabId]]
  const root = path.join(server!.cwd, '.data/workflow-runs', runId)
  const files = ['run.json', 'history.jsonl', 'messages.jsonl']
  const saved = files.map(file => fs.readFileSync(path.join(root, file), 'utf8'))
  const detail = await readRun(page, runId)
  expect(detail.run).toEqual(completed)
  for (const route of ['/api/workflow/runs', `/api/workflow/runs/${runId}`, '/api/sessions/history']) {
    expect((await page.request.get(`${server!.baseURL}${route}`)).status()).toBe(200)
  }
  await noMoreCloses(page, expected)
  await page.goto(`${server!.baseURL}/#workflow/${runId}`)
  await expect(page.getByRole('navigation', { name: '已保存节点状态' }).locator('[data-node-id="A"]')).toHaveAttribute('data-node-state', 'completed')
  await page.reload()
  // A full reload resets to Chat by the existing UI contract. Reopen the Run
  // after the document load; a hash-only navigation is not the refresh evidence.
  await expect(page.getByRole('tab', { name: 'chat', exact: true })).toHaveAttribute('aria-selected', 'true')
  await page.goto(`${server!.baseURL}/#workflow/${runId}`)
  await expect(page.getByRole('navigation', { name: '已保存节点状态' }).locator('[data-node-id="A"]')).toHaveAttribute('data-node-state', 'completed')
  await noMoreCloses(page, expected)
  await server!.restart()
  const restarted = await readRun(page, runId)
  expect(restarted).toEqual({ ...detail, managed: false })
  await page.reload()
  await page.goto(`${server!.baseURL}/#workflow/${runId}`)
  await expect(page.getByRole('navigation', { name: '已保存节点状态' }).locator('[data-node-id="A"]')).toHaveAttribute('data-node-state', 'completed')
  await noMoreCloses(page, expected)
  expect(files.map(file => fs.readFileSync(path.join(root, file), 'utf8'))).toEqual(saved)
  fs.writeFileSync(info.outputPath('read-refresh-restart.json'), JSON.stringify({ observationMs, completed, restarted, calls: server!.herdrCalls(), records: saved }, null, 2))
})

// P18: hold the real close CLI before it returns a result, then terminate the
// owned service tree. The already-persisted completed execution has no close
// outcome. Startup compensation would issue a second command. No persisted
// result flag is introduced to manufacture or detect this condition.
test('P18 loading completed facts without a close result never compensates after restart @time-contract', async ({ page }, info) => {
  const { zone, runId } = await setup(page, true)
  const execution = await dispatch(page, 'A', 'lifecycle-interrupted-close', zone)
  const gate = path.join(path.dirname(server!.configPath), 'release-interrupted-close')
  server!.setHerdrBehavior({ tab_close_gate: { wait_for_file: gate, timeout_ms: 30000 } })
  const completed = await complete(page, 'A')
  await expectClosed(execution.chat.tabId, 1)
  const resultFile = path.join(path.dirname(server!.configPath), 'herdr/closed.jsonl')
  expect(fs.existsSync(resultFile)).toBe(false)
  await server!.stop()
  expect(fs.existsSync(resultFile)).toBe(false)
  const root = path.join(server!.cwd, '.data/workflow-runs', runId)
  const files = ['run.json', 'history.jsonl', 'messages.jsonl']
  const saved = files.map(file => fs.readFileSync(path.join(root, file), 'utf8'))
  const fact = JSON.parse(saved[0])
  expect(fact).toEqual(completed)
  expect(fact.nodes.A.status).toBe('completed')
  expect(fact.nodes.A.execution.autoClose).toBe(true)
  expect(Object.keys(fact.nodes.A.execution).sort()).toEqual(['agent', 'autoClose', 'chat', 'id', 'reportPolicy', 'taskId', 'zone'])
  server!.setHerdrBehavior({})
  await server!.restart()
  const loaded = await readRun(page, runId)
  expect(loaded.run).toEqual(completed)
  expect(loaded.managed).toBe(false)
  await page.goto(`${server!.baseURL}/#workflow/${runId}`)
  await expect(page.getByRole('navigation', { name: '已保存节点状态' }).locator('[data-node-id="A"]')).toHaveAttribute('data-node-state', 'completed')
  await page.reload()
  await page.goto(`${server!.baseURL}/#workflow/${runId}`)
  await expect(page.getByRole('navigation', { name: '已保存节点状态' }).locator('[data-node-id="A"]')).toHaveAttribute('data-node-state', 'completed')
  await noMoreCloses(page, [['tab', 'close', execution.chat.tabId]])
  expect(fs.existsSync(resultFile)).toBe(false)
  expect(files.map(file => fs.readFileSync(path.join(root, file), 'utf8'))).toEqual(saved)
  fs.writeFileSync(info.outputPath('interrupted-close-restart.json'), JSON.stringify({ observationMs, loaded, calls: server!.herdrCalls(), resultExists: fs.existsSync(resultFile), records: saved }, null, 2))
})

// P19: a completed Chat may deliberately open a new Tab through Resume. The
// immutable completion/execution must not become a new completion trigger. A
// replay on restore, or a refresh-time cleanup targeting the newly opened Tab,
// would add a close command and/or change the saved facts within this window.
test('P19 completed Chat restoration opens a new Tab without recreating completion or closing it @time-contract', async ({ page }, info) => {
  const { zone, runId } = await setup(page, true)
  const execution = await dispatch(page, 'A', 'lifecycle-resume-completed', zone)
  const completed = await complete(page, 'A')
  await expectClosed(execution.chat.tabId, 1)
  const root = path.join(server!.cwd, '.data/workflow-runs', runId)
  const files = ['run.json', 'history.jsonl', 'messages.jsonl']
  const saved = files.map(file => fs.readFileSync(path.join(root, file), 'utf8'))
  host('lifecycle-resume-completed', zone)
  const offset = server!.herdrCalls().length
  const resumed = await post(page, '/api/chats/resume', { sessionId: 'lifecycle-resume-completed' })
  expect(resumed.outcome).toBe('restored')
  expect(resumed.target.tabId).not.toBe(execution.chat.tabId)
  const calls = server!.herdrCalls().slice(offset)
  expect(calls.filter(call => call.kind === 'tab_create')).toHaveLength(1)
  const starts = calls.filter(call => call.kind === 'agent_start')
  expect(starts).toHaveLength(1)
  expect(starts[0].args).toEqual(expect.arrayContaining(['--resume', 'lifecycle-resume-completed']))
  expect((starts[0].args as string[]).some(arg => path.basename(arg) === 'omp-node-chat.ts')).toBe(true)
  expect(calls.filter(call => ['agent_prompt', 'agent_input'].includes(String(call.kind)))).toEqual([])
  const after = await readRun(page, runId)
  expect(after.run).toEqual(completed)
  expect(after.run.nodes.A.execution).toEqual(execution)
  expect(after.run.nodes.A.completion).toEqual(completed.nodes.A.completion)
  expect(after.run.nodes.A.previousExecutions).toEqual([])
  await page.goto(`${server!.baseURL}/#workflow/${runId}`)
  await expect(page.getByRole('navigation', { name: '已保存节点状态' }).locator('[data-node-id="A"]')).toHaveAttribute('data-node-state', 'completed')
  await page.reload()
  await noMoreCloses(page, [['tab', 'close', execution.chat.tabId]])
  expect(closes().some(call => call.target === resumed.target.tabId)).toBe(false)
  expect(files.map(file => fs.readFileSync(path.join(root, file), 'utf8'))).toEqual(saved)
  fs.writeFileSync(info.outputPath('completed-chat-resume.json'), JSON.stringify({ observationMs, completed, resumed, after, calls: server!.herdrCalls(), records: saved }, null, 2))
})

for (const enabled of [true, false]) {
  // P19: restoring the same native Chat is not dispatch/retry. An implementation
  // that rereads the changed Map, replays launch input, uses the normal Task's
  // standalone role or replaces execution identity must fail these observations.
  test(`P19 unfinished Chat restoration keeps original ${enabled} choice against the changed Map @time-contract`, async ({ page }, info) => {
    const { zone, runId } = await setup(page, enabled)
    const execution = await dispatch(page, 'A', 'lifecycle-resume-unfinished', zone)
    expect(execution.autoClose).toBe(enabled)
    await setChoice(page, 'A', !enabled)
    const before = await readRun(page, runId)
    const root = path.join(server!.cwd, '.data/workflow-runs', runId)
    const files = ['run.json', 'history.jsonl', 'messages.jsonl']
    const saved = files.map(file => fs.readFileSync(path.join(root, file), 'utf8'))
    host('lifecycle-resume-unfinished', zone)
    const offset = server!.herdrCalls().length
    const resumed = await post(page, '/api/chats/resume', { sessionId: 'lifecycle-resume-unfinished' })
    expect(resumed.outcome).toBe('restored')
    expect(resumed.target.tabId).not.toBe(execution.chat.tabId)
    const calls = server!.herdrCalls().slice(offset)
    const starts = calls.filter(call => call.kind === 'agent_start')
    expect(starts).toHaveLength(1)
    expect(starts[0].args).toEqual(expect.arrayContaining(['--resume', 'lifecycle-resume-unfinished']))
    expect((starts[0].args as string[]).some(arg => path.basename(arg) === 'omp-node-chat.ts')).toBe(true)
    expect((starts[0].args as string[]).some(arg => path.basename(arg) === 'omp-manager-chat.ts')).toBe(false)
    expect(calls.filter(call => ['agent_prompt', 'agent_input', 'tab_close'].includes(String(call.kind)))).toEqual([])
    expect(await readRun(page, runId)).toEqual(before)
    expect(files.map(file => fs.readFileSync(path.join(root, file), 'utf8'))).toEqual(saved)
    const completed = await complete(page, 'A')
    expect(completed.nodes.A.execution).toEqual(execution)
    expect(completed.nodes.A.previousExecutions).toEqual([])
    if (enabled) await expectClosed(execution.chat.tabId, 1)
    await noMoreCloses(page, enabled ? [['tab', 'close', execution.chat.tabId]] : [])
    fs.writeFileSync(info.outputPath(`unfinished-resume-${enabled}.json`), JSON.stringify({ observationMs, execution, resumed, completed, calls: server!.herdrCalls() }, null, 2))
  })
}

for (const enabled of [true, false]) {
  // P8: independent retry is a new execution, not Chat restoration. Reusing the
  // old execution's choice/Tab is a plausible violation; retained old identity
  // and exact external target distinguish it in both configuration directions.
  test(`P8 independent retry adopts ${enabled} after the opposite original choice @time-contract`, async ({ page }) => {
    const { zone, runId } = await setup(page, !enabled)
    const old = await dispatch(page, 'A', 'lifecycle-retry-old', zone)
    expect(old.autoClose).toBe(!enabled)
    await post(page, '/api/workflow/manager/fail', { sessionId: manager, nodeId: 'A', reason: 'Original attempt failed' })
    await setChoice(page, 'A', enabled)
    host('lifecycle-retry-new', zone)
    const result = await post(page, '/api/workflow/manager/retry', { sessionId: manager, nodeId: 'A', confirmed: true, reason: 'Explicit independent new attempt' })
    expect(result.result.status).toBe('running')
    const current = result.snapshot.nodes.A.execution
    expect(current.autoClose).toBe(enabled)
    expect(current.id).not.toBe(old.id)
    expect(current.chat.sessionId).toBe('lifecycle-retry-new')
    expect(current.chat.tabId).not.toBe(old.chat.tabId)
    expect(result.snapshot.nodes.A.previousExecutions[0].execution).toEqual(old)
    const completed = await complete(page, 'A')
    expect(completed.nodes.A.execution).toEqual(current)
    expect((await readRun(page, runId)).run.nodes.A.previousExecutions[0].execution).toEqual(old)
    if (enabled) await expectClosed(current.chat.tabId, 1)
    await noMoreCloses(page, enabled ? [['tab', 'close', current.chat.tabId]] : [])
  })
}
