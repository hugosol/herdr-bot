import fs from 'node:fs'
import path from 'node:path'
import { test, expect } from '@playwright/test'
import { startServer, type E2EServer } from './support'
import { expectNoHorizontalOverflow } from './ui'

let server: E2EServer | null = null
test.afterEach(async () => { await server?.cleanup(); server = null })

const chat = { sessionId: 'saved-node-chat', workspaceId: 'w1', tabId: 't1', paneId: 'p1', agentName: 'omp' }
const node = (status: string) => ({ status, execution: null, failure: null, completion: null, previousExecutions: [] })
async function fixture() {
  server = await startServer({ separateCwd: true, herdr: {} })
  const root = path.join(server.cwd, '.data/workflow-runs')
  fs.mkdirSync(root, { recursive: true })
  const runs = [
    { id: 'old-run', map: 'alpha.json', createdAt: 1000, status: 'completed', nodes: { A: node('completed') } },
    { id: 'new-run', map: 'beta.json', createdAt: 3000, status: 'paused', nodes: { A: node('completed'), B: node('failed'), C: node('pending') } },
    { id: 'middle-run', map: 'alpha.json', createdAt: 2000, status: 'running', nodes: { A: node('running') } },
  ]
  for (const sample of runs) {
    const dir = path.join(root, sample.id)
    fs.mkdirSync(dir)
    fs.writeFileSync(path.join(dir, 'run.json'), JSON.stringify({ managerTaskId: 'saved-manager-task', zone: 'C:/saved-zone', workspaceName: 'saved-workspace', userInput: `USER INPUT ${sample.id}`, manager: { ...chat, sessionId: 'saved-manager-chat' }, finished: null, ...sample }))
    fs.writeFileSync(path.join(dir, 'history.jsonl'), '')
    fs.writeFileSync(path.join(dir, 'messages.jsonl'), '')
  }
  return { server, root }
}

async function currentFixture() {
  const context = await fixture()
  const { server, root } = context
  const maps = path.join(server.cwd, '.data/workflow-maps'), bots = path.join(server.cwd, '.data/bots')
  fs.mkdirSync(maps, { recursive: true }); fs.mkdirSync(bots, { recursive: true })
  const taskResponse = await fetch(`${server.baseURL}/api/tasks`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ name: '当前执行 Task', taskType: 'normal', agent: 'omp', chatName: '{chat.id}', model: 'default', thinking: 'default', skill: null, prompt: '', extensions: [] }) })
  expect(taskResponse.status).toBe(200)
  const taskId = (await taskResponse.json()).tasks[0].id
  fs.writeFileSync(path.join(bots, 'worker.json'), JSON.stringify({ id: 'worker', name: '当前共享 Bot', when: '', plan_guide: '', node_guide: '', task_id: taskId }))
  const definition = { title: '当前 Beta Map', description: '', nodes: {
    A: { title: '交接工作', bot_id: 'worker', zone: 'C:/definition-zone', input: '当前定义输入 A', blocked_by: [] },
    B: { title: '失败工作', bot_id: 'worker', zone: 'C:/definition-zone', input: '当前定义输入 B', blocked_by: ['A'] },
    C: { title: '依赖工作', bot_id: 'worker', zone: 'C:/definition-zone', input: '当前定义输入 C', blocked_by: ['B'] },
    D: { title: '就绪工作', bot_id: 'worker', zone: 'C:/definition-zone', input: '当前定义输入 D', blocked_by: [] },
    E: { title: '报告工作', bot_id: 'worker', zone: 'C:/definition-zone', input: '当前定义输入 E', blocked_by: [] },
  } }
  const mapPath = path.join(maps, 'beta.json')
  fs.writeFileSync(mapPath, JSON.stringify(definition))
  const runPath = path.join(root, 'new-run/run.json')
  const run = JSON.parse(fs.readFileSync(runPath, 'utf8'))
  run.status = 'running'
  run.nodes = {
    A: { ...node('completed'), execution: { agent: 'omp', id: 'exec-A', taskId, zone: 'C:/saved-execution-zone', chat }, completion: { id: 'complete-A', at: 4000, actor: run.manager, reason: '交接材料已核实' } },
    B: { ...node('failed'), execution: { agent: 'omp', id: 'exec-B', taskId: 'deleted-task', zone: 'C:/failed-zone', chat: { ...chat, sessionId: 'failed-chat' } }, failure: { stage: 'execution', message: '缺少已核实的源文件' } },
    C: node('pending'), D: node('pending'),
    E: { ...node('running'), execution: { agent: 'omp', id: 'exec-E', taskId, zone: 'C:/report-zone', chat: { ...chat, sessionId: 'report-chat-offline' } } },
  }
  fs.writeFileSync(runPath, JSON.stringify(run))
  const report = { id: 'report-current', type: 'report', at: 5000, runId: 'new-run', nodeId: 'E', executionId: 'exec-E', source: run.nodes.E.execution.chat, target: run.manager, body: '报告正文：结果在 result.txt，限制为只读' }
  fs.writeFileSync(path.join(root, 'new-run/messages.jsonl'), JSON.stringify(report) + '\n')
  // A is actually online/working while the saved node is completed. E has no
  // live registration while its saved state is running: never derive graph from Chat.
  const online = await fetch(`${server.baseURL}/api/sessions/events`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ sessionId: chat.sessionId, kind: 'register', data: { cwd: 'C:/saved-execution-zone', sessionName: '仍在收尾', agentType: 'omp' } }) })
  expect(online.status).toBe(200)
  const working = await fetch(`${server.baseURL}/api/sessions/events`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ sessionId: chat.sessionId, kind: 'state', data: { value: 'working' } }) })
  expect(working.status).toBe(200)
  return { ...context, mapPath, definition, run, runPath, report }
}

// P51.01/P52.01 source: literal definitions, execution facts and report above.
// Violation: painting Chat liveness as completion, reversing dependencies, or
// hiding saved execution values behind changed current definitions. Exercise
// real graph selection and inspect separate visible definition/execution facts.
test('P51.01 P52.01 current Map graph and node details distinguish execution facts from Chat status @time-contract', async ({ page }, info) => {
  const { server, root } = await currentFixture()
  const calls = server.herdrCalls()
  const records = ['run.json', 'history.jsonl', 'messages.jsonl'].map(file => fs.readFileSync(path.join(root, 'new-run', file), 'utf8'))
  await page.goto(server.baseURL)
  await expect(page.getByTestId('session-board')).toContainText('仍在收尾')
  await expect(page.getByTestId('session-board')).toContainText('工作中')
  await expect(page.getByTestId('session-board').getByRole('img', { name: '在线', exact: true })).toBeVisible()
  await page.goto(`${server.baseURL}/#workflow/new-run`)
  await expect(page.getByText('图来自当前 Map，非历史定义快照。', { exact: true })).toBeVisible()
  const graph = page.getByRole('group', { name: 'Map 依赖图', exact: true })
  await expect(graph.getByRole('button')).toHaveCount(5)
  await expect(graph.getByRole('img', { name: 'A → B', exact: true })).toHaveAttribute('marker-end', /arrow/)
  await expect(graph.getByRole('img', { name: 'B → C', exact: true })).toBeAttached()
  await graph.scrollIntoViewIfNeeded()
  for (const [source, target] of [['交接工作 A', '失败工作 B'], ['失败工作 B', '依赖工作 C']]) {
    const edge = graph.getByRole('img', { name: `${source.slice(-1)} → ${target.slice(-1)}`, exact: true })
    const ends = await edge.evaluate(element => {
      const line = element as SVGPathElement, matrix = line.getScreenCTM()!
      const start = line.getPointAtLength(0).matrixTransform(matrix), end = line.getPointAtLength(line.getTotalLength()).matrixTransform(matrix)
      const midpoint = line.getPointAtLength(line.getTotalLength() / 2).matrixTransform(matrix)
      return { start: { x: start.x, y: start.y }, end: { x: end.x, y: end.y }, painted: document.elementFromPoint(midpoint.x, midpoint.y) === line }
    })
    const from = (await graph.getByRole('button', { name: source, exact: true }).boundingBox())!
    const to = (await graph.getByRole('button', { name: target, exact: true }).boundingBox())!
    expect(ends.painted).toBe(true)
    expect(Math.abs(ends.start.x - (from.x + from.width))).toBeLessThan(2)
    expect(Math.abs(ends.end.x - to.x)).toBeLessThan(2)
  }
  await expect(graph.getByRole('button', { name: '依赖工作 C', exact: true })).toHaveCSS('border-style', 'dotted')
  await expect(graph.getByRole('button', { name: '失败工作 B', exact: true })).toHaveCSS('border-width', '3px')
  const a = graph.getByRole('button', { name: '交接工作 A', exact: true })
  await expect(a).toContainText('✓ completed')
  await expect(a).toHaveCSS('border-style', 'solid')
  await a.click()
  await expect(a).toHaveAttribute('aria-pressed', 'true')
  await expect(a).toHaveCSS('border-style', 'double')
  let details = page.getByRole('region', { name: '当前节点详情' })
  await expect(details).toContainText('当前共享 Bot · worker')
  await expect(details).toContainText('当前执行 Task')
  await expect(details).toContainText('C:/definition-zone')
  await expect(details).toContainText('C:/saved-execution-zone')
  await expect(details).toContainText('当前定义输入 A')
  await expect(details).toContainText('交接材料已核实')
  await expect(details.locator('time')).toHaveAttribute('datetime', '1970-01-01T00:00:04.000Z')
  await graph.getByRole('button', { name: '失败工作 B', exact: true }).click()
  await expect(details).toContainText('缺少已核实的源文件')
  await expect(details).toContainText('deleted-task')
  await expect(details).toContainText('交接工作 · A')
  await expect(graph.getByRole('button', { name: '失败工作 B', exact: true })).toContainText('! failed')
  await graph.getByRole('button', { name: '依赖工作 C', exact: true }).click()
  await expect(details).toContainText('前置未完成：B')
  await expect(details).toContainText('尚无当前执行或 Chat 关联；读取不会创建执行。')
  await expect(graph.getByRole('button', { name: '依赖工作 C', exact: true })).toContainText('⊘ blocked')
  await graph.getByRole('button', { name: '就绪工作 D', exact: true }).click()
  await expect(graph.getByRole('button', { name: '就绪工作 D', exact: true })).toContainText('○ ready')
  await graph.getByRole('button', { name: '报告工作 E', exact: true }).click()
  await expect(graph.getByRole('button', { name: '报告工作 E', exact: true })).toContainText('▶ running')
  await expect(details).toContainText('报告正文：结果在 result.txt，限制为只读')
  await expect(details.locator('time')).toHaveAttribute('datetime', '1970-01-01T00:00:05.000Z')
  await expect(details).toContainText('report-chat-offline')
  await page.getByRole('button', { name: '刷新 Run', exact: true }).click()
  await expect(page.getByTestId('run-user-input')).toHaveText('USER INPUT new-run')
  await page.waitForTimeout(750)
  expect(server.herdrCalls()).toEqual(calls)
  expect(['run.json', 'history.jsonl', 'messages.jsonl'].map(file => fs.readFileSync(path.join(root, 'new-run', file), 'utf8'))).toEqual(records)
  await page.screenshot({ path: info.outputPath('current-graph-details.png'), fullPage: true })
  fs.writeFileSync(info.outputPath('no-implicit-execution.json'), JSON.stringify({ observationMs: 750, recordsBefore: records, recordsAfter: ['run.json', 'history.jsonl', 'messages.jsonl'].map(file => fs.readFileSync(path.join(root, 'new-run', file), 'utf8')), callsBefore: calls, callsAfter: server.herdrCalls() }, null, 2))
})

// P57.01 source: ticket's three unavailable-definition conditions. Violation:
// reconstructing a graph from saved statuses or attaching old nodes to a renamed
// current node. Start with a real current graph, then change the owned file and
// manually read again. Reports and retained identities must remain accessible.
for (const condition of ['missing', 'invalid', 'mismatch'] as const) {
  test(`P57.01 ${condition} current definition never fabricates a historical graph @time-contract`, async ({ page }, info) => {
    const { server, root, mapPath, definition, run, runPath, report } = await currentFixture()
    run.nodes.E.previousExecutions = [{ execution: { agent: 'omp', id: 'old-exec-E', taskId: 'old-saved-task', zone: 'C:/old-zone', chat: { ...chat, sessionId: 'saved-old-chat' } }, failure: { stage: 'execution', message: '旧执行已保留的失败' } }]
    fs.writeFileSync(runPath, JSON.stringify(run))
    fs.writeFileSync(path.join(root, 'new-run/messages.jsonl'), JSON.stringify({ ...report, id: 'report-old', executionId: 'old-exec-E', body: '旧执行报告正文仍可读取', source: run.nodes.E.previousExecutions[0].execution.chat, at: 4500 }) + '\n' + JSON.stringify(report) + '\n')
    const records = ['run.json', 'history.jsonl', 'messages.jsonl'].map(file => fs.readFileSync(path.join(root, 'new-run', file), 'utf8'))
    const calls = server.herdrCalls()
    await page.goto(`${server.baseURL}/#workflow/new-run`)
    await expect(page.getByRole('group', { name: 'Map 依赖图', exact: true })).toBeVisible()
    if (condition === 'missing') fs.unlinkSync(mapPath)
    else if (condition === 'invalid') fs.writeFileSync(mapPath, '{ broken current Map')
    else fs.writeFileSync(mapPath, JSON.stringify({ ...definition, nodes: { Z: { ...definition.nodes.A, title: '不应关联的改名工作' } } }))
    fs.unlinkSync(path.join(server.cwd, '.data/bots/worker.json'))
    await page.getByRole('button', { name: '刷新 Run', exact: true }).click()
    await expect(page.getByRole('alert').filter({ hasText: '不补造历史图' })).toBeVisible()
    await expect(page.getByRole('group', { name: 'Map 依赖图', exact: true })).toHaveCount(0)
    await expect(page.getByTestId('run-user-input')).toHaveText('USER INPUT new-run')
    await expect(page.getByText('saved-manager-chat', { exact: true })).toBeVisible()
    await page.getByRole('button', { name: 'B：failed', exact: true }).click()
    const details = page.getByRole('region', { name: '当前节点详情' })
    await expect(details.getByRole('heading', { name: 'B', exact: true })).toBeVisible()
    await expect(details).toContainText('deleted-task')
    await expect(details).toContainText('缺少已核实的源文件')
    await expect(details).toContainText('failed-chat')
    await page.getByRole('button', { name: 'E：running', exact: true }).click()
    await expect(details).toContainText('报告正文：结果在 result.txt，限制为只读')
    await expect(details).toContainText('旧执行报告正文仍可读取')
    await expect(details).toContainText('saved-old-chat')
    await expect(details).toContainText('old-saved-task')
    await expect(details).toContainText('C:/old-zone')
    await expect(page.getByText('不应关联的改名工作', { exact: true })).toHaveCount(0)
    await page.waitForTimeout(750)
    expect(server.herdrCalls()).toEqual(calls)
    expect(['run.json', 'history.jsonl', 'messages.jsonl'].map(file => fs.readFileSync(path.join(root, 'new-run', file), 'utf8'))).toEqual(records)
    await page.screenshot({ path: info.outputPath(`fallback-${condition}.png`), fullPage: true })
    fs.writeFileSync(info.outputPath('fallback-evidence.json'), JSON.stringify({ condition, recordsBefore: records, recordsAfter: ['run.json', 'history.jsonl', 'messages.jsonl'].map(file => fs.readFileSync(path.join(root, 'new-run', file), 'utf8')), callsBefore: calls, callsAfter: server.herdrCalls(), observationMs: 750 }, null, 2))
  })
}

// Approved seams S1 delivered browser page, S2 real HTTP, S4 isolated record files.
// P50.01 source: ticket's literal multi-Map/time/state example above. Violation:
// showing only selected/current Map, sorting by ID or omitting old Runs. Reading
// the visible table (not a helper) must return all three independently ordered rows.
test('P50.01 workflow lists every Run newest first with completion counts and ownership', async ({ page }, info) => {
  const { server } = await fixture()
  await page.goto(server.baseURL)
  await page.getByRole('tab', { name: 'workflow', exact: true }).click()
  const rows = page.getByRole('table', { name: '全部 Run' }).getByRole('row')
  await expect(rows).toHaveCount(4)
  await expect(rows.nth(1)).toContainText('new-run')
  await expect(rows.nth(1)).toContainText('beta.json')
  await expect(rows.nth(1)).toContainText('paused')
  await expect(rows.nth(1)).toContainText('1 / 3')
  await expect(rows.nth(1)).toContainText('未接管')
  await expect(rows.nth(2)).toContainText('middle-run')
  await expect(rows.nth(2)).toContainText('0 / 1')
  await expect(rows.nth(3)).toContainText('old-run')
  await expect(rows.nth(3)).toContainText('1 / 1')
  await expect(rows.nth(1).locator('time')).toHaveAttribute('datetime', '1970-01-01T00:00:03.000Z')
  await expect(page.getByRole('searchbox')).toHaveCount(0)
  await expect(page.getByRole('combobox')).toHaveCount(0)
  await page.getByRole('link', { name: 'new-run', exact: true }).click()
  await expect(page.getByTestId('run-user-input')).toHaveText('USER INPUT new-run')
  await page.screenshot({ path: info.outputPath('workflow-list.png'), fullPage: true })
  await page.setViewportSize({ width: 390, height: 844 })
  await expectNoHorizontalOverflow(page)
  await expect(page.getByTestId('run-user-input')).toBeVisible()
  await page.screenshot({ path: info.outputPath('workflow-list-mobile.png'), fullPage: true })
})
