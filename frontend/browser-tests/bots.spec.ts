import fs from 'node:fs'

import { expect, test } from '@playwright/test'
import type { Page } from '@playwright/test'

import { startServer, type BotView, type E2EServer } from './support'
import { saveBot, serializedChatRule } from './ui'

/** 只取 Bot 字段，忽略后端生成的标识。 */
function fieldsOf(bot: BotView) {
  const { id: _id, ...fields } = bot
  return fields
}

async function createOpen(page: Page) {
  await page.getByTestId('bot-create-open').click()
  await expect(page.getByTestId('bot-dialog')).toBeVisible()
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

test('真实浏览器新增、共享读取、编辑、删除完整 Bot', async ({ page }) => {
  server = await startServer()

  const literal = { name: '发布检查',
  chatName: '{zone.name} · {bot.name} · {agent} · {chat.id}',
  agent: 'pi',
  model: 'deepseek-flash',
  thinking: 'max',
  skill: 'code-review', prompt: '第一行：检查登录\n第二行：保留 "引号" 与反斜杠 \\', extensions: [] }

  await page.goto(server.baseURL)
  await expect(page.getByTestId('tab-bots')).toHaveText('bot')

  // 既有 tab 仍可操作。
  await page.getByTestId('tab-files').click()
  await expect(page.getByTestId('panel-files')).toBeVisible()
  await page.getByTestId('tab-control').click()
  await expect(page.getByTestId('start-button')).toBeEnabled()
  await page.getByTestId('tab-bots').click()
  await expect(page.getByTestId('bot-empty')).toBeVisible()

  await createOpen(page)
  const dialog = page.getByTestId('bot-dialog')
  await page.getByTestId('bot-name').fill('发布检查')

  // 四个已承诺字段的插入与示例预览。
  const chatName = page.getByTestId('bot-chat-name')
  await chatName.fill('')
  await page.getByTestId('insert-zone-name').click()
  await expect.poll(() => serializedChatRule(chatName)).toBe('{zone.name}')
  await page.getByTestId('insert-bot-name').click()
  await expect.poll(() => serializedChatRule(chatName)).toBe('{zone.name}{bot.name}')
  await page.getByTestId('insert-agent').click()
  await page.getByTestId('insert-chat-id').click()
  await expect
    .poll(() => serializedChatRule(chatName))
    .toBe('{zone.name}{bot.name}{agent}{chat.id}')
  await expect(dialog.getByTestId('chat-name-preview')).toHaveText(
    'alpha发布检查ompa7f3c2',
  )
  await chatName.fill(literal.chatName)
  await expect(dialog.getByTestId('chat-name-preview')).toHaveText(
    'alpha · 发布检查 · omp · a7f3c2',
  )

  await page.getByTestId('bot-agent').selectOption('pi')
  await page.getByTestId('bot-model').selectOption('deepseek-flash')
  await page.getByTestId('bot-thinking').selectOption('max')
  await page.getByTestId('bot-skill').selectOption('code-review')
  await page.getByTestId('bot-prompt').fill(literal.prompt)
  await saveBot(page)

  await expect(page.getByTestId('bot-dialog')).toBeHidden()
  const row = page.getByTestId('bot-item')
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
  const shared = await server.getBots()
  expect(shared.bots).toHaveLength(1)
  const saved = shared.bots[0]
  expect(saved.id.length).toBeGreaterThan(0)
  expect(fieldsOf(saved)).toEqual(literal)

  const selected = await fetch(
    `${server.baseURL}/api/bots/${encodeURIComponent(saved.id)}`,
  )
  expect(selected.status).toBe(200)
  expect((await selected.json()).bot).toEqual(saved)

  // 编辑：表单带回已保存值，保存后独立客户端读到新字面值。
  await row.getByTestId('bot-edit').click()
  await expect(page.getByTestId('bot-dialog')).toBeVisible()
  await expect(page.getByTestId('bot-name')).toHaveValue('发布检查')
  await expect
    .poll(() => serializedChatRule(page.getByTestId('bot-chat-name')))
    .toBe(literal.chatName)
  await expect(page.getByTestId('bot-prompt')).toHaveValue(literal.prompt)
  await page.getByTestId('bot-name').fill('发布检查（改）')
  await page.getByTestId('bot-chat-name').fill('{bot.name} · {agent} · {chat.id}')
  await page.getByTestId('bot-agent').selectOption('omp')
  await page.getByTestId('bot-model').selectOption('gpt-6.1-sol')
  await page.getByTestId('bot-thinking').selectOption('xhigh')
  await page.getByTestId('bot-skill').selectOption('teach')
  await page.getByTestId('bot-prompt').fill('编辑后的第一行\n编辑后的第二行')
  await saveBot(page)

  await expect(page.getByTestId('bot-item')).toContainText('发布检查（改）')
  await expect(page.getByTestId('bot-item')).toContainText(
    '发布检查（改） · omp · a7f3c2',
  )
  const afterEdit = await server.getBots()
  expect(afterEdit.bots).toHaveLength(1)
  expect(afterEdit.bots[0].id).toBe(saved.id)
  expect(fieldsOf(afterEdit.bots[0])).toEqual({ name: '发布检查（改）',
  chatName: '{bot.name} · {agent} · {chat.id}',
  agent: 'omp',
  model: 'gpt-6.1-sol',
  thinking: 'xhigh',
  skill: 'teach', prompt: '编辑后的第一行\n编辑后的第二行', extensions: [] })

  // 删除：独立客户端再读取并尝试按标识选择时，目标 Bot 不可用。
  await page.getByTestId('bot-item').getByTestId('bot-delete').click()
  await expect(page.getByTestId('bot-empty')).toBeVisible()
  expect((await server.getBots()).bots).toEqual([])
  const unavailable = await fetch(
    `${server.baseURL}/api/bots/${encodeURIComponent(saved.id)}`,
  )
  expect(unavailable.status).toBe(404)
  expect((await unavailable.json()).code).toBe('bot-not-found')

  // 删除后仍停留在 bot tab，不自动跳到 chat。
  await expect(page.getByTestId('tab-bots')).toHaveAttribute('aria-selected', 'true')
})

test('真实浏览器按 agent/model 能力限制 thinking 并处理切换', async ({ page }) => {
  server = await startServer()
  await page.goto(server.baseURL)
  await page.getByTestId('tab-bots').click()
  await expect(page.getByTestId('bot-empty')).toBeVisible()
  await createOpen(page)

  const agent = page.getByTestId('bot-agent')
  const model = page.getByTestId('bot-model')
  const thinking = page.getByTestId('bot-thinking')
  const skill = page.getByTestId('bot-skill')

  // agent 固定选项只有 omp / pi；model 只有默认值与两个已确认模型；Skill 仅三个选项。
  await expect(agent.locator('option')).toHaveCount(2)
  expect(await selectValues(page, 'bot-agent')).toEqual(['omp', 'pi'])
  expect(await selectValues(page, 'bot-model')).toEqual([
    'default',
    'gpt-6.1-sol',
    'deepseek-flash',
  ])
  expect(await selectValues(page, 'bot-skill')).toEqual([
    '',
    'grill-with-docs',
    'teach',
    'code-review',
  ])

  // model 默认时 thinking 只能沿用默认值。
  expect(await selectValues(page, 'bot-thinking')).toEqual(['default'])

  await model.selectOption('gpt-6.1-sol')
  expect(await selectValues(page, 'bot-thinking')).toEqual([
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
  expect(await selectValues(page, 'bot-thinking')).toEqual(['default'])

  // Pi 的能力表与 OMP 不同：gpt-6.1-sol 不支持 off，但支持 xhigh / max。
  await model.selectOption('gpt-6.1-sol')
  expect(await selectValues(page, 'bot-thinking')).toEqual([
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
  expect(await selectValues(page, 'bot-thinking')).toEqual([
    'default',
    'off',
    'low',
    'high',
    'max',
  ])

  // off 只在 deepseek-flash 受支持；换回 gpt-6.1-sol 时重置并提示。
  await thinking.selectOption('off')
  await model.selectOption('gpt-6.1-sol')
  await expect(thinking).toHaveValue('default')
  await expect(page.getByTestId('thinking-notice')).toContainText(
    '当前思考级别「off」不适用于 pi / gpt-6.1-sol',
  )

  // 改回默认模型：thinking 也只能默认。
  await model.selectOption('default')
  await expect(thinking).toHaveValue('default')
  expect(await selectValues(page, 'bot-thinking')).toEqual(['default'])
})

test('保存失败不表示已保存；预先写定的 Bot 经真实进程重启后仍可读取', async ({ page }) => {
  server = await startServer()

  const old = { name: '重启保留',
  chatName: '{zone.name} · {bot.name} · {chat.id}',
  agent: 'omp',
  model: 'gpt-6.1-sol',
  thinking: 'high',
  skill: 'teach', prompt: '重启前的第一行\n重启前的第二行', extensions: [] }
  const created = await fetch(`${server.baseURL}/api/bots`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(old),
  })
  expect(created.status).toBe(200)
  const saved = (await created.json()).bots[0] as BotView

  await page.goto(server.baseURL)
  await page.getByTestId('tab-bots').click()
  await expect(page.getByTestId('bot-item')).toContainText('重启保留')

  // 真实存储写入失败：页面显示失败，不表示已保存；独立读取仍是此前值。
  fs.chmodSync(server.storageFile, 0o444)
  await createOpen(page)
  await page.getByTestId('bot-name').fill('不应保存')
  await saveBot(page)
  await expect(page.getByTestId('bot-form-error')).toHaveAttribute('data-kind', 'error')
  await expect(page.getByTestId('bot-dialog')).toBeVisible()
  await expect(page.getByTestId('bot-feedback')).toHaveCount(0)
  await expect(page.getByTestId('bot-item')).toHaveCount(1)
  await expect(page.getByTestId('bot-item')).not.toContainText('不应保存')
  expect((await server.getBots()).bots).toEqual([saved])

  // 恢复可写并真实重启进程：预先写定的 Bot 仍在。
  fs.chmodSync(server.storageFile, 0o666)
  await page.getByTestId('bot-cancel').click()
  await server.restart()
  // 新进程可能监听新的临时端口：按新的公开入口重新打开页面，而不是重载旧地址。
  await page.goto(server.baseURL)
  await page.getByTestId('tab-bots').click()
  await expect(page.getByTestId('bot-item')).toContainText('重启保留')
  await expect(page.getByTestId('bot-item')).not.toContainText('不应保存')
  expect((await server.getBots()).bots).toEqual([saved])
})
