import { execFileSync } from 'node:child_process'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'

import { expect, test } from '@playwright/test'
import type { Page } from '@playwright/test'

import { startServer, type E2EServer } from './support'
import { zoneRowFor } from './ui'

const BOT = {
  name: '审查',
  chatName: '{zone.name} · {bot.name} · {chat.id}',
  agent: 'omp',
  model: 'default',
  thinking: 'default',
  skill: null,
  prompt: '',
}

let server: E2EServer | null = null
let zoneRoot: string | null = null

test.afterEach(async () => {
  try { await server?.cleanup() }
  finally {
    server = null
    if (zoneRoot !== null) {
      fs.rmSync(zoneRoot, { recursive: true, force: true })
      zoneRoot = null
    }
  }
})

async function jsonPost(baseURL: string, endpoint: string, body: unknown) {
  const response = await fetch(`${baseURL}${endpoint}`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  })
  if (!response.ok) {
    throw new Error(`${endpoint} 失败：HTTP ${response.status} ${await response.text()}`)
  }
  return response.json()
}

/** 预建普通目录与 Git 工作树，并把它们登记到真实服务。 */
async function prepareZones(baseURL: string) {
  zoneRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'herdr-bot zones '))
  const normal = path.join(zoneRoot, 'alpha')
  const git = path.join(zoneRoot, 'beta')
  fs.mkdirSync(normal)
  fs.mkdirSync(git)
  execFileSync('git', ['init', git], { stdio: 'ignore' })

  const first = await jsonPost(baseURL, '/api/zones', { path: normal })
  const second = await jsonPost(baseURL, '/api/zones', { path: git })
  const paths: string[] = [...first.zones, ...second.zones].map(
    (zone: { path: string }) => zone.path,
  )
  return {
    normal: paths.find((candidate) => candidate.includes('alpha')) ?? normal,
    git: paths.find((candidate) => candidate.includes('beta')) ?? git,
  }
}

async function prepareBot(baseURL: string, bot: Record<string, unknown> = BOT) {
  const saved = await jsonPost(baseURL, '/api/bots', bot)
  return saved.bots[saved.bots.length - 1] as { id: string }
}

async function openChatDialog(page: Page, zonePath: string, useCurrent = false) {
  await page.getByTestId('tab-files').click()
  const row = zoneRowFor(page, zonePath)
  await expect(row).toHaveCount(1)
  await row.getByTestId('zone-create-chat').click()
  const dialog = page.getByTestId('chat-creation-dialog')
  await expect(dialog).toBeVisible()
  if (!useCurrent) await dialog.locator('#chat-workspace-target').selectOption('default')
  return dialog
}

async function waitForHerdrCall(page: Page, kind: string) {
  await expect
    .poll(
      () => server?.herdrCalls().some((call) => call.kind === kind) ?? false,
      { message: `受控 Herdr 应收到 ${kind}`, timeout: 10_000 },
    )
    .toBe(true)
}

function callsOfKind(kind: string): Record<string, unknown>[] {
  return (server?.herdrCalls() ?? []).filter((call) => call.kind === kind)
}

test('从普通目录与 Git 工作树对应的 zone 行发起 Chat：预览名称、阶段、成功与定位', { tag: '@time-contract' }, async ({
  page,
}) => {
  server = await startServer({
    herdr: {
      workspaces: [{ workspace_id: 'w-target', label: 'alpha' }, { workspace_id: 'w-beta', label: 'beta' }],
      behavior: { agent_start_delay_ms: 2_000, prompt_delay_ms: 1_500 },
    },
  })
  const zones = await prepareZones(server.baseURL)
  await prepareBot(server.baseURL)
  await page.goto(server.baseURL)

  // 两个 zone 行都有独立的“+”；分别打开时绑定正确的 zone。
  const gitDialog = await openChatDialog(page, zones.git)
  await expect(gitDialog.getByTestId('chat-zone')).toContainText(zones.git)
  await gitDialog.getByTestId('chat-create-close').click()
  await expect(gitDialog).toHaveCount(0)

  const dialog = await openChatDialog(page, zones.normal, true)
  await expect(dialog.getByTestId('chat-zone')).toContainText(zones.normal)
  await expect(dialog.locator('#chat-workspace-target')).toHaveValue('current')

  const preview = dialog.getByTestId('chat-name-preview')
  await expect(preview).toHaveText(/^alpha · 审查 · [0-9a-f]{6}$/)
  const shortId = ((await preview.textContent()) ?? '').split(' · ')[2]
  const previewRequests: string[] = []
  page.on('request', request => {
    if (request.url().endsWith('/api/chats/preview')) previewRequests.push(request.url())
  })
  await dialog.locator('#chat-name-rule').fill('{zone.name} · 发布检查 · {chat.id}')
  const previewName = `alpha · 发布检查 · ${shortId}`
  await expect(preview).toHaveText(previewName)
  expect(previewRequests).toEqual([])
  expect((await server.getBots()).bots[0].chatName).toBe(BOT.chatName)

  await dialog.getByTestId('chat-input').fill('普通任务：检查登录逻辑')
  await dialog.getByTestId('chat-create-submit').click()

  // 启动阶段是稳定的界面状态；受控 agent 尚未就绪时绝无初始输入。
  await expect(dialog.getByTestId('chat-create-stage')).toHaveText('正在启动')
  await waitForHerdrCall(page, 'agent_start')
  expect(callsOfKind('agent_prompt')).toHaveLength(0)
  await page.waitForTimeout(300)
  expect(callsOfKind('agent_prompt')).toHaveLength(0)

  // 就绪之后才进入提交阶段；提交响应未确认之前没有成功。
  await expect(dialog.getByTestId('chat-create-stage')).toHaveText('正在提交初始指示', {
    timeout: 10_000,
  })
  await expect(dialog.getByTestId('chat-create-success')).toHaveCount(0)

  await expect(dialog.getByTestId('chat-create-success')).toHaveText(
    '创建成功，初始指示已提交',
    { timeout: 10_000 },
  )
  await expect(dialog.getByTestId('chat-result-name')).toHaveText(previewName)
  await expect(dialog.getByTestId('chat-result-zone')).toHaveText(zones.normal)

  // 外部记录：Tab 名与预览一致；结果里的 Tab / Pane 标识来自受控 Herdr 的响应。
  const tabs = server.herdrTabs()
  expect(tabs).toHaveLength(1)
  expect(tabs[0].label).toBe(previewName)
  expect(tabs[0].cwd).toBe(zones.normal)
  expect(tabs[0].workspace).toBe('w-target')
  await expect(dialog.getByTestId('chat-result-tab')).toHaveText(String(tabs[0].tab_id))
  await expect(dialog.getByTestId('chat-result-pane')).toHaveText(String(tabs[0].pane_id))

  const prompts = callsOfKind('agent_prompt')
  expect(prompts).toHaveLength(1)
  expect(prompts[0].text).toBe('普通任务：检查登录逻辑')

  // 结果留在弹窗；没有自动切换到会话列表。
  await expect(page.getByTestId('tab-files')).toHaveAttribute('aria-selected', 'true')
  await expect(dialog).toBeVisible()
  await dialog.getByTestId('chat-create-close').click()
  await expect(page.getByTestId('chat-creation-dialog')).toHaveCount(0)
})

test('提交被明确拒绝时失败留在弹窗，页面与外部已保留的现场一致', { tag: '@time-contract' }, async ({ page }) => {
  server = await startServer({ herdr: { behavior: { fail_prompt: true } } })
  const zones = await prepareZones(server.baseURL)
  await prepareBot(server.baseURL)
  await page.goto(server.baseURL)

  const dialog = await openChatDialog(page, zones.normal)
  await expect(dialog.getByTestId('chat-name-preview')).toHaveText(/^alpha · 审查 · [0-9a-f]{6}$/)
  await dialog.getByTestId('chat-input').fill('会被拒绝的指示')
  await dialog.getByTestId('chat-create-submit').click()

  const failure = dialog.getByTestId('chat-create-failure')
  await expect(failure).toBeVisible({ timeout: 10_000 })
  await expect(failure).toContainText('提交初始指示')
  await expect(failure).toContainText(
    '现场已存在：agent 已就绪，但初始指示的提交被明确拒绝；不会自动重发。',
  )
  await expect(dialog.getByTestId('chat-create-success')).toHaveCount(0)

  // 外部独立现场：Tab 已创建并保留；提交只到达一次且被拒绝，没有清理动作。
  const tabs = server.herdrTabs()
  expect(tabs).toHaveLength(1)
  await expect(dialog.getByTestId('chat-result-tab')).toHaveText(String(tabs[0].tab_id))
  await expect(dialog.getByTestId('chat-result-pane')).toHaveText(String(tabs[0].pane_id))
  const prompts = callsOfKind('agent_prompt')
  expect(prompts).toHaveLength(1)
  expect(prompts[0].text).toBe('会被拒绝的指示')
  expect(server.herdrCalls().filter((call) => call.kind === 'unsupported')).toHaveLength(0)
  await page.waitForTimeout(300)
  expect(callsOfKind('agent_prompt')).toHaveLength(1)
  expect(server.herdrTabs()).toHaveLength(1)

  // 结果留在弹窗与文件列表；用户关闭前不自动切换会话列表。
  await expect(page.getByTestId('tab-files')).toHaveAttribute('aria-selected', 'true')
  await expect(dialog).toBeVisible()
  await dialog.getByTestId('chat-create-close').click()
  await expect(page.getByTestId('chat-creation-dialog')).toHaveCount(0)
})

test('目标 Workspace 不存在时失败留在弹窗，不假报成功也不自动跳转', async ({ page }) => {
  server = await startServer({
    herdr: {
      behavior: {},
      workspaces: [{ workspace_id: 'w-other', label: 'other', focused: true }],
    },
  })
  const zones = await prepareZones(server.baseURL)
  await prepareBot(server.baseURL)
  await page.goto(server.baseURL)

  const dialog = await openChatDialog(page, zones.normal)
  await expect(dialog.getByTestId('chat-name-preview')).toHaveText(/^alpha · 审查 · [0-9a-f]{6}$/)
  await dialog.getByTestId('chat-input').fill('不会被提交')
  await dialog.getByTestId('chat-create-submit').click()

  const failure = dialog.getByTestId('chat-create-failure')
  await expect(failure).toBeVisible({ timeout: 10_000 })
  await expect(failure).toContainText('查找 Workspace')
  await expect(failure).toContainText('找不到名称恰好为 “bot-playground” 的 Herdr Workspace')
  await expect(dialog.getByTestId('chat-create-success')).toHaveCount(0)
  await expect(dialog.getByTestId('chat-result-tab')).toHaveCount(0)
  await expect(dialog.getByTestId('chat-result-pane')).toHaveCount(0)

  expect(server.herdrTabs()).toHaveLength(0)
  expect(callsOfKind('agent_prompt')).toHaveLength(0)
  await expect(page.getByTestId('tab-files')).toHaveAttribute('aria-selected', 'true')

  await dialog.getByTestId('chat-create-close').click()
  await expect(page.getByTestId('chat-creation-dialog')).toHaveCount(0)
})

test('Skill、固定 prompt 与本次 input 经真实页面合成一次原生 Skill 提交', async ({ page }) => {
  server = await startServer({ herdr: { behavior: {} } })
  const zones = await prepareZones(server.baseURL)

  // 预写的完整样例：prompt 含 slash 文本，仍应作为正文的一部分原样提交。
  const prompt =
    '固定指示第一行\n固定指示第二行：保留 /skill:teach 与 /code-review 字面'
  const input = '本次任务第一行\n第二行带引号 "quoted" 与 & 符号'
  const expected =
    '/skill:code-review 固定指示第一行\n固定指示第二行：保留 /skill:teach 与 /code-review 字面\n\n本次任务第一行\n第二行带引号 "quoted" 与 & 符号'
  await prepareBot(server.baseURL, {
    ...BOT,
    skill: 'code-review',
    prompt,
  })
  await page.goto(server.baseURL)

  const dialog = await openChatDialog(page, zones.normal)
  await expect(dialog.getByTestId('chat-name-preview')).toHaveText(/^alpha · 审查 · [0-9a-f]{6}$/)
  await dialog.getByTestId('chat-input').fill(input)
  await dialog.getByTestId('chat-create-submit').click()

  await expect(dialog.getByTestId('chat-create-success')).toHaveText(
    '创建成功，初始指示已提交',
    { timeout: 10_000 },
  )
  const prompts = callsOfKind('agent_prompt')
  expect(prompts).toHaveLength(1)
  expect(prompts[0].text).toBe(expected)
})

test('Skill、prompt、input 全空仍从页面提交，并由受控 Herdr 拒绝为普通失败', async ({
  page,
}) => {
  server = await startServer({ herdr: { behavior: {} } })
  const zones = await prepareZones(server.baseURL)
  await prepareBot(server.baseURL)
  await page.goto(server.baseURL)

  const dialog = await openChatDialog(page, zones.normal)
  await expect(dialog.getByTestId('chat-name-preview')).toHaveText(/^alpha · 审查 · [0-9a-f]{6}$/)
  // 不填写初始 input；Bot 既没有 Skill，固定 prompt 也为空。
  await expect(dialog.getByTestId('chat-input')).toHaveValue('')
  await dialog.getByTestId('chat-create-submit').click()

  const failure = dialog.getByTestId('chat-create-failure')
  await expect(failure).toBeVisible({ timeout: 10_000 })
  await expect(failure).toContainText('提交初始指示')
  await expect(failure).toContainText('agent prompt must not be empty')
  await expect(dialog.getByTestId('chat-create-success')).toHaveCount(0)

  // 现场保留并可定位；受控 Herdr 确实收到过这次空正文提交，而不是被提前跳过。
  const tabs = server.herdrTabs()
  expect(tabs).toHaveLength(1)
  await expect(dialog.getByTestId('chat-result-tab')).toHaveText(String(tabs[0].tab_id))
  await expect(dialog.getByTestId('chat-result-pane')).toHaveText(String(tabs[0].pane_id))
  const prompts = callsOfKind('agent_prompt')
  expect(prompts).toHaveLength(1)
  expect(prompts[0].text).toBe('')
})
