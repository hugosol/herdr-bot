import fs from 'node:fs'
import path from 'node:path'
import { expect, test } from '@playwright/test'
import { startServer, type E2EServer } from './support'

let server: E2EServer | null = null
test.afterEach(async () => { await server?.cleanup(); server = null })
const ompSuite = { id: 'shared', name: '共享 OMP 工具', agents: ['omp'], mcp: ['search'] }
const piSuite = { id: 'pi-only', name: 'Pi 工具', agents: ['pi'], pi: { codemode: 'on' } }
function definitions(server: E2EServer) {
  const directory = path.join(path.dirname(server.configPath), 'meta-data')
  fs.mkdirSync(directory, { recursive: true })
  fs.writeFileSync(path.join(directory, 'suites.json'), JSON.stringify({ suites: [ompSuite, piSuite] }))
  fs.writeFileSync(path.join(directory, 'mcp.json'), '{"mcpServers":{"search":{"command":"controlled-search"}}}')
  return directory
}

// S1/P1: independent literal selectable IDs. A multiple selector, missing empty
// option, incompatible option or silently reset reference must fail this trace.
test('P1 browser offers only one compatible Suite or none and never silently changes it', async ({ page }) => {
  server = await startServer()
  definitions(server)
  await page.goto(`${server.baseURL}?layout=expanded`)
  await page.getByTestId('tab-tasks').click()
  await page.getByTestId('task-create-open').click()
  await page.getByTestId('task-name').fill('Browser source')
  await page.getByTestId('task-next').click()
  const choice = page.getByTestId('task-suite')
  await expect(choice).toBeVisible()
  expect(await choice.evaluate(element => (element as HTMLSelectElement).multiple)).toBe(false)
  expect(await choice.locator('option:not([disabled])').evaluateAll(options => options.map(option => (option as HTMLOptionElement).value))).toEqual(['', 'shared'])
  await choice.selectOption('shared')
  await page.getByTestId('task-next').click()
  await expect(page.getByTestId('summary-suite')).toContainText('共享 OMP 工具')
  await page.getByTestId('save-task').click()
  await expect(page.getByTestId('task-dialog')).toBeHidden()
  const { tasks: saved } = await server.getTasks()
  expect(saved).toHaveLength(1)
  expect(saved[0]).toMatchObject({ name: 'Browser source', suite: 'shared' })
  const id = saved[0].id
  await page.getByTestId('task-edit').click()
  await page.getByTestId('task-agent').selectOption('pi')
  await page.getByTestId('task-next').click()
  await expect(choice).toHaveValue('shared')
  await expect(page.getByTestId('task-suite-issue')).toContainText('不适用于 pi')
  expect(await choice.locator('option:not([disabled])').evaluateAll(options => options.map(option => (option as HTMLOptionElement).value))).toEqual(['', 'pi-only'])
  await choice.selectOption('')
  await page.getByTestId('task-next').click()
  await page.getByTestId('save-task').click()
  await expect(page.getByTestId('task-dialog')).toBeHidden()
  const read = await page.request.get(`${server.baseURL}/api/tasks/${id}`)
  expect(read.status()).toBe(200)
  expect((await read.json()).task.suite ?? null).toBeNull()
})

// S1/P33: the browser must surface the server's real refusal and every Task,
// not optimistically hide the Suite or remove any reference from the list.
test('P33 browser refuses deletion and lists all referencing Tasks without changing them', async ({ page }) => {
  server = await startServer()
  definitions(server)
  for (const name of ['First browser reference', 'Second browser reference']) {
    const response = await page.request.post(`${server.baseURL}/api/tasks`, { data: { name, chatName: '{task.name}', agent: 'omp', model: 'default', thinking: 'default', skill: null, prompt: '', extensions: [], suite: 'shared' } })
    expect(response.status()).toBe(200)
  }
  const { tasks: before } = await server.getTasks()
  await page.goto(`${server.baseURL}?layout=expanded`)
  await page.getByTestId('tab-suite').click()
  await page.getByRole('button', { name: '共享 OMP 工具 · shared', exact: true }).click()
  page.once('dialog', dialog => dialog.accept())
  await page.getByRole('button', { name: '删除 Suite', exact: true }).click()
  const failure = page.getByRole('alert')
  await expect(failure).toContainText('不能删除')
  for (const task of before) {
    await expect(failure).toContainText(task.name)
    await expect(failure).toContainText(task.id)
    const read = await page.request.get(`${server.baseURL}/api/tasks/${task.id}`)
    expect(read.status()).toBe(200)
    expect((await read.json()).task).toEqual(task)
  }
  await expect(page.getByRole('button', { name: '共享 OMP 工具 · shared', exact: true })).toBeVisible()
  const suite = await page.request.get(`${server.baseURL}/api/suites/shared`)
  expect(suite.status()).toBe(200)
  expect((await suite.json()).suite).toMatchObject(ompSuite)
})

// S1/P18: after an independent file deletion, both reading surfaces must show
// the retained missing ID. Hiding it while saving an empty list is a violation.
test('P18 browser retains and marks missing central MCP references and refuses saving them', async ({ page }) => {
  server = await startServer()
  const directory = definitions(server)
  const input = { name: '失效 MCP 来源', chatName: '{task.name}', agent: 'omp', model: 'default', thinking: 'default', skill: null, prompt: 'Keep trigger', extensions: [], suite: 'shared' }
  const created = await page.request.post(`${server.baseURL}/api/tasks`, { data: input })
  expect(created.status()).toBe(200)
  const saved = (await created.json()).tasks[0]
  const authority = fs.readFileSync(path.join(directory, 'suites.json'), 'utf8')
  await page.goto(`${server.baseURL}?layout=expanded`)
  await page.getByTestId('tab-suite').click()
  await page.getByRole('button', { name: '共享 OMP 工具 · shared', exact: true }).click()
  await expect(page.getByLabel('search', { exact: true })).toBeChecked()
  fs.writeFileSync(path.join(directory, 'mcp.json'), '{"mcpServers":{}}')
  await page.getByRole('button', { name: '重新读取 Suite', exact: true }).click()
  const missing = page.getByLabel('search（失效：集中 MCP 定义不存在）', { exact: true })
  await expect(missing).toBeChecked()
  await page.getByRole('button', { name: '保存 Suite', exact: true }).click()
  await expect(page.getByRole('alert')).toContainText('search')
  expect(fs.readFileSync(path.join(directory, 'suites.json'), 'utf8')).toBe(authority)
  await page.getByTestId('tab-tasks').click()
  await expect(page.getByTestId('task-item')).toContainText('集中 MCP 引用失效：search')
  await page.getByTestId('task-edit').click()
  await page.getByTestId('task-next').click()
  await expect(page.getByTestId('task-suite')).toHaveValue('shared')
  await expect(page.getByTestId('task-suite-issue')).toContainText('search')
  await page.getByTestId('task-next').click()
  await page.getByTestId('save-task').click()
  await expect(page.getByTestId('task-form-error')).toContainText('search')
  const read = await page.request.get(`${server.baseURL}/api/tasks/${saved.id}`)
  expect(read.status()).toBe(200)
  expect((await read.json()).task).toEqual(saved)
  expect(fs.readFileSync(path.join(directory, 'suites.json'), 'utf8')).toBe(authority)
})
