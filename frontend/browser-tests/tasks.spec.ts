import fs from 'node:fs'

import { expect, test } from '@playwright/test'
import type { Page } from '@playwright/test'

import { startServer, type TaskView, type E2EServer } from './support'
import { saveTask, serializedChatRule } from './ui'

/** 只取 Task 字段，忽略后端生成的标识。 */
function fieldsOf(task: TaskView) {
  const { id: _id, ...fields } = task
  return fields
}

async function createOpen(page: Page) {
  await page.getByTestId('task-create-open').click()
  await expect(page.getByTestId('task-dialog')).toBeVisible()
}

async function selectValues(page: Page, testId: string) {
  return page
    .getByTestId(testId)
    .locator('option')
    .evaluateAll((options) =>
      options.map((option) => (option as HTMLOptionElement).value),
    )
}

let server: E2EServer | null = null

test.afterEach(async () => {
  await server?.cleanup()
  server = null
})

test('真实浏览器新增、共享读取、编辑、删除完整 Task', async ({ page }) => {
  server = await startServer()

  const literal = { name: '发布检查',
  chatName: '{zone.name} · {task.name} · {agent} · {chat.id}',
  agent: 'pi',
    taskType: 'normal',
  model: 'deepseek-flash',
  thinking: 'max',
  skill: 'code-review', prompt: '第一行：检查登录\n第二行：保留 "引号" 与反斜杠 \\', extensions: [] }

  await page.goto(server.baseURL)
  await expect(page.getByTestId('tab-tasks')).toHaveText('task')

  // 既有 tab 仍可操作。
  await page.getByTestId('tab-files').click()
  await expect(page.getByTestId('panel-files')).toBeVisible()
  await page.getByTestId('server-button').click()
  await expect(page.getByTestId('start-button')).toBeEnabled()
  await page.getByTestId('server-back').click()
  await page.getByTestId('tab-tasks').click()
  await expect(page.getByTestId('task-empty')).toBeVisible()

  await createOpen(page)
  const dialog = page.getByTestId('task-dialog')
  await page.getByTestId('task-name').fill('发布检查')

  // 四个已承诺字段的插入与示例预览。
  const chatName = page.getByTestId('task-chat-name')
  await chatName.fill('')
  await page.getByTestId('insert-zone-name').click()
  await expect.poll(() => serializedChatRule(chatName)).toBe('{zone.name}')
  await page.getByTestId('insert-task-name').click()
  await expect.poll(() => serializedChatRule(chatName)).toBe('{zone.name}{task.name}')
  await page.getByTestId('insert-agent').click()
  await page.getByTestId('insert-chat-id').click()
  await expect
    .poll(() => serializedChatRule(chatName))
    .toBe('{zone.name}{task.name}{agent}{chat.id}')
  await expect(dialog.getByTestId('chat-name-preview')).toHaveText(
    'alpha发布检查ompa7f3c2',
  )
  await chatName.fill(literal.chatName)
  await expect(dialog.getByTestId('chat-name-preview')).toHaveText(
    'alpha · 发布检查 · omp · a7f3c2',
  )

  await page.getByTestId('task-agent').selectOption('pi')
  await page.getByTestId('task-model').selectOption('deepseek-flash')
  await page.getByTestId('task-thinking').selectOption('max')
  await page.getByTestId('task-skill').selectOption('code-review')
  await page.getByTestId('task-prompt').fill(literal.prompt)
  await saveTask(page)

  await expect(page.getByTestId('task-dialog')).toBeHidden()
  const row = page.getByTestId('task-item')
  await expect(row).toHaveCount(1)
  await expect(row).toContainText('发布检查')
  await expect(row).toContainText('alpha · 发布检查 · pi · a7f3c2')
  await expect(row).not.toContainText('{zone.name}')
  await expect(row).toContainText('pi')
  await expect(row).toContainText('deepseek-flash')
  await expect(row).toContainText('max')
  await expect(row).toContainText('/code-review')
  await expect(row).toContainText('第一行：检查登录')

  // 独立 HTTP 客户端读取同一份已保存值（不是同页缓存）。
  const shared = await server.getTasks()
  expect(shared.tasks).toHaveLength(1)
  const saved = shared.tasks[0]
  expect(saved.id.length).toBeGreaterThan(0)
  expect(fieldsOf(saved)).toEqual(literal)

  const selected = await fetch(
    `${server.baseURL}/api/tasks/${encodeURIComponent(saved.id)}`,
  )
  expect(selected.status).toBe(200)
  expect((await selected.json()).task).toEqual(saved)

  // 编辑：表单带回已保存值，保存后独立客户端读到新字面值。
  await row.getByTestId('task-edit').click()
  await expect(page.getByTestId('task-dialog')).toBeVisible()
  await expect(page.getByTestId('task-name')).toHaveValue('发布检查')
  await expect
    .poll(() => serializedChatRule(page.getByTestId('task-chat-name')))
    .toBe(literal.chatName)
  await expect(page.getByTestId('task-prompt')).toHaveValue(literal.prompt)
  await page.getByTestId('task-name').fill('发布检查（改）')
  await page.getByTestId('task-chat-name').fill('{task.name} · {agent} · {chat.id}')
  await page.getByTestId('task-agent').selectOption('omp')
  await page.getByTestId('task-model').selectOption('gpt-6.1-sol')
  await page.getByTestId('task-thinking').selectOption('xhigh')
  await page.getByTestId('task-skill').selectOption('teach')
  await page.getByTestId('task-prompt').fill('编辑后的第一行\n编辑后的第二行')
  await saveTask(page)

  await expect(page.getByTestId('task-item')).toContainText('发布检查（改）')
  await expect(page.getByTestId('task-item')).toContainText(
    '发布检查（改） · omp · a7f3c2',
  )
  const afterEdit = await server.getTasks()
  expect(afterEdit.tasks).toHaveLength(1)
  expect(afterEdit.tasks[0].id).toBe(saved.id)
  expect(fieldsOf(afterEdit.tasks[0])).toEqual({ name: '发布检查（改）',
  chatName: '{task.name} · {agent} · {chat.id}',
  agent: 'omp',
    taskType: 'normal',
  model: 'gpt-6.1-sol',
  thinking: 'xhigh',
  skill: 'teach', prompt: '编辑后的第一行\n编辑后的第二行', extensions: [] })

  // 打开、取消及 Escape 均不删除；只有明确确认才提交删除。
  let deletes = 0
  page.on('request', (request) => {
    if (request.method() === 'DELETE') deletes++
  })
  const deleteButton = page.getByTestId('task-item').getByTestId('task-delete')
  await deleteButton.click()
  await expect(page.getByTestId('task-delete-cancel')).toBeFocused()
  expect((await server.getTasks()).tasks).toEqual(afterEdit.tasks)
  expect(deletes).toBe(0)
  await page.getByTestId('task-delete-cancel').click()
  await expect(page.getByTestId('task-delete-dialog')).toHaveCount(0)
  await expect(deleteButton).toBeFocused()
  await deleteButton.click()
  await page.keyboard.press('Escape')
  await expect(page.getByTestId('task-delete-dialog')).toHaveCount(0)
  expect(deletes).toBe(0)
  await deleteButton.click()
  await page.getByTestId('task-delete-confirm').click()
  await expect(page.getByTestId('task-empty')).toBeVisible()
  expect(deletes).toBe(1)
  expect((await server.getTasks()).tasks).toEqual([])
  const unavailable = await fetch(
    `${server.baseURL}/api/tasks/${encodeURIComponent(saved.id)}`,
  )
  expect(unavailable.status).toBe(404)
  expect((await unavailable.json()).code).toBe('task-not-found')

  // 删除后仍停留在 task tab，不自动跳到 chat。
  await expect(page.getByTestId('tab-tasks')).toHaveAttribute('aria-selected', 'true')
})

test('真实浏览器按 agent/model 能力限制 thinking 并处理切换', async ({ page }) => {
  server = await startServer()
  await page.goto(server.baseURL)
  await page.getByTestId('tab-tasks').click()
  await expect(page.getByTestId('task-empty')).toBeVisible()
  await createOpen(page)

  const agent = page.getByTestId('task-agent')
  const model = page.getByTestId('task-model')
  const thinking = page.getByTestId('task-thinking')
  const skill = page.getByTestId('task-skill')

  // Agent 与 Skill 选择独立于模型目录。
  await expect(agent.locator('option')).toHaveCount(2)
  expect(await selectValues(page, 'task-agent')).toEqual(['omp', 'pi'])
  expect(await selectValues(page, 'task-skill')).toEqual([
    '',
    'grill-with-docs',
    'teach',
    'code-review',
    'wayfinder',
  ])

  // model 默认时 thinking 只能沿用默认值。
  expect(await selectValues(page, 'task-thinking')).toEqual(['default'])

  await model.selectOption('gpt-6.1-sol')
  expect(await selectValues(page, 'task-thinking')).toEqual([
    'default',
    'low',
    'medium',
    'high',
    'xhigh',
    'max',
  ])
  await thinking.selectOption('xhigh')

  // 不适用：重置并提示。
  await model.selectOption('deepseek-flash')
  await expect(thinking).toHaveValue('default')
  await expect(page.getByTestId('thinking-notice')).toContainText(
    '当前思考级别「xhigh」不适用于 omp / deepseek-flash，已重置为「沿用 Agent 默认值」。',
  )

  // 仍适用：保留，不提示。
  await thinking.selectOption('high')
  await model.selectOption('gpt-6.1-sol')
  await expect(thinking).toHaveValue('high')
  await expect(page.getByTestId('thinking-notice')).toHaveCount(0)

  // 更换 agent：model 与 thinking 都重置。
  await agent.selectOption('pi')
  await expect(model).toHaveValue('default')
  await expect(thinking).toHaveValue('default')
  expect(await selectValues(page, 'task-thinking')).toEqual(['default'])

  // Pi 的能力表与 OMP 不同：gpt-6.1-sol 不支持 off，但支持 xhigh / max。
  await model.selectOption('gpt-6.1-sol')
  expect(await selectValues(page, 'task-thinking')).toEqual([
    'default',
    'minimal',
    'low',
    'medium',
    'high',
    'xhigh',
    'max',
  ])
  await thinking.selectOption('xhigh')
  await model.selectOption('deepseek-flash')
  await expect(thinking).toHaveValue('default')
  await expect(page.getByTestId('thinking-notice')).toContainText(
    '当前思考级别「xhigh」不适用于 pi / deepseek-flash',
  )
  expect(await selectValues(page, 'task-thinking')).toEqual([
    'default',
    'off',
    'low',
    'high',
    'max',
  ])

  // DeepSeek 的 off 不能沿用到 GPT-6.1-sol。
  await thinking.selectOption('off')
  await model.selectOption('gpt-6.1-sol')
  await expect(thinking).toHaveValue('default')
  await expect(page.getByTestId('thinking-notice')).toContainText(
    '当前思考级别「off」不适用于 pi / gpt-6.1-sol',
  )

  // Pi 的 Luna 支持 off；切换到 Astra 必须重置，而不能提交不支持的 off。
  await model.selectOption('gpt-6-luna')
  await thinking.selectOption('off')
  await expect(thinking).toHaveValue('off')
  await model.selectOption('gpt-6-astra')
  await expect(thinking).toHaveValue('default')
  await expect(thinking.locator('option[value="off"]')).toHaveCount(0)
  await thinking.selectOption('minimal')

  // OMP 不支持 Pi 的 minimal/off，但两个新模型均支持 max。
  await agent.selectOption('omp')
  await model.selectOption('gpt-6-luna')
  await expect(thinking.locator('option[value="off"], option[value="minimal"]')).toHaveCount(0)
  await thinking.selectOption('max')
  await model.selectOption('gpt-6-astra')
  await expect(thinking).toHaveValue('max')

  // 改回默认模型：thinking 也只能默认。
  await model.selectOption('default')
  await expect(thinking).toHaveValue('default')
  expect(await selectValues(page, 'task-thinking')).toEqual(['default'])
})

test('保存失败不表示已保存；预先写定的 Task 经真实进程重启后仍可读取', async ({ page }) => {
  server = await startServer()

  const old = { name: '重启保留',
  chatName: '{zone.name} · {task.name} · {chat.id}',
  agent: 'omp',
    taskType: 'normal',
  model: 'gpt-6.1-sol',
  thinking: 'high',
  skill: 'teach', prompt: '重启前的第一行\n重启前的第二行', extensions: [] }
  const created = await fetch(`${server.baseURL}/api/tasks`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(old),
  })
  expect(created.status).toBe(200)
  const saved = (await created.json()).tasks[0] as TaskView

  await page.goto(server.baseURL)
  await page.getByTestId('tab-tasks').click()
  await expect(page.getByTestId('task-item')).toContainText('重启保留')

  // 真实存储写入失败：页面显示失败，不表示已保存；独立读取仍是此前值。
  fs.chmodSync(server.storageFile, 0o444)
  await createOpen(page)
  await page.getByTestId('task-name').fill('不应保存')
  await saveTask(page)
  await expect(page.getByTestId('task-form-error')).toHaveAttribute('data-kind', 'error')
  await expect(page.getByTestId('task-dialog')).toBeVisible()
  await expect(page.getByTestId('task-feedback')).toHaveCount(0)
  await expect(page.getByTestId('task-item')).toHaveCount(1)
  await expect(page.getByTestId('task-item')).not.toContainText('不应保存')
  expect((await server.getTasks()).tasks).toEqual([saved])

  // 恢复可写并真实重启进程：预先写定的 Task 仍在。
  fs.chmodSync(server.storageFile, 0o666)
  await page.getByTestId('task-cancel').click()
  await server.restart()
  // 新进程可能监听新的临时端口：按新的公开入口重新打开页面，而不是重载旧地址。
  await page.goto(server.baseURL)
  await page.getByTestId('tab-tasks').click()
  await expect(page.getByTestId('task-item')).toContainText('重启保留')
  await expect(page.getByTestId('task-item')).not.toContainText('不应保存')
  expect((await server.getTasks()).tasks).toEqual([saved])
})
