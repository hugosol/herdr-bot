import fs from 'node:fs'
import path from 'node:path'
import { expect, test, type Page, type TestInfo } from '@playwright/test'
import { startServer, type E2EServer } from '../support/support'

let server: E2EServer | null = null
const manager = 'end-node-manager'
test.afterEach(async () => { await server?.cleanup(); server = null })
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
async function screenshot(page: Page, info: TestInfo, name: string) {
  const image = await page.screenshot({ path: info.outputPath(`${name}.png`), fullPage: true })
  await info.attach(name, { body: image, contentType: 'image/png' })
}

// Delivered page + public HTTP + test-owned production Herdr recorder. No Run,
// execution or report is fabricated. This does not prove native model behavior.
test('end-node edits round-trip and dispatch hands an open Chat to the user without business completion', async ({ page }, info) => {
  server = await startServer({ separateCwd: true, herdr: { workspaces: [{ workspace_id: 'w-end-node', label: 'end-node-zone', focused: false }] } })
  const zone = path.join(server.cwd, 'end-node-zone')
  fs.mkdirSync(zone)
  fs.writeFileSync(path.join(zone, 'business.txt'), 'USER BUSINESS MATERIAL')
  await post(page, '/api/zones', { id: 'end/zone', path: zone })
  const tasks: Record<string, string> = {}
  for (const taskType of ['manager', 'normal']) {
    const result = await post(page, '/api/tasks', { name: taskType, taskType, agent: 'omp', chatName: '{chat.id}', model: 'default', thinking: 'default', prompt: 'ISOLATED END NODE ACCEPTANCE' })
    tasks[taskType] = result.tasks.find((task: { name: string }) => task.name === taskType).id
  }
  await post(page, '/api/scheduler/bots/save', { id: 'end-worker', revision: null, content: JSON.stringify({ name: 'End worker', when: '', plan_guide: '', node_guide: '', task_id: tasks.normal }) })
  const maps = path.join(server.cwd, 'meta-data/workflow-maps')
  fs.mkdirSync(maps, { recursive: true })
  const common = { output_rule: [], bot_id: 'end-worker', input: 'Preserve business input', zone: 'end/zone', blocked_by: [] }
  fs.writeFileSync(path.join(maps, 'end-node.json'), JSON.stringify({ title: 'End node handoff', description: '', nodes: {
    A: { ...common, title: 'Other work' },
    Z: { ...common, title: 'User conversation', auto_approved: false, auto_close: true, input_rule: [{ id: 'business', type: 'file', zone_id: 'end/zone', path: 'business.txt' }] },
  } }))
  await page.goto(`${server.baseURL}?layout=expanded`)
  await page.getByTestId('tab-scheduler').click()
  await page.locator('[data-map-file="end-node.json"]').click()
  await page.getByRole('region', { name: '节点列表', exact: true }).getByRole('button', { name: 'User conversation Z', exact: true }).click()
  await page.getByRole('button', { name: '修改 node', exact: true }).click()
  const end = page.getByRole('checkbox', { name: 'end_node', exact: true })
  await expect(end).toBeEnabled()
  await end.check()
  await expect(page.getByRole('checkbox', { name: 'auto_close', exact: true })).toHaveCount(0)
  await expect(page.getByRole('radio', { name: 'manual', exact: true })).toHaveCount(0)
  await screenshot(page, info, 'end-node-settings')
  await page.getByRole('button', { name: '保存 node', exact: true }).click()
  await expect(page.getByRole('form', { name: 'node 编辑', exact: true })).toHaveCount(0)
  const saved = await (await page.request.get(`${server.baseURL}/api/scheduler/maps/end-node.json`)).json()
  expect(saved.nodes.Z).toMatchObject({ end_node: true, auto_approved: false, auto_close: true, input: 'Preserve business input' })
  await page.getByRole('button', { name: '修改 node', exact: true }).click()
  await expect(end).toBeChecked()
  await end.uncheck()
  await expect(page.getByRole('checkbox', { name: 'auto_close', exact: true })).toBeChecked()
  await expect(page.getByRole('radio', { name: 'manual', exact: true })).toBeChecked()
  await page.getByRole('button', { name: '放弃编辑', exact: true }).click()

  server.setHerdrBehavior({ native_session: { source: 'herdr:omp', agent: 'omp', kind: 'id', value: manager }, native_cwd: zone })
  const started = await post(page, '/api/scheduler/maps/end-node.json/runs', { managerTaskId: tasks.manager, zone: 'end/zone', userInput: 'Handoff only after all other work' }, 201)
  await post(page, '/api/workflow/manager/prepare_inputs', { sessionId: manager, fileInputs: [], confirmed: true, reason: 'Reviewed fixture inputs' })
  await page.goto(`${server.baseURL}/#workflow/${started.id}`)
  const graph = page.getByRole('group', { name: 'Map 依赖图', exact: true })
  await expect(graph.locator('[data-node-id="Z"]')).toHaveAttribute('data-node-state', 'blocked')
  server.setHerdrBehavior({ native_session: { source: 'herdr:omp', agent: 'omp', kind: 'id', value: 'end-node-other' }, native_cwd: zone })
  await post(page, '/api/workflow/manager/dispatch', { sessionId: manager, nodes: ['A'] })
  await post(page, '/api/workflow/node/report', { sessionId: 'end-node-other', body: 'Other work ready', canComplete: true })
  const context = await post(page, '/api/workflow/node/context', { sessionId: 'end-node-other' })
  await post(page, '/api/workflow/node/approve', { sessionId: 'end-node-other', executionId: context.executionId, source: 'auto' })
  await post(page, '/api/workflow/manager/complete', { sessionId: manager, nodeId: 'A', reason: 'Other work accepted' })
  server.setHerdrBehavior({ native_session: { source: 'herdr:omp', agent: 'omp', kind: 'id', value: 'end-node-chat' }, native_cwd: zone })
  const dispatched = await post(page, '/api/workflow/manager/dispatch', { sessionId: manager, nodes: ['Z'] })
  expect(dispatched.results[0].status).toBe('running')
  const launched = await read(page, started.id)
  expect(launched.run.status).toBe('running')
  expect(launched.run.nodes.Z).toMatchObject({ status: 'running', completion: null, execution: { endNode: true, autoClose: false, approved: false, approvalSource: null, chat: { sessionId: 'end-node-chat' }, inputs: [{ id: 'business', path: path.join(zone, 'business.txt') }] } })
  expect(launched.reports.filter((report: { nodeId: string }) => report.nodeId === 'Z')).toEqual([])
  await page.getByRole('button', { name: '刷新 Run', exact: true }).click()
  await graph.locator('[data-node-id="Z"]').click()
  await expect(graph.locator('[data-node-id="Z"]')).toHaveAttribute('data-node-state', 'handed-to-user')
  const details = page.getByRole('region', { name: '当前节点详情', exact: true })
  await expect(details).toContainText('不是业务已完成')
  await expect(details).toContainText('不适用（结束节点）')
  await expect(details.locator('dt').filter({ hasText: /^approved$|^approvalSource$/ })).toHaveCount(0)
  await expect(page.getByRole('region', { name: '当前执行交接事实', exact: true })).toHaveCount(0)
  await screenshot(page, info, 'end-node-handed-to-user')
  await post(page, '/api/workflow/manager/finish', { sessionId: manager })
  const finished = await read(page, started.id)
  expect(finished.run).toMatchObject({ status: 'completed', finished: { reason: 'handed-to-user' }, nodes: { Z: { status: 'running', completion: null } } })
  await page.getByRole('button', { name: '刷新 Run', exact: true }).click()
  await expect(page.getByText(/Run 结束原因/)).toContainText('handed-to-user')
  await screenshot(page, info, 'end-node-run-finished')
  const resumed = await post(page, '/api/chats/resume', { sessionId: 'end-node-chat' })
  expect(resumed).toBeTruthy()
  expect(server.herdrCalls().filter(call => call.kind === 'tab_close')).toEqual([])
  const evidence = JSON.stringify({ saved, launched, finished, calls: server.herdrCalls() }, null, 2)
  fs.writeFileSync(info.outputPath('end-node-public-facts.json'), evidence)
  await info.attach('end-node-public-facts', { body: evidence, contentType: 'application/json' })
})
