import fs from 'node:fs'
import path from 'node:path'
import { test, expect, type Page, type Locator, type TestInfo } from '@playwright/test'
import { startServer, type E2EServer } from './support'
import { expectNoHorizontalOverflow } from './ui'

let server: E2EServer | null = null
test.afterEach(async () => { await server?.cleanup(); server = null })
const reference = (sessionId: string) => ({ sessionId, workspaceId: 'saved-workspace', tabId: `saved-${sessionId}`, paneId: `saved-pane-${sessionId}`, agentName: 'saved-name' })
const target = (id: string, pane: string) => ({ agent: 'omp', name: `decoy-name-${id}`, workspace_id: 'live-workspace', tab_id: `live-tab-${pane}`, pane_id: pane, focused: false, agent_session: { source: 'herdr:omp', agent: 'omp', kind: 'id', value: id } })
async function fixture(condition: 'missing' | 'invalid' | 'mismatch' = 'missing') {
  server = await startServer({ separateCwd: true, herdr: { behavior: { agents: [target('manager-native', 'manager-pane'), target('current-native', 'current-pane'), target('old-native', 'old-pane'), target('wrong-native', 'decoy-pane')] } } })
  for (const sessionId of ['manager-native', 'current-native', 'old-native']) {
    const response = await fetch(`${server.baseURL}/api/sessions/events`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ sessionId, kind: 'register', data: { agentType: 'omp', cwd: server.cwd, sessionName: 'Same display name' } }) })
    expect(response.status).toBe(200)
  }
  const directory = path.join(server.cwd, '.data/workflow-runs/history-run')
  fs.mkdirSync(directory, { recursive: true })
  if (condition !== 'missing') {
    const maps = path.join(server.cwd, '.data/workflow-maps')
    fs.mkdirSync(maps, { recursive: true })
    fs.writeFileSync(path.join(maps, 'missing.json'), condition === 'invalid' ? '{ invalid current definition' : JSON.stringify({ title: 'Unrelated definition', description: '', nodes: { Z: { title: 'Never map saved A here', bot_id: 'absent-bot', zone: '', input: '', blocked_by: [] } } }))
  }
  const manager = reference('manager-native'), current = reference('current-native'), old = reference('old-native')
  const run = { id: 'history-run', map: 'missing.json', createdAt: 1000, managerTaskId: 'manager-task', zone: server.cwd, workspaceName: 'saved-workspace', userInput: 'KEEP USER INPUT', status: 'running', manager, finished: null, nodes: {
    A: { status: 'running', execution: { agent: 'omp', id: 'exec-new', taskId: 'task-new', zone: 'C:/new-zone', chat: current }, failure: null, completion: null, previousExecutions: [{ execution: { agent: 'omp', id: 'exec-old', taskId: 'task-old', zone: 'C:/old-zone', chat: old }, failure: { stage: 'execution', message: 'OLD FAILURE' } }] },
    B: { status: 'pending', execution: null, failure: null, completion: null, previousExecutions: [] },
  } }
  const reports = [
    { id: 'report-new', type: 'report', at: 5000, runId: run.id, nodeId: 'A', executionId: 'exec-new', source: current, target: manager, body: 'CURRENT APPLICABLE BODY' },
    { id: 'report-old', type: 'report', at: 2000, runId: run.id, nodeId: 'A', executionId: 'exec-old', source: old, target: manager, body: 'OLD BODY NOT APPLICABLE' },
  ]
  fs.writeFileSync(path.join(directory, 'run.json'), JSON.stringify(run))
  fs.writeFileSync(path.join(directory, 'messages.jsonl'), reports.map(report => JSON.stringify(report)).join('\n') + '\n')
  fs.writeFileSync(path.join(directory, 'history.jsonl'), '')
  return { server, directory }
}
async function saveEvidence(info: TestInfo, name: string, data: unknown) {
  const file = info.outputPath(`${name}.json`)
  fs.writeFileSync(file, JSON.stringify(data, null, 2))
  await info.attach(name, { path: file, contentType: 'application/json' })
}
async function resume(page: Page, scope: Locator, sessionId: string) {
  const response = page.waitForResponse(reply => reply.url().endsWith('/api/chats/resume') && reply.request().method() === 'POST')
  await scope.getByRole('button', { name: 'Resume', exact: true }).click()
  const reply = await response
  expect(reply.request().postDataJSON()).toEqual({ sessionId })
  return { status: reply.status(), body: await reply.json() }
}

// Approved seams S1/S2/S3/S4; independent literal current/old identities and
// report bodies above. Violation: old report displayed as current, expanded
// history, or Resume targeting saved pane/name instead of exact native Chat.
// The actual delivered page + HTTP + production CLI adapter remain exercised;
// only Herdr is controlled. These calls do not prove native host navigation.
for (const condition of ['missing', 'invalid', 'mismatch'] as const) {
test(`P52.02 P53.01 P57.02 ${condition} Map retains current and collapsed old history Resume @time-contract`, async ({ page }, info) => {
  const { server, directory } = await fixture(condition)
  const files = ['run.json', 'history.jsonl', 'messages.jsonl']
  const before = files.map(file => fs.readFileSync(path.join(directory, file), 'utf8'))
  await page.goto(`${server.baseURL}/#workflow/history-run`)
  await expect(page.getByText(/^历史现场 \/ 未接管：/)).toBeVisible()
  await expect(page.getByRole('alert').filter({ hasText: '不补造历史图' })).toBeVisible()
  await expect(page.getByRole('group', { name: 'Map 依赖图', exact: true })).toHaveCount(0)
  const details = page.getByRole('region', { name: '当前节点详情', exact: true })
  const current = details.getByRole('region', { name: '当前执行 Chat', exact: true })
  await expect(current).toContainText('current-native')
  const applicable = details.getByRole('region', { name: '当前执行适用报告', exact: true })
  await expect(applicable).toContainText('CURRENT APPLICABLE BODY')
  await expect(applicable).not.toContainText('OLD BODY NOT APPLICABLE')
  const history = details.locator('details[aria-label="已保存旧执行事实"]')
  await expect(history).not.toHaveAttribute('open')
  await expect(history.getByText('OLD FAILURE')).not.toBeVisible()
  await history.getByText('旧执行历史', { exact: false }).click()
  await expect(history).toHaveAttribute('open', '')
  await expect(history).toContainText('exec-old')
  await expect(history).toContainText('task-old')
  await expect(history).toContainText('C:/old-zone')
  await expect(history).toContainText('OLD BODY NOT APPLICABLE')
  await expect(applicable.locator('time')).toHaveAttribute('datetime', '1970-01-01T00:00:05.000Z')
  await expect(history.locator('time')).toHaveAttribute('datetime', '1970-01-01T00:00:02.000Z')
  await page.locator('.workflow-run-background > summary').click()
  for (const [scope, id, pane] of [
    [page.getByRole('region', { name: '管理者 Chat', exact: true }), 'manager-native', 'manager-pane'],
    [current, 'current-native', 'current-pane'],
    [history.getByRole('region', { name: '旧执行 Chat', exact: true }), 'old-native', 'old-pane'],
  ] as const) {
    const result = await resume(page, scope, id)
    expect(result.status).toBe(200)
    expect(result.body.outcome).toBe('located')
    await expect(scope.getByRole('status')).toContainText('已切换到当前herdr焦点')
    expect(JSON.parse(fs.readFileSync(path.join(path.dirname(server.configPath), 'herdr/focused.json'), 'utf8'))).toBe(pane)
  }
  const start = Date.now()
  await page.waitForTimeout(300)
  const calls = server.herdrCalls()
  expect(calls.filter(call => call.kind === 'agent_focus').map(call => call.argv)).toEqual([['agent', 'focus', 'manager-pane'], ['agent', 'focus', 'current-pane'], ['agent', 'focus', 'old-pane']])
  expect(calls.some(call => ['agent_start', 'tab_create', 'agent_prompt', 'agent_input'].includes(String(call.kind)))).toBe(false)
  const detail = await (await fetch(`${server.baseURL}/api/workflow/runs/history-run`)).json()
  expect(detail.managed).toBe(false)
  expect(detail.run.nodes.A.status).toBe('running')
  expect(files.map(file => fs.readFileSync(path.join(directory, file), 'utf8'))).toEqual(before)
  await page.screenshot({ path: info.outputPath('current-old-manager-resume.png'), fullPage: true })
  await page.getByRole('navigation', { name: '已保存节点状态' }).locator('[data-node-id="B"]').click()
  await expect(details.getByRole('button', { name: 'Resume', exact: true })).toHaveCount(0)
  await expect(details).toContainText('尚无当前执行或 Chat 关联')
  await page.waitForTimeout(150)
  expect(server.herdrCalls()).toEqual(calls)
  expect(files.map(file => fs.readFileSync(path.join(directory, file), 'utf8'))).toEqual(before)
  await page.screenshot({ path: info.outputPath('no-definition-unassociated-node.png'), fullPage: true })
  await saveEvidence(info, 'bounded-resume-calls-NOT-real-Herdr', { observationMs: Date.now() - start, scope: 'Controlled Herdr adapter; not native host evidence or indefinite guarantee', condition, calls, detail, savedRecords: before })
})

}

// P15.04/P68.09: independent literal initial prompt + latest extension list.
// Violation: a workflow Resume replays launch-only instructions, restores stale
// env, or grants old manager ownership. Actually start and dispatch the workflow
// before editing its source Task; don't seed provenance or mock Resume internals.
// Existing backend Resume already promises this behavior; verify it through the
// newly delivered workflow page rather than manufacture a red.
test('P15.04 P53.01 P68.09 workflow Resume uses latest source Task env without triggers or takeover @time-contract', async ({ page }, info) => {
  server = await startServer({ separateCwd: true, herdr: { workspaces: [{ workspace_id: 'workflow-workspace', label: 'process-cwd', cwd: 'C:/unused-default' }] } })
  const host = server, root = path.dirname(host.configPath)
  const post = async (route: string, body: unknown, status = 200) => {
    const response = await fetch(`${host.baseURL}${route}`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify(body) })
    expect(response.status, await response.clone().text()).toBe(status)
    return response.json()
  }
  const zone = (await post('/api/zones', { path: host.cwd })).zones[0].path
  const base = { agent: 'omp', chatName: '{chat.id}', model: 'gpt-6.1-sol', thinking: 'high', skill: 'teach', extensions: [] }
  const managerTask = (await post('/api/tasks', { ...base, name: 'Manager source', taskType: 'manager', prompt: 'MANAGER INITIAL' })).tasks[0]
  const nodeConfig = { ...base, name: 'Node source', taskType: 'normal', prompt: 'NODE INITIAL ONLY', extensions: ['omp-connect-bot'] }
  const nodeTask = (await post('/api/tasks', nodeConfig)).tasks.find((task: { name: string }) => task.name === 'Node source')
  const maps = path.join(host.cwd, '.data/workflow-maps'), bots = path.join(host.cwd, '.data/bots')
  fs.mkdirSync(maps, { recursive: true }); fs.mkdirSync(bots, { recursive: true })
  fs.writeFileSync(path.join(maps, 'resume.json'), JSON.stringify({ title: 'Resume Map', description: '', nodes: { A: { title: 'Actual dispatch', bot_id: 'worker', input: 'NODE INPUT ONLY', zone, blocked_by: [] } } }))
  fs.writeFileSync(path.join(bots, 'worker.json'), JSON.stringify({ id: 'worker', name: '', when: '', plan_guide: '', node_guide: '', task_id: nodeTask.id }))
  const native = (id: string) => {
    const file = path.join(host.cwd, `${id}.jsonl`)
    fs.writeFileSync(file, JSON.stringify({ type: 'session', version: 3, id, cwd: host.cwd }) + '\n')
    host.setHerdrBehavior({ native_session: { agent: 'omp', source: 'herdr:omp', kind: 'path', value: file } })
    return file
  }
  native('real-dispatch-manager')
  const { id } = await post('/api/scheduler/maps/resume.json/runs', { managerTaskId: managerTask.id, zone, userInput: 'RUN INPUT ONLY' }, 201)
  const sessionFile = native('real-dispatch-node')
  expect((await post('/api/workflow/manager/dispatch', { sessionId: 'real-dispatch-manager', nodes: ['A'] })).results[0].status).toBe('running')
  const initialCalls = host.herdrCalls()
  const initialPrompt = initialCalls.filter(call => call.kind === 'agent_prompt').at(-1)!.text as string
  expect(initialPrompt).toMatch(/^\/skill:teach NODE INITIAL ONLY\n\nNode ID: A\nNode: Actual dispatch\n/)
  expect(initialPrompt).toMatch(/\n\nNODE INPUT ONLY$/)
  const initialStart = initialCalls.filter(call => call.kind === 'agent_start').at(-1)!
  expect((initialStart.args as string[]).slice(0, 4)).toEqual(['--model', 'openai-codex/gpt-6.1-sol', '--thinking', 'high'])
  const sourceHistory = JSON.parse(fs.readFileSync(path.join(root, 'session-history.json'), 'utf8'))
  expect(sourceHistory.find((chat: { sessionId: string }) => chat.sessionId === 'real-dispatch-node').sourceTaskId).toBe(nodeTask.id)
  const edited = await fetch(`${host.baseURL}/api/tasks/${nodeTask.id}`, { method: 'PUT', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ ...nodeConfig, name: 'Renamed source', extensions: ['omp-codebase-tools'] }) })
  expect(edited.status).toBe(200)
  const latest = path.join(root, 'latest-extensions')
  fs.mkdirSync(latest)
  const extensionIds = ['omp-herdr-agent-state', 'omp-model-cost', 'omp-node-chat', 'omp-codebase-tools']
  for (const entry of extensionIds) fs.writeFileSync(path.join(latest, `${entry}.ts`), "throw new Error('controlled fixture never loads extensions')\n")
  const metadata = fs.readFileSync(host.metadataFile, 'utf8').replaceAll(path.join(root, 'extensions'), latest)
  fs.writeFileSync(host.metadataFile, metadata)
  const directory = path.join(host.cwd, '.data/workflow-runs', id)
  const files = ['run.json', 'history.jsonl', 'messages.jsonl']
  const saved = files.map(file => fs.readFileSync(path.join(directory, file), 'utf8'))
  const before = host.herdrCalls().length
  await page.goto(`${host.baseURL}/#workflow/${id}`)
  await expect(page.getByText(/^历史现场 \/ 未接管：/)).toHaveCount(0)
  const scope = page.getByRole('region', { name: '当前执行 Chat', exact: true })
  const restored = await resume(page, scope, 'real-dispatch-node')
  expect(restored.body.outcome).toBe('restored')
  await expect(scope.getByRole('status')).toContainText('已确认恢复同一 OMP 原生对话')
  await expect(scope.getByRole('status')).toContainText('未提交输入或重新应用 Task 预设')
  const observationStart = Date.now()
  await page.waitForTimeout(350)
  const managedObservationMs = Date.now() - observationStart
  const calls = host.herdrCalls().slice(before)
  expect(calls.filter(call => call.kind === 'agent_start')).toHaveLength(1)
  expect(calls.find(call => call.kind === 'agent_start')!.args).toEqual(['--no-extensions', '--extension', path.join(latest, 'omp-herdr-agent-state.ts'), '--extension', path.join(latest, 'omp-model-cost.ts'), '--extension', path.join(latest, 'omp-node-chat.ts'), '--extension', path.join(latest, 'omp-codebase-tools.ts'), '--resume', sessionFile])
  expect(calls.some(call => ['agent_prompt', 'agent_input'].includes(String(call.kind)))).toBe(false)
  expect(files.map(file => fs.readFileSync(path.join(directory, file), 'utf8'))).toEqual(saved)
  const current = await (await fetch(`${host.baseURL}/api/workflow/runs/${id}`)).json()
  expect(current.managed).toBe(true)
  expect(current.run.nodes.A.status).toBe('running')
  await host.restart()
  const runURLBeforeReload1 = page.url()

  await page.reload()

  await expect(page.getByRole('tab', { name: 'chat', exact: true })).toHaveAttribute('aria-selected', 'true')

  await page.goto(runURLBeforeReload1)
  await expect(page.getByText(/^历史现场 \/ 未接管：/)).toBeVisible()
  // Same current Chat in an unmanaged Run can still Resume; never restore the
  // manager's process-local write permission as a side effect of navigation.
  const again = await resume(page, page.getByRole('region', { name: '当前执行 Chat', exact: true }), 'real-dispatch-node')
  expect(again.body.outcome).toBe('restored')
  const unmanagedObservationStart = Date.now()
  await page.waitForTimeout(350)
  expect(host.herdrCalls().slice(before).some(call => ['agent_prompt', 'agent_input'].includes(String(call.kind)))).toBe(false)
  expect(host.herdrCalls().slice(before).filter(call => call.kind === 'agent_start')).toHaveLength(2)
  await expect(page.getByText(/^历史现场 \/ 未接管：/)).toBeVisible()
  const denied = await fetch(`${host.baseURL}/api/workflow/manager/status`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ sessionId: 'real-dispatch-manager' }) })
  expect(denied.status).toBe(403)
  expect((await denied.json()).code).toBe('manager-unassociated')
  expect((await (await fetch(`${host.baseURL}/api/workflow/runs/${id}`)).json()).managed).toBe(false)
  expect(files.map(file => fs.readFileSync(path.join(directory, file), 'utf8'))).toEqual(saved)
  await page.setViewportSize({ width: 390, height: 844 })
  await expectNoHorizontalOverflow(page)
  await page.screenshot({ path: info.outputPath('workflow-resumed-unmanaged.png'), fullPage: true })
  await saveEvidence(info, 'workflow-resume-environment-NOT-real-loading', { scope: 'Controlled CLI adapter; not native loading or indefinite guarantee', initialCalls, restored, again, managedObservationMs, unmanagedObservationMs: Date.now() - unmanagedObservationStart, elapsedSinceManagedObservation: Date.now() - observationStart, calls: host.herdrCalls().slice(before), current, savedRecords: saved })
})

// P53.01 source: ticket requires success/failure/unknown to stay distinct. A
// plausible violation renders any accepted start as success or retries on error.
// Inject deterministic failures in the external CLI, not in page/API internals.
test('P53.01 P68.09 manager current old Resume failures and unknown outcomes never imply success or takeover @time-contract', async ({ page }, info) => {
  const { server, directory } = await fixture()
  const files = ['run.json', 'history.jsonl', 'messages.jsonl']
  const before = files.map(file => fs.readFileSync(path.join(directory, file), 'utf8'))
  await page.goto(`${server.baseURL}/#workflow/history-run`)
  const details = page.getByRole('region', { name: '当前节点详情', exact: true })
  await details.locator('details[aria-label="已保存旧执行事实"] > summary').click()
  await page.locator('.workflow-run-background > summary').click()
  const cases = [
    { scope: page.getByRole('region', { name: '管理者 Chat', exact: true }), id: 'manager-native', behavior: { agents: [target('manager-native', 'manager-pane')], fail_agent_focus: true }, code: 'herdr-command-failed', text: 'Herdr 聚焦失败', outcome: 'failed' },
    { scope: details.getByRole('region', { name: '旧执行 Chat', exact: true }), id: 'old-native', behavior: { fail_agent_list: true }, code: 'herdr-command-failed', text: '查找／身份核实失败', outcome: 'failed' },
    { scope: details.getByRole('region', { name: '当前执行 Chat', exact: true }), id: 'current-native', behavior: { agents: [], timeout_agent_start: true }, code: 'restore-unconfirmed', text: '启动结果未确认', outcome: 'unconfirmed' },
  ]
  const results = []
  for (const scenario of cases) {
    server.setHerdrBehavior(scenario.behavior)
    const reply = await resume(page, scenario.scope, scenario.id)
    results.push({ id: scenario.id, reply })
    expect(reply.status).toBe(502)
    expect(reply.body.code).toBe(scenario.code)
    expect(reply.body.outcome).toBe(scenario.outcome)
    await expect(scenario.scope.getByRole('alert')).toContainText(scenario.text)
    await expect(scenario.scope.getByRole('status')).toHaveCount(0)
    if (scenario.outcome === 'unconfirmed') {
      expect(reply.body.progress).toEqual({ tabCreated: true, startSubmitted: true, conversationConfirmed: false, focused: false })
      for (const value of Object.values(reply.body.target)) await expect(scenario.scope.locator('dl[aria-label="Resume 目标"]')).toContainText(String(value))
    }
  }
  const observationStart = Date.now()
  await page.waitForTimeout(350)
  const calls = server.herdrCalls()
  expect(calls.filter(call => call.kind === 'agent_start')).toHaveLength(1)
  expect(calls.filter(call => call.kind === 'tab_create')).toHaveLength(1)
  expect(calls.some(call => ['agent_prompt', 'agent_input'].includes(String(call.kind)))).toBe(false)
  expect(files.map(file => fs.readFileSync(path.join(directory, file), 'utf8'))).toEqual(before)
  const detail = await (await fetch(`${server.baseURL}/api/workflow/runs/history-run`)).json()
  expect(detail.managed).toBe(false)
  expect(detail.run.status).toBe('running')
  expect(detail.run.nodes.A.status).toBe('running')
  await expect(page.getByText(/^历史现场 \/ 未接管：/)).toBeVisible()
  await expect(details.locator('.workflow-status')).toHaveAttribute('data-status', 'running')
  await page.setViewportSize({ width: 390, height: 844 })
  await expectNoHorizontalOverflow(page)
  await page.screenshot({ path: info.outputPath('resume-failures-unconfirmed.png'), fullPage: true })
  await saveEvidence(info, 'controlled-failure-unconfirmed-calls', { observationMs: Date.now() - observationStart, results, calls, detail, savedRecords: before, scope: 'Controlled adapter only; no indefinite absence claim' })
})
