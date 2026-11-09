import fs from 'node:fs'
import path from 'node:path'
import { expect, test } from '@playwright/test'
import { startServer, type E2EServer } from './support'

let server: E2EServer | null = null
test.afterEach(async () => { await server?.cleanup(); server = null })

// S1/S2 P04/P06/P10/P11: launch through the delivered form, dispatch over
// public HTTP, read both Runs and actual external effects. A JSON-only child
// with no usable page or a required-interface bypass cannot satisfy this case.
test('nested dispatch launches node-local manager and exposes success and required-input refusal', async ({ page }) => {
  server = await startServer({ separateCwd: true, herdr: {} })
  const rootZone = path.join(path.dirname(server.configPath), 'bot-playground')
  const childZone = path.join(path.dirname(server.configPath), 'child-place')
  fs.mkdirSync(rootZone); fs.mkdirSync(childZone)
  for (const [id, directory] of [['root-zone', rootZone], ['child-zone', childZone]]) {
    expect((await page.request.post(`${server.baseURL}/api/zones`, { data: { id, path: directory } })).ok()).toBe(true)
  }
  const tasks: Record<string, string> = {}
  for (const [name, taskType] of [['Outer manager', 'manager'], ['Child manager', 'manager'], ['Worker', 'normal']]) {
    const response = await page.request.post(`${server.baseURL}/api/tasks`, { data: { name, taskType, agent: 'omp', chatName: '{chat.id}', model: 'default', thinking: 'default', prompt: `FIXED ${name}` } })
    expect(response.ok()).toBe(true)
    tasks[name] = (await response.json()).tasks.find((task: { name: string }) => task.name === name).id
  }
  const maps = path.join(server.cwd, 'meta-data/workflow-maps'), bots = path.join(server.cwd, 'meta-data/bots')
  fs.mkdirSync(maps, { recursive: true }); fs.mkdirSync(bots, { recursive: true })
  fs.writeFileSync(path.join(bots, 'nested.json'), JSON.stringify({ id: 'nested', name: 'Nested', when: '', plan_guide: '', node_guide: '', map_file: 'inner.json' }))
  fs.writeFileSync(path.join(bots, 'worker.json'), JSON.stringify({ id: 'worker', name: 'Worker', when: '', plan_guide: '', node_guide: '', task_id: tasks.Worker }))
  const inner = { title: 'Inner dispatch map', description: '', input_files: [{ id: 'material', required: false }], nodes: { A: { title: 'Internal work', bot_id: 'worker', input: 'INNER', zone: 'root-zone', blocked_by: [] } } }
  fs.writeFileSync(path.join(maps, 'inner.json'), JSON.stringify(inner))
  const call = { title: 'Nested call', bot_id: 'nested', manager_task_id: tasks['Child manager'], input: 'CHILD 原始业务\nNo production access', zone: 'child-zone', blocked_by: [] }
  fs.writeFileSync(path.join(maps, 'outer.json'), JSON.stringify({ title: 'Outer dispatch map', description: '', nodes: { A: call, B: { ...call, title: 'Required call' } } }))
  server.setHerdrBehavior({ native_session: { source: 'herdr:omp', agent: 'omp', kind: 'id', value: 'page-root' }, native_cwd: rootZone })
  await page.goto(`${server.baseURL}?layout=expanded`)
  await page.getByTestId('tab-scheduler').click()
  await page.getByRole('button', { name: 'Outer dispatch map', exact: true }).click()
  await page.getByRole('button', { name: '+ 发起 Run', exact: true }).click()
  await page.getByRole('combobox', { name: '管理者 Task' }).selectOption(tasks['Outer manager'])
  await page.getByRole('combobox', { name: '管理者 zone' }).selectOption('root-zone')
  await page.getByRole('textbox', { name: '本次要求', exact: true }).fill('ROOT independent background')
  await page.getByRole('button', { name: '启动 Run', exact: true }).click()
  await expect(page.getByRole('heading', { name: 'Run 详情', exact: true })).toBeVisible()
  const rootId = page.url().split('#workflow/')[1]
  server.setHerdrBehavior({ native_session: { source: 'herdr:omp', agent: 'omp', kind: 'id', value: 'page-child' }, native_cwd: childZone })
  const dispatch = await page.request.post(`${server.baseURL}/api/workflow/manager/dispatch`, { data: { sessionId: 'page-root', nodes: ['A'] } })
  expect(dispatch.status()).toBe(200)
  const result = await dispatch.json()
  expect(result.results[0].status).toBe('running')
  const execution = result.snapshot.nodes.A.execution
  const child = await (await page.request.get(`${server.baseURL}/api/workflow/runs/${execution.childRunId}`)).json()
  expect(child.run.parent).toEqual({ runId: rootId, nodeId: 'A', executionId: execution.id })
  expect(child.run.managerTaskId).toBe(tasks['Child manager'])
  expect(child.run.zone).toBe(childZone)
  expect(child.run.userInput).toBe('CHILD 原始业务\nNo production access')
  expect(child.run.fileInputs).toEqual([])
  await page.getByRole('button', { name: '刷新 Run', exact: true }).click()
  await expect(page.getByRole('region', { name: '当前节点详情', exact: true })).toContainText('page-child')
  await expect(page.getByRole('region', { name: '当前节点详情', exact: true })).toContainText('Child manager')
  inner.input_files[0].required = true
  fs.writeFileSync(path.join(maps, 'inner.json'), JSON.stringify(inner))
  const before = server.herdrCalls()
  const rejected = await (await page.request.post(`${server.baseURL}/api/workflow/manager/dispatch`, { data: { sessionId: 'page-root', nodes: ['B'] } })).json()
  expect(rejected.results[0].status).toBe('failed')
  expect(rejected.results[0].failure.message).toContain('material')
  expect(rejected.snapshot.nodes.B.execution.childRunId).toBeUndefined()
  expect(server.herdrCalls()).toEqual(before)
  await page.getByRole('button', { name: '刷新 Run', exact: true }).click()
  await page.getByRole('button', { name: /Required call/ }).first().click()
  await expect(page.getByRole('region', { name: '当前节点详情', exact: true })).toContainText('material')
  const calls = server.herdrCalls()
  expect(calls.filter(call => call.kind === 'agent_start')).toHaveLength(2)
  expect(calls.filter(call => call.kind === 'agent_prompt').some(call => String(call.text).includes('CHILD 原始业务\nNo production access'))).toBe(true)
  const stored = JSON.parse(fs.readFileSync(path.join(server.cwd, '.data/workflow-runs', execution.childRunId, 'run.json'), 'utf8'))
  expect(stored).toEqual(child.run)
})
