import { execFileSync } from 'node:child_process'
import fs from 'node:fs'
import path from 'node:path'
import { expect, test } from '@playwright/test'
import { startServer, type E2EServer } from '../support/support'
import { zoneRowFor } from '../support/ui'

let server: E2EServer | null = null
test.afterEach(async () => { await server?.cleanup(); server = null })
const ompSuite = { id: 'shared-omp', name: '共享工具 · OMP', harness: 'omp', skills: [], extensions: [], mcp: ['search'], lsp: {} }
const piSuite = { id: 'shared-pi', name: '共享工具 · Pi', harness: 'pi', skills: [], extensions: [], mcp: [], prompts: [], codemode: 'on' }
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
  expect(await choice.locator('option:not([disabled])').evaluateAll(options => options.map(option => (option as HTMLOptionElement).value))).toEqual(['', 'shared-omp'])
  await choice.selectOption('shared-omp')
  await page.getByTestId('task-next').click()
  await expect(page.getByTestId('summary-suite')).toHaveText('共享工具 · OMP · shared-omp')
  await page.getByTestId('save-task').click()
  await expect(page.getByTestId('task-dialog')).toBeHidden()
  const { tasks: saved } = await server.getTasks()
  expect(saved).toHaveLength(1)
  expect(saved[0]).toMatchObject({ name: 'Browser source', suite: 'shared-omp' })
  const taskURL = `${server.baseURL}/api/tasks/${saved[0].id}`
  await page.getByTestId('task-edit').click()
  await page.getByTestId('task-agent').selectOption('pi')
  await page.getByTestId('task-next').click()
  await expect(choice).toHaveValue('shared-omp')
  await expect(choice.locator('option:checked')).toHaveText('共享工具 · OMP · shared-omp（不可用，保留引用）')
  await expect(page.getByTestId('task-suite-issue')).toContainText('不适用于 pi')
  expect(await choice.locator('option:not([disabled])').evaluateAll(options => options.map(option => (option as HTMLOptionElement).value))).toEqual(['', 'shared-pi'])
  await page.getByTestId('task-next').click()
  await expect(page.getByTestId('summary-suite')).toHaveText('共享工具 · OMP · shared-omp')
  await expect(page.getByTestId('save-task')).toBeDisabled()
  expect((await (await page.request.get(taskURL)).json()).task).toEqual(saved[0])
  await page.getByTestId('task-back').click()
  await expect(choice).toHaveValue('shared-omp')
  await choice.selectOption('shared-pi')
  await page.getByTestId('task-next').click()
  await expect(page.getByTestId('save-task')).toBeEnabled()
  await page.getByTestId('save-task').click()
  await expect(page.getByTestId('task-dialog')).toBeHidden()
  expect((await (await page.request.get(taskURL)).json()).task).toMatchObject({ agent: 'pi', suite: 'shared-pi' })

  await page.getByTestId('task-edit').click()
  await page.getByTestId('task-agent').selectOption('omp')
  await page.getByTestId('task-next').click()
  await expect(choice).toHaveValue('shared-pi')
  await choice.selectOption('')
  await page.getByTestId('task-next').click()
  await page.getByTestId('save-task').click()
  await expect(page.getByTestId('task-dialog')).toBeHidden()
  const read = await page.request.get(taskURL)
  expect(read.status()).toBe(200)
  const { task } = await read.json()
  expect(task.agent).toBe('omp')
  expect(task.suite ?? null).toBeNull()
})

// pstack P1, approved S2: the built Task/zone page, public HTTP and the
// external Herdr CLI. The shipped authority must offer pstack to OMP, not Pi.
// A save-only compatibility check would let the same preview start a Pi Chat
// after an offline Task edit; compare actual CLI calls and created Tabs.
test('pstack P1: only OMP can select, save and launch the shipped pstack Suite', async ({ page }, info) => {
  server = await startServer({ herdr: {} })
  const root = path.dirname(server.configPath)
  const definitions = path.join(root, 'meta-data')
  fs.mkdirSync(definitions)
  for (const name of ['suites.json', 'mcp.json']) {
    fs.copyFileSync(new URL(`../../../meta-data/${name}`, import.meta.url), path.join(definitions, name))
  }
  // Isolate the CLI boundary: P1 exercises host compatibility, not native
  // consumption of the delivered toolkit (which has separate acceptance).
  const toolkit = path.join(root, 'toolkit')
  const suite = path.join(toolkit, 'pstack-suite')
  fs.mkdirSync(path.join(suite, 'generated', 'skills'), { recursive: true })
  fs.writeFileSync(path.join(suite, 'omp.ts'), '// Controlled CLI does not execute extensions.\n')
  fs.appendFileSync(path.join(root, 'paths.toml'), `toolkit = '${toolkit}'\n`)
  const zonePath = path.join(root, 'pstack-zone')
  fs.mkdirSync(zonePath)
  execFileSync('git', ['init', '--quiet', zonePath])
  await server.restart()
  const baseURL = server.baseURL
  const zone = 'pstack-p1'
  const registered = await page.request.post(`${baseURL}/api/zones`, { data: { id: zone, path: zonePath } })
  expect(registered.ok(), await registered.text()).toBe(true)

  await page.goto(`${baseURL}?layout=expanded`)
  await page.getByTestId('tab-tasks').click()
  await page.getByTestId('task-create-open').click()
  await page.getByTestId('task-name').fill('P1 pstack Task')
  await page.getByTestId('task-next').click()
  const choice = page.getByTestId('task-suite')
  const pstack = choice.locator('option[value="pstack-omp"]')
  await expect(pstack).toBeEnabled()
  await choice.selectOption('pstack-omp')
  await page.screenshot({ path: info.outputPath('omp-selects-pstack.png') })
  await page.getByTestId('task-next').click()
  await page.getByTestId('save-task').click()
  await expect(page.getByTestId('task-dialog')).toBeHidden()
  const { tasks } = await server.getTasks()
  expect(tasks).toHaveLength(1)
  const [saved] = tasks
  expect(saved).toMatchObject({ agent: 'omp', suite: 'pstack-omp' })
  const { id, ...input } = saved
  const taskURL = `${baseURL}/api/tasks/${id}`
  const persisted = fs.readFileSync(server.storageFile, 'utf8')

  await page.getByTestId('task-edit').click()
  await page.getByTestId('task-agent').selectOption('pi')
  await page.getByTestId('task-next').click()
  await expect(choice).toHaveValue('pstack-omp')
  await expect(pstack).toBeDisabled()
  await expect(page.getByTestId('task-suite-issue')).toContainText('Suite「pstack-omp」不适用于 pi')
  await page.screenshot({ path: info.outputPath('pi-rejects-pstack.png') })
  await page.getByTestId('task-cancel').click()

  const incompatibility = { code: 'task-suite-invalid', message: expect.stringContaining('Suite「pstack-omp」不适用于 pi') }
  const piInput = { ...input, agent: 'pi' }
  const rejected = []
  for (const [method, url] of [['POST', `${baseURL}/api/tasks`], ['PUT', taskURL]]) {
    const response = await page.request.fetch(url, { method, data: piInput })
    expect(response.status(), await response.text()).toBe(400)
    const error = await response.json()
    expect(error).toMatchObject(incompatibility)
    rejected.push({ method, error })
  }
  expect((await (await page.request.get(taskURL)).json()).task).toEqual(saved)
  expect(fs.readFileSync(server.storageFile, 'utf8')).toBe(persisted)

  await page.getByTestId('tab-files').click()
  await zoneRowFor(page, zone).getByTestId('zone-create-chat').click()
  const dialog = page.getByTestId('chat-creation-dialog')
  await dialog.locator('#chat-workspace-target').selectOption('default')
  await dialog.getByTestId('chat-input').fill('P1 owned input')
  await dialog.getByTestId('chat-create-submit').click()
  await expect(dialog.getByTestId('chat-create-success')).toHaveText('创建成功，初始指示已提交')
  const calls = server.herdrCalls()
  const tabs = server.herdrTabs()
  expect(tabs).toHaveLength(1)
  expect(calls.filter(call => call.kind === 'agent_start')).toEqual([
    expect.objectContaining({ agent_kind: 'omp', pane: tabs[0].pane_id }),
  ])

  const previewURL = `${baseURL}/api/chats/preview`
  const selection = { zone, taskId: id }
  const preview = await page.request.post(previewURL, { data: selection })
  expect(preview.ok(), await preview.text()).toBe(true)
  const { chatId, name } = await preview.json()
  // Model an already persisted incompatible Task, bypassing save deliberately.
  const stale = JSON.stringify({ tasks: [{ ...saved, agent: 'pi' }] })
  fs.writeFileSync(server.storageFile, stale)
  expect((await (await page.request.get(taskURL)).json()).task).toMatchObject({ agent: 'pi', suite: 'pstack-omp' })
  const refusedPreview = await page.request.post(previewURL, { data: selection })
  expect(refusedPreview.status(), await refusedPreview.text()).toBe(400)
  expect(await refusedPreview.json()).toMatchObject(incompatibility)
  const refused = await page.request.post(`${baseURL}/api/chats`, {
    data: { ...selection, chatId, expectedName: name, workspaceTarget: 'default', input: 'Must not start' },
  })
  expect(refused.status(), await refused.text()).toBe(400)
  const launchError = await refused.json()
  expect(launchError).toMatchObject(incompatibility)
  expect(server.herdrCalls()).toEqual(calls)
  expect(server.herdrTabs()).toEqual(tabs)
  expect(fs.readFileSync(server.storageFile, 'utf8')).toBe(stale)
  await info.attach('pstack-p1-http-and-herdr', {
    body: JSON.stringify({ saved, rejected, previewError: await refusedPreview.json(), launchError, calls, tabs }, null, 2),
    contentType: 'application/json',
  })
})

// S1/P33: the browser must surface the server's real refusal and every Task,
// not optimistically hide the Suite or remove any reference from the list.
test('P33 browser refuses deletion and lists all referencing Tasks without changing them', async ({ page }) => {
  server = await startServer()
  definitions(server)
  for (const name of ['First browser reference', 'Second browser reference']) {
    const response = await page.request.post(`${server.baseURL}/api/tasks`, { data: { name, chatName: '{task.name}', agent: 'omp', model: 'default', thinking: 'default', skill: null, prompt: '', suite: 'shared-omp' } })
    expect(response.status()).toBe(200)
  }
  const { tasks: before } = await server.getTasks()
  await page.goto(`${server.baseURL}?layout=expanded`)
  await page.getByTestId('tab-suite').click()
  await page.getByRole('button', { name: '共享工具 · OMP · shared-omp', exact: true }).click()
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
  await expect(page.getByRole('button', { name: '共享工具 · OMP · shared-omp', exact: true })).toBeVisible()
  const suite = await page.request.get(`${server.baseURL}/api/suites/shared-omp`)
  expect(suite.status()).toBe(200)
  expect((await suite.json()).suite).toMatchObject(ompSuite)
})

// S1/P18: after an independent file deletion, both reading surfaces must show
// the retained missing ID. Hiding it while saving an empty list is a violation.
test('P18 browser retains and marks missing central MCP references and refuses saving them', async ({ page }) => {
  server = await startServer()
  const directory = definitions(server)
  const input = { name: '失效 MCP 来源', chatName: '{task.name}', agent: 'omp', model: 'default', thinking: 'default', skill: null, prompt: 'Keep trigger', suite: 'shared-omp' }
  const created = await page.request.post(`${server.baseURL}/api/tasks`, { data: input })
  expect(created.status()).toBe(200)
  const saved = (await created.json()).tasks[0]
  const authority = fs.readFileSync(path.join(directory, 'suites.json'), 'utf8')
  await page.goto(`${server.baseURL}?layout=expanded`)
  await page.getByTestId('tab-suite').click()
  await page.getByRole('button', { name: '共享工具 · OMP · shared-omp', exact: true }).click()
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
  await expect(page.getByTestId('task-suite')).toHaveValue('shared-omp')
  await expect(page.getByTestId('task-suite-issue')).toContainText('search')
  await page.getByTestId('task-next').click()
  await page.getByTestId('save-task').click()
  await expect(page.getByTestId('task-form-error')).toContainText('search')
  const read = await page.request.get(`${server.baseURL}/api/tasks/${saved.id}`)
  expect(read.status()).toBe(200)
  expect((await read.json()).task).toEqual(saved)
  expect(fs.readFileSync(path.join(directory, 'suites.json'), 'utf8')).toBe(authority)
})
