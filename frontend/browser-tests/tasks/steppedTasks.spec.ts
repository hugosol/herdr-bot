import fs from 'node:fs'
import { expect, test } from '@playwright/test'
import { startServer, type E2EServer } from '../support/support'
import { serializedChatRule } from '../support/ui'

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
  await page.goto(`${server.baseURL}?layout=expanded`)
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
  await expect(page.getByTestId('task-fixed-extensions')).toHaveCount(0)
  await expect(page.getByTestId('task-suite')).toHaveValue('')
  await expect(page.getByRole('checkbox')).toHaveCount(0)
  await page.getByTestId('task-back').click()
  await expect(page.getByTestId('task-name')).toHaveValue('发布巡检')
  expect(await serializedChatRule(rule)).toBe(PRESET.chatName)
  await expect(page.getByTestId('task-agent')).toHaveValue('omp')
  await expect(page.getByTestId('task-model')).toHaveValue('gpt-6.1-sol')
  await expect(page.getByTestId('task-thinking')).toHaveValue('high')
  await expect(page.getByTestId('task-skill')).toHaveValue('code-review')
  await expect(page.getByTestId('task-prompt')).toHaveValue(PRESET.prompt)
  await page.getByTestId('task-next').click()
  await page.getByTestId('task-next').click()
  await expect(page.getByTestId('task-step')).toHaveText('3 / 3 · 确认保存')
  const summary = page.getByTestId('task-summary')
  for (const [id, value] of Object.entries({
    name: '发布巡检', chatName: PRESET.chatName, agent: 'oh-my-pi', model: 'GPT-6.1-Sol', thinking: 'high',
    skill: '/code-review', prompt: PRESET.prompt, discovery: '关闭',
  })) await expect(summary.getByTestId(`summary-${id}`)).toHaveText(value)
  expect((await server.getTasks()).tasks).toEqual([])
  await page.getByTestId('save-task').click()
  await expect(page.getByTestId('task-feedback')).toHaveAttribute('data-kind', 'success')
  const saved = (await server.getTasks()).tasks[0]
  expect(saved).toEqual({ id: expect.any(String), ...PRESET })
  await expect(page.getByTestId('task-item')).not.toContainText('固定扩展')
  await expect(page.getByTestId('task-item')).toContainText('不选 Suite')

  // The externally hosted Chat already exists; deleting its Task must leave that conversation untouched.
  const zoneDir = `${server.storageFile}.zone`
  fs.mkdirSync(zoneDir)
  const zoneResponse = await fetch(`${server.baseURL}/api/zones`, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ id: 'test/zone', path: zoneDir }) })
  const zone = (await zoneResponse.json()).zones[0].id
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
  await expect(page.getByRole('checkbox')).toHaveCount(0)
  await page.getByTestId('task-next').click()
  await page.getByTestId('task-back').click()
  expect((await server.getTasks()).tasks).toEqual([saved])
  await page.getByTestId('task-next').click()
  await expect(summary.getByTestId('summary-prompt')).toHaveText('编辑首行\n编辑次行')
  await page.getByTestId('save-task').click()
  await expect(page.getByTestId('task-dialog')).toHaveCount(0)
  expect((await server.getTasks()).tasks).toEqual([{
    id: saved.id, taskType: 'normal', name: '编辑后的巡检', chatName: '固定 {agent} · {task.name} · {chat.id}', agent: 'pi',
    model: 'deepseek-flash', thinking: 'off', skill: 'teach', prompt: '编辑首行\n编辑次行',
  }])
  await page.getByTestId('task-delete').click()
  await page.getByTestId('task-delete-confirm').click()
  await expect(page.getByTestId('task-empty')).toBeVisible()
  expect((await server.getTasks()).tasks).toEqual([])
  expect(server.herdrCalls().filter((call) => call.kind !== 'agent_get')).toEqual(calls)
  expect(server.herdrTabs()).toEqual(tabs)
})

// S1/S2; P14/P17. Violation: hidden OMP choices reappear, or same-label Pi choices inherit them.
test('切换 Agent 不提供旧扩展编辑，新 Task 明确不选 Suite', async ({ page }) => {
  server = await startServer()
  await page.goto(`${server.baseURL}?layout=expanded`)
  await page.getByTestId('tab-tasks').click()
  await page.getByTestId('task-create-open').click()
  await page.getByTestId('task-name').fill('零选样例')
  await page.getByTestId('task-model').selectOption('gpt-6.1-sol')
  await page.getByTestId('task-thinking').selectOption('max')
  await page.getByTestId('task-next').click()
  await page.getByTestId('task-back').click()
  await page.getByTestId('task-agent').selectOption('pi')
  await expect(page.getByTestId('task-model')).toHaveValue('default')
  await expect(page.getByTestId('task-thinking')).toHaveValue('default')
  await page.getByTestId('task-next').click()
  await expect(page.getByRole('checkbox')).toHaveCount(0)
  await page.getByTestId('task-back').click()
  await page.getByTestId('task-agent').selectOption('omp')
  await expect(page.getByTestId('task-model')).toHaveValue('default')
  await expect(page.getByTestId('task-thinking')).toHaveValue('default')
  await page.getByTestId('task-next').click()
  await page.getByTestId('task-next').click()
  await expect(page.getByTestId('summary-suite')).toHaveText('不选 Suite')
  await page.getByTestId('save-task').click()
  await expect(page.getByTestId('task-dialog')).toHaveCount(0)
  expect((await server.getTasks()).tasks[0]).toMatchObject({ name: '零选样例', model: 'default', thinking: 'default', skill: null, prompt: '' })
})

// S1/S2/S3; P25/P26. Violation: editor reuses cached labels, or save trusts stale eligibility.


// S1/S2/S3; P4/P19/P20. Read-only storage rejects the actual replacement.
// Violation: optimistic success or destructive replacement would lose the old literal after restart.
test('真实磁盘写入故障明确失败且重读和重启保留旧 Task 与扩展', async ({ page }) => {
  server = await startServer()
  expect((await save({ ...PRESET })).status).toBe(200)
  const saved = (await server.getTasks()).tasks[0]
  await page.goto(`${server.baseURL}?layout=expanded`)
  await page.getByTestId('tab-tasks').click()
  await page.getByTestId('task-edit').click()
  await page.getByTestId('task-name').fill('不应保存的名字')
  await page.getByTestId('task-prompt').fill('失败首行\n失败次行')
  await page.getByTestId('task-next').click()
  await page.getByTestId('task-next').click()
  expect((await server.getTasks()).tasks).toEqual([saved])
  const originalMode = fs.statSync(server.storageFile).mode
  fs.chmodSync(server.storageFile, 0o444)
  try {
    await page.getByTestId('save-task').click()
    await expect(page.getByTestId('task-form-error')).toHaveAttribute('data-kind', 'error')
    await expect(page.getByTestId('task-feedback')).toHaveCount(0)
    await expect(page.getByTestId('task-dialog')).toBeVisible()
    expect((await server.getTasks()).tasks).toEqual([saved])
  } finally { fs.chmodSync(server.storageFile, originalMode) }
  await page.getByTestId('task-back').click()
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

