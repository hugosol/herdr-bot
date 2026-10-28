import fs from 'node:fs'
import { expect, test } from '@playwright/test'
import { startServer, type E2EServer } from './support'
import { serializedChatRule } from './ui'

let server: E2EServer | null = null
test.afterEach(async () => { await server?.cleanup(); server = null })

const PRESET = {
  taskType: 'normal',
  name: '发布巡检', chatName: '巡检 {zone.name} / {task.name} / {agent} / {chat.id}',
  agent: 'omp', model: 'gpt-6.1-sol', thinking: 'high', skill: 'code-review',
  prompt: '首行：检查发布\n次行：保留 "引号" 和路径 C:\\alpha',
}

async function save(body: unknown, id?: string) {
  return fetch(`${server!.baseURL}/api/tasks${id ? `/${id}` : ''}`, {
    method: id ? 'PUT' : 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body),
  })
}

// S1/S2; P1–P6/P12/P14/P16/P18. The literals below are independent of UI rendering.
// Violation: navigation writes early, loses fields, mislabels fixed/optional choices, or saves a different summary.
test('三步新增和编辑保留完整草稿，摘要与公开持久结果一致，删除不关闭既有 Chat', async ({ page }) => {
  server = await startServer({ herdr: {} })
  await page.goto(server.baseURL)
  await page.getByTestId('tab-tasks').click()
  await page.getByTestId('task-create-open').click()
  await expect(page.getByTestId('task-step')).toHaveText('1 / 3 · Task 预设')
  await expect(page.getByTestId('save-task')).toHaveCount(0)
  await page.getByTestId('task-name').fill(PRESET.name)
  const rule = page.getByTestId('task-chat-name')
  await rule.fill('')
  for (const id of ['insert-zone-name', 'insert-task-name', 'insert-agent', 'insert-chat-id']) {
    await page.getByTestId(id).click()
  }
  expect(await serializedChatRule(rule)).toBe('{zone.name}{task.name}{agent}{chat.id}')
  await expect(page.getByTestId('task-dialog').getByTestId('chat-name-preview')).toHaveText('alpha发布巡检ompa7f3c2')
  await rule.fill(PRESET.chatName)
  await expect(page.getByTestId('task-dialog').getByTestId('chat-name-preview')).toHaveText('巡检 alpha / 发布巡检 / omp / a7f3c2')
  await page.getByTestId('task-model').selectOption('gpt-6.1-sol')
  await page.getByTestId('task-thinking').selectOption('high')
  await page.getByTestId('task-skill').selectOption('code-review')
  await page.getByTestId('task-prompt').fill(PRESET.prompt)
  await page.getByTestId('task-next').click()
  await expect(page.getByTestId('task-step')).toHaveText('2 / 3 · 扩展配置')
  const fixed = page.getByTestId('task-fixed-extensions')
  await expect(fixed).toHaveText('固定扩展（始终加载，不可取消）herdr-agent-state、model-cost')
  await expect(fixed.getByRole('checkbox')).toHaveCount(0)
  await expect(page.getByRole('checkbox')).toHaveCount(3)
  await expect(page.getByTestId('extension-omp-codebase-tools')).not.toBeChecked()
  await expect(page.getByTestId('extension-omp-connect-bot')).not.toBeChecked()
  await page.getByTestId('extension-omp-codebase-tools').check()
  await page.getByTestId('extension-omp-connect-bot').check()
  await page.getByTestId('task-back').click()
  await expect(page.getByTestId('task-name')).toHaveValue('发布巡检')
  expect(await serializedChatRule(rule)).toBe(PRESET.chatName)
  await expect(page.getByTestId('task-agent')).toHaveValue('omp')
  await expect(page.getByTestId('task-model')).toHaveValue('gpt-6.1-sol')
  await expect(page.getByTestId('task-thinking')).toHaveValue('high')
  await expect(page.getByTestId('task-skill')).toHaveValue('code-review')
  await expect(page.getByTestId('task-prompt')).toHaveValue(PRESET.prompt)
  await page.getByTestId('task-next').click()
  await expect(page.getByTestId('extension-omp-codebase-tools')).toBeChecked()
  await expect(page.getByTestId('extension-omp-connect-bot')).toBeChecked()
  await page.getByTestId('task-next').click()
  await expect(page.getByTestId('task-step')).toHaveText('3 / 3 · 确认保存')
  const summary = page.getByTestId('task-summary')
  for (const [id, value] of Object.entries({
    name: '发布巡检', chatName: PRESET.chatName, agent: 'oh-my-pi', model: 'GPT-6.1-Sol', thinking: 'high',
    skill: '/code-review', prompt: PRESET.prompt, fixed: 'herdr-agent-state、model-cost', optional: 'codebase-tools、connect-bot', discovery: '关闭',
  })) await expect(summary.getByTestId(`summary-${id}`)).toHaveText(value)
  expect((await server.getTasks()).tasks).toEqual([])
  await page.getByTestId('save-task').click()
  await expect(page.getByTestId('task-feedback')).toHaveAttribute('data-kind', 'success')
  const saved = (await server.getTasks()).tasks[0]
  expect(saved).toEqual({ id: expect.any(String), ...PRESET, extensions: ['omp-codebase-tools', 'omp-connect-bot'] })
  await expect(page.getByTestId('task-item')).toContainText('固定扩展')
  await expect(page.getByTestId('task-item')).toContainText('connect-bot')

  // The externally hosted Chat already exists; deleting its Task must leave that conversation untouched.
  const zoneDir = `${server.storageFile}.zone`
  fs.mkdirSync(zoneDir)
  const zoneResponse = await fetch(`${server.baseURL}/api/zones`, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ path: zoneDir }) })
  const zone = (await zoneResponse.json()).zones[0].path
  const previewResponse = await fetch(`${server.baseURL}/api/chats/preview`, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ zone, taskId: saved.id }) })
  const preview = await previewResponse.json()
  const chatResponse = await fetch(`${server.baseURL}/api/chats`, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ zone, taskId: saved.id, chatId: preview.chatId, input: '检查' }) })
  expect(chatResponse.status).toBe(202)
  const chat = await chatResponse.json()
  await expect.poll(async () => (await (await fetch(`${server!.baseURL}/api/chats/${chat.id}`)).json()).stage).toBe('succeeded')
  const calls = server.herdrCalls().filter((call) => call.kind !== 'agent_get')
  const tabs = server.herdrTabs()

  await page.getByTestId('task-edit').click()
  await expect(page.getByTestId('task-step')).toHaveText('1 / 3 · Task 预设')
  await page.getByTestId('task-name').fill('编辑后的巡检')
  await rule.fill('固定 {agent} · {task.name} · {chat.id}')
  await page.getByTestId('task-agent').selectOption('pi')
  await page.getByTestId('task-model').selectOption('deepseek-flash')
  await page.getByTestId('task-thinking').selectOption('off')
  await page.getByTestId('task-skill').selectOption('teach')
  await page.getByTestId('task-prompt').fill('编辑首行\n编辑次行')
  await page.getByTestId('task-next').click()
  await expect(page.getByRole('checkbox')).toHaveCount(3)
  await page.getByTestId('extension-pi-codebase-tools').check()
  await page.getByTestId('task-next').click()
  await page.getByTestId('task-back').click()
  await expect(page.getByTestId('extension-pi-codebase-tools')).toBeChecked()
  expect((await server.getTasks()).tasks).toEqual([saved])
  await page.getByTestId('task-next').click()
  await expect(summary.getByTestId('summary-prompt')).toHaveText('编辑首行\n编辑次行')
  await page.getByTestId('save-task').click()
  await expect(page.getByTestId('task-dialog')).toHaveCount(0)
  expect((await server.getTasks()).tasks).toEqual([{
    id: saved.id, taskType: 'normal', name: '编辑后的巡检', chatName: '固定 {agent} · {task.name} · {chat.id}', agent: 'pi',
    model: 'deepseek-flash', thinking: 'off', skill: 'teach', prompt: '编辑首行\n编辑次行', extensions: ['pi-codebase-tools'],
  }])
  await page.getByTestId('task-delete').click()
  await page.getByTestId('task-delete-confirm').click()
  await expect(page.getByTestId('task-empty')).toBeVisible()
  expect((await server.getTasks()).tasks).toEqual([])
  expect(server.herdrCalls().filter((call) => call.kind !== 'agent_get')).toEqual(calls)
  expect(server.herdrTabs()).toEqual(tabs)
})

// S1/S2; P14/P17. Violation: hidden OMP choices reappear, or same-label Pi choices inherit them.
test('切换 Agent 清空扩展且切回不恢复，新 Task 零选明确保存为空', async ({ page }) => {
  server = await startServer()
  await page.goto(server.baseURL)
  await page.getByTestId('tab-tasks').click()
  await page.getByTestId('task-create-open').click()
  await page.getByTestId('task-name').fill('零选样例')
  await page.getByTestId('task-model').selectOption('gpt-6.1-sol')
  await page.getByTestId('task-thinking').selectOption('max')
  await page.getByTestId('task-next').click()
  await page.getByTestId('extension-omp-codebase-tools').check()
  await page.getByTestId('extension-omp-connect-bot').check()
  await page.getByTestId('task-back').click()
  await page.getByTestId('task-agent').selectOption('pi')
  await expect(page.getByTestId('task-model')).toHaveValue('default')
  await expect(page.getByTestId('task-thinking')).toHaveValue('default')
  await page.getByTestId('task-next').click()
  await expect(page.getByRole('checkbox')).toHaveCount(3)
  await expect(page.getByTestId('extension-pi-codebase-tools')).not.toBeChecked()
  await page.getByTestId('task-back').click()
  await page.getByTestId('task-agent').selectOption('omp')
  await expect(page.getByTestId('task-model')).toHaveValue('default')
  await expect(page.getByTestId('task-thinking')).toHaveValue('default')
  await page.getByTestId('task-next').click()
  await expect(page.getByTestId('extension-omp-codebase-tools')).not.toBeChecked()
  await expect(page.getByTestId('extension-omp-connect-bot')).not.toBeChecked()
  await page.getByTestId('task-next').click()
  await expect(page.getByTestId('summary-optional')).toHaveText('无')
  await expect(page.getByTestId('summary-fixed')).toHaveText('herdr-agent-state、model-cost')
  await page.getByTestId('save-task').click()
  await expect(page.getByTestId('task-dialog')).toHaveCount(0)
  expect((await server.getTasks()).tasks[0]).toMatchObject({ name: '零选样例', model: 'default', thinking: 'default', skill: null, prompt: '', extensions: [] })
})

// S1/S2/S3; P25/P26. Violation: editor reuses cached labels, or save trusts stale eligibility.
test('扩展元数据在同一进程重新编辑和保存时生效，只持久保存稳定选择', async ({ page }) => {
  server = await startServer()
  const original = fs.readFileSync(server.metadataFile, 'utf8')
  expect((await save({ ...PRESET, extensions: ['omp-codebase-tools'] })).status).toBe(200)
  const saved = (await server.getTasks()).tasks[0]
  await page.goto(server.baseURL)
  await page.getByTestId('tab-tasks').click()
  await page.getByTestId('task-edit').click()
  await page.getByTestId('task-next').click()
  await expect(page.getByRole('checkbox', { name: 'codebase-tools', exact: true })).toBeChecked()
  await page.getByTestId('task-cancel').click()
  const updated = original.replace("label = 'codebase-tools'", "label = '当前代码工具'").replace('omp-auto\\codebase-tools.ts', 'omp-auto\\changed-tools.ts')
  fs.writeFileSync(server.metadataFile, updated)
  await page.getByTestId('task-edit').click()
  await page.getByTestId('task-next').click()
  await expect(page.getByRole('checkbox', { name: '当前代码工具', exact: true })).toBeChecked()
  await page.getByTestId('task-next').click()
  await expect(page.getByTestId('summary-optional')).toHaveText('当前代码工具')
  // Alter eligibility after opening: the later save must use current metadata, not the page snapshot.
  fs.writeFileSync(server.metadataFile, updated.replace("kind = 'optional'", "kind = 'fixed'"))
  await page.getByTestId('save-task').click()
  await expect(page.getByTestId('task-form-error')).toContainText('不是当前 Agent 的可选扩展')
  await expect(page.getByTestId('task-feedback')).toHaveCount(0)
  expect((await server.getTasks()).tasks).toEqual([saved])
  fs.writeFileSync(server.metadataFile, updated)
  await page.getByTestId('save-task').click()
  await expect(page.getByTestId('task-dialog')).toHaveCount(0)
  expect((await server.getTasks()).tasks).toEqual([saved])
  expect(JSON.parse(fs.readFileSync(server.storageFile, 'utf8')).tasks).toEqual([saved])
})

// S1/S2/S3; P4/P19/P20. A real directory blocks the same-directory temporary write.
// Violation: optimistic success or destructive replacement would lose the old literal after restart.
test('真实磁盘写入故障明确失败且重读和重启保留旧 Task 与扩展', async ({ page }) => {
  server = await startServer()
  expect((await save({ ...PRESET, extensions: ['omp-codebase-tools', 'omp-connect-bot'] })).status).toBe(200)
  const saved = (await server.getTasks()).tasks[0]
  await page.goto(server.baseURL)
  await page.getByTestId('tab-tasks').click()
  await page.getByTestId('task-edit').click()
  await page.getByTestId('task-name').fill('不应保存的名字')
  await page.getByTestId('task-prompt').fill('失败首行\n失败次行')
  await page.getByTestId('task-next').click()
  await page.getByTestId('extension-omp-connect-bot').uncheck()
  await page.getByTestId('task-next').click()
  expect((await server.getTasks()).tasks).toEqual([saved])
  fs.mkdirSync(`${server.storageFile}.tmp`)
  await page.getByTestId('save-task').click()
  await expect(page.getByTestId('task-form-error')).toHaveAttribute('data-kind', 'error')
  await expect(page.getByTestId('task-feedback')).toHaveCount(0)
  await expect(page.getByTestId('task-dialog')).toBeVisible()
  expect((await server.getTasks()).tasks).toEqual([saved])
  fs.rmdirSync(`${server.storageFile}.tmp`)
  await page.getByTestId('task-back').click()
  await expect(page.getByTestId('extension-omp-connect-bot')).not.toBeChecked()
  await page.getByTestId('task-back').click()
  await expect(page.getByTestId('task-name')).toHaveValue('不应保存的名字')
  await expect(page.getByTestId('task-prompt')).toHaveValue('失败首行\n失败次行')
  await page.getByTestId('task-cancel').click()
  await server.restart()
  const independent = await fetch(`${server.baseURL}/api/tasks/${saved.id}`)
  expect((await independent.json()).task).toEqual(saved)
  expect((await server.getTasks()).tasks).toEqual([saved])
})

// S2/S3; P12–P16/P19/P25. Source: the six approved literal combinations.
// Violation: accepting IDs without Agent/kind validation, or dropping selections on disk/restart.
test('公开保存六种扩展组合并在独立客户端和真实重启后保留稳定标识', async () => {
  server = await startServer()
  const options = (await (await fetch(`${server.baseURL}/api/tasks`)).json()).options
  expect(options.extensions.map(({ id, agent, kind }: { id: string; agent: string; kind: string }) => ({ id, agent, kind }))).toEqual([
    { id: 'omp-herdr-agent-state', agent: 'omp', kind: 'fixed' },
    { id: 'omp-model-cost', agent: 'omp', kind: 'fixed' },
    { id: 'omp-manager-chat', agent: 'omp', kind: 'fixed' },
    { id: 'omp-node-chat', agent: 'omp', kind: 'fixed' },
    { id: 'omp-codebase-tools', agent: 'omp', kind: 'optional' },
    { id: 'omp-connect-bot', agent: 'omp', kind: 'optional' },
    { id: 'omp-oh-my-wayfinder', agent: 'omp', kind: 'optional' },
    { id: 'pi-herdr-agent-state', agent: 'pi', kind: 'fixed' },
    { id: 'pi-model-cost', agent: 'pi', kind: 'fixed' },
    { id: 'pi-codebase-tools', agent: 'pi', kind: 'optional' },
    { id: 'pi-connect-bot', agent: 'pi', kind: 'optional' },
    { id: 'pi-web-access', agent: 'pi', kind: 'optional' },
  ])
  const cases = [
    { agent: 'omp', extensions: [] },
    { agent: 'omp', extensions: ['omp-codebase-tools'] },
    { agent: 'omp', extensions: ['omp-connect-bot'] },
    { agent: 'omp', extensions: ['omp-codebase-tools', 'omp-connect-bot'] },
    { agent: 'pi', extensions: [] },
    { agent: 'pi', extensions: ['pi-codebase-tools'] },
  ]
  const saved = []
  for (const selection of cases) {
    const literal = { ...PRESET, ...selection }
    const response = await save(literal)
    expect(response.status).toBe(200)
    const task = (await response.json()).tasks.at(-1)
    expect(task).toEqual({ id: expect.any(String), ...literal })
    saved.push(task)
  }
  for (const extensions of [['pi-codebase-tools'], ['omp-model-cost'], ['missing-extension']]) {
    const rejected = await save({ ...PRESET, extensions })
    expect(rejected.status).toBe(400)
    expect((await server.getTasks()).tasks).toEqual(saved)
  }
  expect((await save({ ...PRESET, agent: 'pi', extensions: ['omp-codebase-tools'] })).status).toBe(400)
  expect((await save({ ...PRESET, extensions: ['omp-codebase-tools', 'omp-codebase-tools'] })).status).toBe(400)
  expect((await server.getTasks()).tasks).toEqual(saved)
  const stored = JSON.parse(fs.readFileSync(server.storageFile, 'utf8')).tasks
  expect(stored).toEqual(saved)
  expect(Object.keys(stored[3]).sort()).toEqual(['agent', 'chatName', 'extensions', 'id', 'model', 'name', 'prompt', 'skill', 'taskType', 'thinking'])
  await server.restart()
  expect((await server.getTasks()).tasks).toEqual(saved)
  for (const task of saved) {
    const response = await fetch(`${server.baseURL}/api/tasks/${task.id}`)
    expect((await response.json()).task).toEqual(task)
  }
})
