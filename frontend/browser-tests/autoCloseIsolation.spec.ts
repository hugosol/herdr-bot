import fs from 'node:fs'
import path from 'node:path'
import { test, expect, type Page, type JSHandle } from '@playwright/test'
import { startServer, type E2EServer } from './support'

let server: E2EServer | null = null
test.afterEach(async () => { await server?.cleanup(); server = null })

// Approved seams: S1 delivered workflow, S2 real scheduler/manager HTTP,
// S3 actual CLI process boundary and delivered backend stdout/stderr. Only
// external Herdr responses are controlled; completion/persistence stay real.
async function scenario(page: Page) {
  server = await startServer({ separateCwd: true, herdr: {} })
  const host = server
  const post = async (route: string, data: object, status = 200) => {
    const response = await page.request.post(`${host.baseURL}${route}`, { data })
    expect(response.status(), await response.text()).toBe(status)
    return response.json()
  }
  const zone = path.join(host.cwd, 'bot-playground')
  fs.mkdirSync(zone)
  await post('/api/zones', { path: zone })
  const tasks: Record<string, string> = {}
  for (const role of ['manager', 'normal']) {
    const result = await post('/api/tasks', { name: role, taskType: role, agent: 'omp', chatName: '{chat.id}', model: 'default', thinking: 'default', skill: null, prompt: '', extensions: [] })
    tasks[role] = result.tasks.find((task: { name: string }) => task.name === role).id
  }
  const bots = path.join(host.cwd, 'meta-data/bots')
  const maps = path.join(host.cwd, 'meta-data/workflow-maps')
  fs.mkdirSync(bots, { recursive: true }); fs.mkdirSync(maps, { recursive: true })
  fs.writeFileSync(path.join(bots, 'worker.json'), JSON.stringify({ id: 'worker', name: 'Worker', when: '', plan_guide: '', node_guide: '', task_id: tasks.normal }))
  fs.writeFileSync(path.join(maps, 'isolation.json'), JSON.stringify({ title: '关闭隔离验收', description: '', nodes: {
    A: { title: '交接工作', bot_id: 'worker', zone, input: '', blocked_by: [], auto_close: true },
    B: { title: '后续工作', bot_id: 'worker', zone, input: '', blocked_by: ['A'] },
  } }))
  const native = (id: string) => ({ native_session: { source: 'herdr:omp', agent: 'omp', kind: 'id', value: id }, native_cwd: zone })
  host.setHerdrBehavior(native('isolation-manager'))
  const started = await post('/api/scheduler/maps/isolation.json/runs', { managerTaskId: tasks.manager, zone, userInput: '验收关闭隔离' }, 201)
  host.setHerdrBehavior(native('isolation-node-A'))
  const dispatched = await post('/api/workflow/manager/dispatch', { sessionId: 'isolation-manager', nodes: ['A'] })
  expect(dispatched.results[0].status).toBe('running')
  const root = path.join(host.cwd, '.data/workflow-runs', started.id)
  const readRun = () => JSON.parse(fs.readFileSync(path.join(root, 'run.json'), 'utf8'))
  const complete = () => post('/api/workflow/manager/complete', { sessionId: 'isolation-manager', nodeId: 'A', reason: '已核实交接结果' })
  const closeCalls = () => host.herdrCalls().filter(call => call.kind === 'tab_close')
  const invocations = (): string[][] => fs.readFileSync(path.join(path.dirname(host.configPath), 'herdr/invocations.jsonl'), 'utf8').trim().split('\n').map(line => JSON.parse(line).argv)
  const invocationsBefore = invocations()
  const logs = () => host.diagnostics().split(/\r?\n/).filter(line => line.includes('auto-close='))
  const before = readRun()
  const historyBefore = fs.readFileSync(path.join(root, 'history.jsonl'), 'utf8').trim().split('\n').filter(Boolean).map(line => JSON.parse(line))
  const filesBefore = fs.readdirSync(root).sort()
  const execution = before.nodes.A.execution
  const context = (line: string) => line.includes(started.id) && line.includes('node=A') && line.includes(execution.id)
  const dialogs: string[] = []
  page.on('dialog', async dialog => { dialogs.push(dialog.message()); await dialog.dismiss() })
  await page.goto(`${host.baseURL}/#workflow/${started.id}`)
  await expect(page.getByRole('navigation', { name: '已保存节点状态' }).locator('[data-node-id="A"]')).toHaveAttribute('data-node-state', 'running')
  // Observe the public DOM while complete happens, including transient output
  // that would be missed by opening the workflow only after cleanup finished.
  const workflowOutput = await page.evaluateHandle(() => {
    const output: string[] = []
    const observer = new MutationObserver(records => {
      for (const record of records) {
        const nodes = record.type === 'characterData' ? [record.target] : [...record.addedNodes]
        for (const node of nodes) if (node.textContent) output.push(node.textContent)
      }
    })
    observer.observe(document.body, { childList: true, subtree: true, characterData: true })
    return output
  })
  return { host, post, native, root, readRun, before, historyBefore, filesBefore, invocations, invocationsBefore, complete, closeCalls, logs, context, dialogs, workflowOutput, runId: started.id, tab: execution.chat.tabId }
}

type CompletionResult = {
  changed: boolean
  snapshot: { nodes: Record<string, { status: string; completion: { id: string; at: number; actor: unknown; reason: string } }> }
}
type CleanupScenario = {
  host: E2EServer
  post: (route: string, data: object, status?: number) => Promise<CompletionResult & { results: { status: string }[] }>
  native: (id: string) => Record<string, unknown>
  root: string
  historyBefore: unknown[]
  filesBefore: string[]
  invocations: () => string[][]
  invocationsBefore: string[][]
  dialogs: string[]
  workflowOutput: JSHandle<string[]>
  // JSON fixtures are constructed/checked through the public HTTP path above.
  readRun: () => { nodes: Record<string, { status: string; execution: unknown }> }
  before: { nodes: Record<string, { status: string; execution: unknown }> }
  complete: () => Promise<CompletionResult>
  runId: string
  tab: string
}

async function observeUnchangedBusiness(page: Page, s: CleanupScenario, result: CompletionResult, externalClose: 'attempted' | 'not-started' = 'attempted') {
  expect(s.invocations().slice(s.invocationsBefore.length)).toEqual(externalClose === 'attempted' ? [['tab', 'close', s.tab]] : [])
  expect(result.changed).toBe(true)
  expect(result.snapshot.nodes.A.status).toBe('completed')
  const completion = result.snapshot.nodes.A.completion
  expect(completion.reason).toBe('已核实交接结果')
  expect(Object.keys(completion).sort()).toEqual(['actor', 'at', 'id', 'reason'])
  expect(Object.keys(result.snapshot).sort()).toEqual(['createdAt', 'finished', 'id', 'manager', 'managerTaskId', 'map', 'nodes', 'status', 'userInput', 'workspaceName', 'zone'].sort())
  expect(Object.keys(result.snapshot.nodes.A).sort()).toEqual(['completion', 'execution', 'failure', 'previousExecutions', 'status'])
  const execution = s.before.nodes.A.execution
  if (execution === null || typeof execution !== 'object') throw new Error('Expected a dispatched execution')
  expect(Object.keys(execution).sort()).toEqual(['agent', 'autoClose', 'chat', 'id', 'reportPolicy', 'taskId', 'zone'])
  // Independent expected delta: completion is the only new durable fact. Exact
  // equality rejects result fields even if they have innocuous or unknown names.
  expect(s.readRun()).toEqual({ ...s.before, nodes: { ...s.before.nodes, A: { ...s.before.nodes.A, status: 'completed', completion } } })
  const history = fs.readFileSync(path.join(s.root, 'history.jsonl'), 'utf8').trim().split('\n').map(line => JSON.parse(line))
  const event = history.find(entry => entry.action === 'complete')
  expect(event).toEqual({ id: completion.id, at: completion.at, actor: completion.actor, action: 'complete', nodeId: 'A', before: 'running', after: 'completed', reason: completion.reason, execution: s.before.nodes.A.execution })
  expect(history).toEqual([...s.historyBefore, event])
  expect(fs.readdirSync(s.root).sort()).toEqual(s.filesBefore)
  expect(fs.readFileSync(path.join(s.root, 'messages.jsonl'), 'utf8')).toBe('')
  const detail = await (await page.request.get(`${s.host.baseURL}/api/workflow/runs/${s.runId}`)).json()
  expect(detail.run).toEqual(result.snapshot)
  expect(detail.reports).toEqual([])
  expect(Object.keys(detail).sort()).toEqual(['id', 'run', 'managed', 'startupError', 'readError', 'reports', 'pendingReports'].sort())
  expect(Object.keys(result).sort()).toEqual(['blocked', 'canFinish', 'changed', 'historyError', 'pendingReports', 'ready', 'reports', 'snapshot'].sort())
  // One bounded negative-observation window, not a claim about infinite time.
  await page.waitForTimeout(750)
  expect(s.dialogs).toEqual([])
  expect((await s.workflowOutput.jsonValue()).join('\n')).not.toMatch(/自动关闭[：:\s]*(成功|失败)|auto[-_ ]?close[=:\s]*(succeeded|failed)|confirmation_required|controlled-close-failure|herdr-unavailable/i)
  await page.goto(`${s.host.baseURL}/#workflow/${s.runId}`)
  const navigation = page.getByRole('navigation', { name: '已保存节点状态' })
  await expect(navigation.locator('[data-node-id="A"]')).toHaveAttribute('data-node-state', 'completed')
  await navigation.locator('[data-node-id="A"]').click()
  await expect(page.getByRole('region', { name: '当前节点详情' })).toContainText('已核实交接结果')
  await expect(page.getByRole('alert')).toHaveCount(0)
  await expect(page.locator('body')).not.toContainText(/自动关闭[：:\s]*(成功|失败)|auto[-_ ]?close[=:\s]*(succeeded|failed)|confirmation_required|controlled-close-failure|herdr-unavailable/i)
  s.host.setHerdrBehavior(s.native('isolation-node-B'))
  const downstream = await s.post('/api/workflow/manager/dispatch', { sessionId: 'isolation-manager', nodes: ['B'] })
  expect(downstream.results[0].status).toBe('running')
  expect(downstream.snapshot.nodes.A.completion).toEqual(completion)
  const duplicate = await s.complete()
  expect(duplicate.changed).toBe(false)
  expect(duplicate.snapshot.nodes.A.completion).toEqual(completion)
  const businessCommands = new Set(s.invocationsBefore.map(argv => argv.slice(0, 2).join(' ')))
  for (const argv of s.invocations()) {
    if (argv[0] === 'tab' && argv[1] === 'close') expect(argv).toEqual(['tab', 'close', s.tab])
    else expect(businessCommands.has(argv.slice(0, 2).join(' ')), JSON.stringify(argv)).toBe(true)
    expect(argv.filter(arg => ['--force', '--confirm', 'confirm'].includes(arg))).toEqual([])
  }
}

// P20/P21 source: the ticket requires logs-only cleanup outcomes and unchanged
// business facts/UI. Violation: success leaks a result field/notification or
// claims success without an external attempt. Real delivered process is observed.
test('P20 P21 successful cleanup is contextual backend output, not a workflow result @time-contract', async ({ page }, info) => {
  const s = await scenario(page)
  const result = await s.complete()
  await expect.poll(() => s.logs().filter(line => /succeeded|success/i.test(line)).length).toBe(1)
  expect(s.logs().every(s.context)).toBe(true)
  expect(s.logs().filter(line => /attempt/.test(line))).toHaveLength(1)
  expect(s.logs().every(line => line.includes(s.tab))).toBe(true)
  expect(s.closeCalls()).toEqual([expect.objectContaining({ target: s.tab, argv: ['tab', 'close', s.tab] })])
  await observeUnchangedBusiness(page, s, result)
  await page.waitForTimeout(750)
  expect(s.closeCalls()).toHaveLength(1)
  fs.writeFileSync(info.outputPath('success-evidence.json'), JSON.stringify({ logs: s.logs(), calls: s.host.herdrCalls(), result, run: s.readRun() }, null, 2))
})

// P17/P20/P21: a rejected external close must not roll back completion, block
// downstream dispatch, retry, or create a UI/durable cleanup result. A host
// adapter that ignores this failure would log success and fail this observation.
test('P17 P20 P21 rejected close leaves completion and downstream business intact @time-contract', async ({ page }, info) => {
  const s = await scenario(page)
  s.host.setHerdrBehavior({ fail_tab_close: true })
  const result = await s.complete()
  await expect.poll(() => s.logs().filter(line => /failed/.test(line) && line.includes('controlled-close-failure')).length).toBe(1)
  expect(s.logs().every(s.context)).toBe(true)
  expect(s.logs().filter(line => /attempt/.test(line))).toHaveLength(1)
  expect(s.logs().some(line => /succeeded|success/i.test(line))).toBe(false)
  expect(s.closeCalls()).toEqual([expect.objectContaining({ target: s.tab, argv: ['tab', 'close', s.tab] })])
  await observeUnchangedBusiness(page, s, result)
  await page.waitForTimeout(750)
  expect(s.closeCalls()).toHaveLength(1)
  expect(s.logs().filter(line => /attempt/.test(line))).toHaveLength(1)
  fs.writeFileSync(info.outputPath('command-failure-evidence.json'), JSON.stringify({ logs: s.logs(), calls: s.host.herdrCalls(), result, run: s.readRun() }, null, 2))
})

// P17/P20 source: Herdr's structured confirmation_required error (tabs.rs)
// has a human message without the code. Violation: discarding that category,
// retrying with force/confirmation, or adding a project confirmation workflow.
test('P17 P20 extra confirmation remains a diagnosed failure without bypass or new UI @time-contract', async ({ page }, info) => {
  const s = await scenario(page)
  s.host.setHerdrBehavior({ tab_close_confirmation_required: true })
  const callsBefore = s.host.herdrCalls()
  const result = await s.complete()
  await expect.poll(() => s.logs().filter(line => /failed/.test(line)).length).toBe(1)
  expect(s.logs().some(line => /confirmation[_-]required/.test(line))).toBe(true)
  expect(s.logs().every(s.context)).toBe(true)
  expect(s.logs().filter(line => /attempt/.test(line))).toHaveLength(1)
  expect(s.logs().some(line => /succeeded|success/i.test(line))).toBe(false)
  expect(s.host.herdrCalls().slice(callsBefore.length)).toEqual([expect.objectContaining({ kind: 'tab_close', target: s.tab, argv: ['tab', 'close', s.tab] })])
  await observeUnchangedBusiness(page, s, result)
  await expect(page.getByRole('button', { name: /确认关闭|强制关闭|confirm.*clos|force.*clos/i })).toHaveCount(0)
  await page.waitForTimeout(750)
  expect(s.closeCalls()).toHaveLength(1)
  expect(s.logs().filter(line => /attempt/.test(line))).toHaveLength(1)
  fs.writeFileSync(info.outputPath('confirmation-evidence.json'), JSON.stringify({ logs: s.logs(), calls: s.host.herdrCalls(), result, run: s.readRun() }, null, 2))
})

// P17/P20: remove the test-owned configured executable only after real dispatch.
// This exercises production spawn failure, not an adapter pretending unavailable.
// One backend attempt and zero started CLI calls distinguish these two facts.
test('P17 P20 unavailable executable attempts once without starting a close command @time-contract', async ({ page }, info) => {
  const s = await scenario(page)
  const executable = path.join(path.dirname(s.host.configPath), 'herdr/herdr-fixture.exe')
  const parked = `${executable}.unavailable`
  const callsBefore = s.host.herdrCalls()
  fs.renameSync(executable, parked)
  try {
    const result = await s.complete()
    await expect.poll(() => s.logs().filter(line => /failed/.test(line) && line.includes('herdr-unavailable')).length).toBe(1)
    expect(s.logs().every(s.context)).toBe(true)
    expect(s.logs().filter(line => /attempt/.test(line))).toHaveLength(1)
    expect(s.logs().some(line => /succeeded|success/i.test(line))).toBe(false)
    expect(s.host.herdrCalls()).toEqual(callsBefore)
    fs.renameSync(parked, executable)
    // Restoring availability must not retry cleanup; it does allow the manager
    // to dispatch genuinely new downstream business in this same Run.
    await observeUnchangedBusiness(page, s, result, 'not-started')
    await page.waitForTimeout(750)
    expect(s.closeCalls()).toHaveLength(0)
    expect(s.logs().filter(line => /attempt/.test(line))).toHaveLength(1)
    fs.writeFileSync(info.outputPath('unavailable-evidence.json'), JSON.stringify({ logs: s.logs(), calls: s.host.herdrCalls(), result, run: s.readRun(), externalCloseProcessesStarted: s.closeCalls().length }, null, 2))
  } finally {
    if (fs.existsSync(parked)) fs.renameSync(parked, executable)
  }
})

// P14/P20: isolated historical facts are the S2-approved persistence fixture.
// Preserve a legitimate dispatched execution with autoClose=true, but remove
// its reliable target. A guessed target or discovery call changes the recorder;
// merely asserting completed would not catch either violation.
for (const association of ['missing-chat', 'blank-tab'] as const) {
  test(`P14 P20 historical ${association} skips cleanup without guessing and still completes @time-contract`, async ({ page }, info) => {
    const s = await scenario(page)
    if (association === 'missing-chat') s.before.nodes.A.execution.chat = null
    else s.before.nodes.A.execution.chat.tabId = '   '
    fs.writeFileSync(path.join(s.root, 'run.json'), JSON.stringify(s.before))
    const callsBefore = s.host.herdrCalls()
    const result = await s.complete()
    await expect.poll(() => s.logs().filter(line => /skipp/.test(line) && /missing|association|target/.test(line)).length).toBe(1)
    expect(s.logs().every(s.context)).toBe(true)
    expect(s.logs().some(line => /attempt|succeeded|success/i.test(line))).toBe(false)
    await page.waitForTimeout(750)
    expect(s.host.herdrCalls()).toEqual(callsBefore)
    await observeUnchangedBusiness(page, s, result, 'not-started')
    expect(s.closeCalls()).toHaveLength(0)
    expect(s.logs()).toHaveLength(1)
    fs.writeFileSync(info.outputPath(`${association}-evidence.json`), JSON.stringify({ logs: s.logs(), callsBefore, callsAfter: s.host.herdrCalls(), result, run: s.readRun() }, null, 2))
  })
}
