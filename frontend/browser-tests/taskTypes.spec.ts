import fs from 'node:fs'
import path from 'node:path'
import { expect, test } from '@playwright/test'
import { startServer, type E2EServer } from './support'

let server: E2EServer | null = null
test.afterEach(async () => { await server?.cleanup(); server = null })

// P5.01 / S1,S2: changing Task purpose preserves its host without repeating the global extension catalog.
test('P5.01 Pi Task types preserve agent without repeating global extensions', async ({ page }, info) => {
  server = await startServer()
  await page.goto(`${server.baseURL}?layout=expanded`)
  await page.getByTestId('tab-tasks').click()
  await page.getByTestId('task-create-open').click()
  await expect(page.getByTestId('task-type')).toHaveValue('normal')
  expect(await page.getByTestId('task-type').locator('option').evaluateAll(options => options.map(option => (option as HTMLOptionElement).value))).toEqual(['normal', 'manager'])
  await page.getByTestId('task-name').fill('类型切换')
  await page.getByTestId('task-agent').selectOption('pi')
  await page.getByTestId('task-type').selectOption('manager')
  await expect(page.getByTestId('task-agent')).toHaveValue('pi')
  await expect(page.getByTestId('task-agent').locator('option[value=pi]')).toBeEnabled()
  await page.getByTestId('task-next').click()
  await expect(page.getByTestId('task-fixed-extensions')).toHaveCount(0)
  await expect(page.getByTestId('task-suite')).toHaveValue('')
  await expect(page.getByTestId('extension-pi-manager-chat')).toHaveCount(0)
  await expect(page.getByTestId('extension-pi-node-chat')).toHaveCount(0)
  await page.getByTestId('task-next').click()
  await page.getByTestId('save-task').click()
  await expect(page.getByTestId('task-dialog')).toBeHidden()
  let saved = (await server.getTasks()).tasks[0]
  expect(saved).toMatchObject({ taskType: 'manager', agent: 'pi' })
  await expect(page.getByTestId('task-item')).not.toContainText('manager-chat')
  await expect(page.getByTestId('task-item')).not.toContainText('node-chat')
  for (const role of ['normal']) {
    await page.getByTestId('task-edit').click()
    await page.getByTestId('task-type').selectOption(role)
    await expect(page.getByTestId('task-agent')).toHaveValue('pi')
    await expect(page.getByTestId('task-agent').locator('option[value=pi]')).toBeEnabled()
    await page.getByTestId('task-next').click()
    await expect(page.getByTestId('task-fixed-extensions')).toHaveCount(0)
    await expect(page.getByTestId('task-suite')).toHaveValue('')
    await page.getByTestId('task-next').click()
    await page.getByTestId('save-task').click()
    await expect(page.getByTestId('task-dialog')).toBeHidden()
    saved = (await server.getTasks()).tasks[0]
    expect(saved).toMatchObject({ taskType: role, agent: 'pi' })
  }
  await page.screenshot({ path: info.outputPath('task-types.png'), fullPage: true })
  fs.writeFileSync(info.outputPath('task-types.json'), JSON.stringify(await server.getTasks(), null, 2))
})

// P5.01: observe effective Task environment at the existing HTTP launch boundary.
// Controlled external calls prove assembly only, not role tools/Prompt execution.
test('P5.01 role changes select exactly one fixed role environment', async ({}, info) => {
  server = await startServer({ herdr: {} })
  const root = path.dirname(server.configPath)
  const zonePath = path.join(root, 'role-zone')
  fs.mkdirSync(zonePath)
  const zoneResponse = await fetch(`${server.baseURL}/api/zones`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ id: 'test/zone', path: zonePath }) })
  const zone = (await zoneResponse.json()).zones[0].id
  let id: string | undefined
  for (const role of ['manager', 'normal']) {
    const response = await fetch(`${server.baseURL}/api/tasks${id ? `/${id}` : ''}`, { method: id ? 'PUT' : 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ name: '角色', chatName: '{chat.id}', taskType: role, agent: 'omp', model: 'default', thinking: 'default', skill: null, prompt: '' }) })
    expect(response.status).toBe(200)
    id = (await response.json()).tasks[0].id
    const preview = await (await fetch(`${server.baseURL}/api/chats/preview`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ taskId: id, zone }) })).json()
    const created = await fetch(`${server.baseURL}/api/chats`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ taskId: id, zone, chatId: preview.chatId, input: '检查角色环境装配' }) })
    expect(created.status).toBe(202)
    const job = await created.json()
    await expect.poll(async () => (await (await fetch(`${server!.baseURL}/api/chats/${job.id}`)).json()).stage).toBe('succeeded')
    const entries = role === 'manager' ? ['omp-herdr-agent-state', 'omp-model-cost', 'omp-manager-chat', 'omp-connect-bot']
      : ['omp-herdr-agent-state', 'omp-model-cost', 'omp-connect-bot']
    expect(server.herdrCalls().filter(call => call.kind === 'agent_start').at(-1)!.args).toEqual(['--no-extensions', ...entries.flatMap(entry => ['--extension', path.join(root, 'extensions', `${entry}.ts`)])])
  }
  fs.writeFileSync(info.outputPath('role-environments-NOT-native-loading.json'), JSON.stringify({ calls: server.herdrCalls(), tasks: await server.getTasks() }, null, 2))
})

// One persisted normal Task serves both entry points; only actual workflow dispatch
// adds the node role. The controlled host records assembly, not native loading.
for (const agent of ['omp', 'pi'] as const) {
  test(`${agent} normal Task starts independently and through Bot with context-only node role`, async ({ page }, info) => {
    server = await startServer({ separateCwd: true, herdr: { workspaces: [{ workspace_id: 'shared-workspace', label: 'process-cwd', cwd: 'C:/unused-default' }, { workspace_id: 'standalone-workspace', label: 'bot-playground', cwd: 'C:/unused-default' }] } })
    const host = server, root = path.dirname(host.configPath)
    const post = async (route: string, body: unknown, status = 200) => {
      const response = await fetch(`${host.baseURL}${route}`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify(body) })
      expect(response.status, await response.clone().text()).toBe(status)
      return response.json()
    }
    const zone = (await post('/api/zones', { id: 'test/zone', path: host.cwd })).zones[0].id
    const base = { agent, chatName: '{chat.id}', model: 'default', thinking: 'default', skill: null, prompt: 'SHARED TASK PROMPT' }
    const task = (await post('/api/tasks', { ...base, name: 'Shared normal', taskType: 'normal' })).tasks[0]
    const manager = (await post('/api/tasks', { ...base, name: 'Manager', taskType: 'manager' })).tasks.find((item: { name: string }) => item.name === 'Manager')
    const identity = (id: string) => host.setHerdrBehavior({ native_session: { agent, source: `herdr:${agent}`, kind: 'id', value: id } })
    identity('standalone-normal')
    const preview = await post('/api/chats/preview', { taskId: task.id, zone })
    const chat = await post('/api/chats', { taskId: task.id, zone, chatId: preview.chatId, input: 'STANDALONE INPUT' }, 202)
    await expect.poll(async () => (await (await fetch(`${host.baseURL}/api/chats/${chat.id}`)).json()).stage).toBe('succeeded')
    const nativeBuiltins = agent === 'pi' ? ['--extension', 'builtin:mcp', '--extension', 'builtin:tool-search', '--extension', 'builtin:codemode'] : []
    const entries = [`${agent}-herdr-agent-state`, `${agent}-model-cost`, ...(agent === 'pi' ? ['pi-suite'] : []), `${agent}-connect-bot`]
    expect(host.herdrCalls().filter(call => call.kind === 'agent_start').at(-1)!.args).toEqual(['--no-extensions', ...nativeBuiltins, ...entries.flatMap(id => ['--extension', path.join(root, 'extensions', `${id}.ts`)])])
    const maps = path.join(host.cwd, 'meta-data/workflow-maps'), bots = path.join(host.cwd, 'meta-data/bots')
    fs.mkdirSync(maps, { recursive: true }); fs.mkdirSync(bots, { recursive: true })
    fs.writeFileSync(path.join(maps, 'shared.json'), JSON.stringify({ title: 'Shared Task', description: '', nodes: { A: { title: 'Shared node', bot_id: 'worker', input: 'WORKFLOW INPUT', zone, blocked_by: [] } } }))
    fs.writeFileSync(path.join(bots, 'worker.json'), JSON.stringify({ id: 'worker', name: 'Worker', when: '', plan_guide: '', node_guide: '', task_id: task.id }))
    identity('shared-manager')
    const run = await post('/api/scheduler/maps/shared.json/runs', { managerTaskId: manager.id, zone, userInput: 'MANAGER INPUT' }, 201)
    await post('/api/workflow/manager/prepare_inputs', { sessionId: 'shared-manager', fileInputs: [], confirmed: true, reason: 'Reviewed shared task fixture inputs' })
    identity('shared-node')
    expect((await post('/api/workflow/manager/dispatch', { sessionId: 'shared-manager', nodes: ['A'] })).results[0].status).toBe('running')
    expect(host.herdrCalls().filter(call => call.kind === 'agent_start').at(-1)!.args).toEqual(['--no-extensions', ...nativeBuiltins, ...[...entries.slice(0, -1), `${agent}-node-chat`, `${agent}-connect-bot`].flatMap(id => ['--extension', path.join(root, 'extensions', `${id}.ts`)])])
    expect((await host.getTasks()).tasks.find(item => item.id === task.id)).toEqual(task)
    const history = JSON.parse(fs.readFileSync(path.join(root, 'session-history.json'), 'utf8'))
    expect(history.filter((item: { sourceTaskId: string }) => item.sourceTaskId === task.id)).toHaveLength(2)
    await page.goto(`${host.baseURL}/#workflow/${run.id}`)
    await expect(page.getByRole('region', { name: '当前执行 Chat', exact: true })).toContainText('shared-node')
    const evidence = info.outputPath(`${agent}-shared-normal-contexts.json`)
    fs.writeFileSync(evidence, JSON.stringify({ task, history, calls: host.herdrCalls() }, null, 2))
    await info.attach('shared-normal-contexts', { path: evidence, contentType: 'application/json' })
  })
}


