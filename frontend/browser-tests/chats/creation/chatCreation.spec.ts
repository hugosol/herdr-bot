import { execFileSync } from 'node:child_process'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'

import { expect, test } from '@playwright/test'
import type { Page } from '@playwright/test'

import { startServer, type E2EServer } from '../../support/support'
import { zoneRowFor } from '../../support/ui'

const TASK = {
  name: '审查',
  chatName: '{zone.name} · {task.name} · {chat.id}',
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

  await jsonPost(baseURL, '/api/zones', { id: 'test/normal', path: normal })
  const second = await jsonPost(baseURL, '/api/zones', { id: 'test/git', path: git })
  return {
    normal: 'test/normal',
    git: 'test/git',
    normalPath: second.zones.find((zone: { id: string }) => zone.id === 'test/normal').path as string,
  }
}

async function prepareTask(baseURL: string, task: Record<string, unknown> = TASK) {
  const saved = await jsonPost(baseURL, '/api/tasks', task)
  return saved.tasks[saved.tasks.length - 1] as { id: string }
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

test('App 创建在默认 Space 降级后显示实际位置并等待原生 ready', async ({ page }, testInfo) => {
  server = await startServer({
    herdr: { behavior: { app_spaces: [{ id: 'space-default', name: 'bot-playground', tabs: [] }] } },
  })
  await server.stop()
  const directory = path.dirname(server.configPath)
  const host = path.join(directory, 'herdr', 'herdr-fixture.exe')
  fs.writeFileSync(path.join(directory, 'herdr', 'chat-app-mode'), '')
  let config = fs.readFileSync(server.configPath, 'utf8')
    .replace('[herdr]', `[herdr]\nbackend = 'app'\napp_executable = '${host}'\ncore_executable = '${host}'`)
  if (process.env.HERDR_BOT_ROUTING_STATIC !== undefined) {
    config = config.replace(/^static_directory = .*$/m, `static_directory = '${process.env.HERDR_BOT_ROUTING_STATIC}'`)
  }
  fs.writeFileSync(server.configPath, config)
  await server.restart()
  const zones = await prepareZones(server.baseURL)
  await prepareTask(server.baseURL)
  await page.goto(server.baseURL)
  const dialog = await openChatDialog(page, zones.normal, true)
  await dialog.getByTestId('chat-input').fill('App placement business input')
  await dialog.getByTestId('chat-create-submit').click()
  await expect(dialog.getByTestId('chat-create-stage')).toHaveText('正在初始化输入')
  await expect(dialog).toContainText('bot-playground（space-default）')
  await expect(dialog).toContainText('位置回退')
  expect(callsOfKind('core_send')).toHaveLength(0)
  const tabs = fs.readFileSync(path.join(directory, 'herdr', 'app-tabs.jsonl'), 'utf8')
    .trim().split('\n').map(line => JSON.parse(line))
  expect(tabs).toHaveLength(1)
  expect(tabs[0].space).toBe('space-default')
  const argv = tabs[0].options.argv as string[]
  const data = { launchId: argv[6], targetId: argv[7], agentType: 'omp', cwd: zones.normalPath, pid: 43127 }
  await jsonPost(server.baseURL, '/api/sessions/events', { kind: 'launch_identity', sessionId: 'browser-app-native', data })
  expect(callsOfKind('core_send')).toHaveLength(0)
  await jsonPost(server.baseURL, '/api/sessions/events', { kind: 'launch_ready', sessionId: 'browser-app-native', data })
  await expect(dialog.getByTestId('chat-create-success')).toHaveText('创建成功，初始指示已提交')
  await expect(dialog).toContainText('bot-playground（space-default）')
  const sent = callsOfKind('core_send')
  expect(sent).toHaveLength(1)
  expect(sent[0].terminal).toBe(tabs[0].terminal)
  expect(sent[0].text).toBe('App placement business input')
  const actualSpace = dialog.getByText('bot-playground（space-default）', { exact: true })
  await actualSpace.scrollIntoViewIfNeeded()
  await expect(actualSpace).toBeVisible()
  await page.screenshot({ path: testInfo.outputPath('app-space-fallback.png'), fullPage: true })
})

test('从普通目录与 Git 工作树对应的 zone 行发起 Chat：预览名称、阶段、成功与定位', { tag: '@time-contract' }, async ({
  page,
}) => {
  server = await startServer({
    herdr: {
      workspaces: [{ workspace_id: 'w-target', label: 'alpha' }, { workspace_id: 'w-beta', label: 'beta' }],
      behavior: { manual_launch_events: true, agent_start_delay_ms: 2_000, prompt_delay_ms: 1_500 },
    },
  })
  const zones = await prepareZones(server.baseURL)
  await prepareTask(server.baseURL)
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
  expect((await server.getTasks()).tasks[0].chatName).toBe(TASK.chatName)

  await dialog.getByTestId('chat-input').fill('普通任务：检查登录逻辑')
  await dialog.getByTestId('chat-create-submit').click()

  // S1/P50: a Tab and agent-start request do not authorize input. The page
  // must show the backend-owned initialization phase, not infer submission.
  await waitForHerdrCall(page, 'agent_start')
  await expect(dialog.getByTestId('chat-create-stage')).toHaveText('正在初始化输入')
  expect(callsOfKind('agent_prompt')).toHaveLength(0)
  const argv = callsOfKind('tab_create')[0].argv as string[]
  const env = (key: string) => argv.find(arg => arg.startsWith(`${key}=`))?.slice(key.length + 1)
  const data = { launchId: env('HERDR_BOT_LAUNCH_ID'), targetId: env('HERDR_BOT_LAUNCH_TARGET_ID'), agentType: 'omp', cwd: zones.normalPath, pid: 43127 }
  await jsonPost(server.baseURL, '/api/sessions/events', { kind: 'launch_identity', sessionId: 'browser-owned-native', data })
  await expect(dialog.getByTestId('chat-create-stage')).toHaveText('正在初始化输入')
  expect(callsOfKind('agent_prompt')).toHaveLength(0)
  await jsonPost(server.baseURL, '/api/sessions/events', { kind: 'launch_ready', sessionId: 'browser-owned-native', data })

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
  await expect(dialog.getByTestId('chat-result-zone')).toHaveText(zones.normalPath)

  // 外部记录：Tab 名与预览一致；结果里的 Tab / Pane 标识来自受控 Herdr 的响应。
  const tabs = server.herdrTabs()
  expect(tabs).toHaveLength(1)
  expect(tabs[0].label).toBe(previewName)
  expect(tabs[0].cwd).toBe(zones.normalPath)
  expect(tabs[0].workspace).toBe('w-target')
  await expect(dialog.getByTestId('chat-result-tab')).toHaveText(String(tabs[0].tab_id))
  await expect(dialog.getByTestId('chat-result-pane')).toHaveText(String(tabs[0].pane_id))

  const prompts = callsOfKind('agent_prompt')
  expect(prompts).toHaveLength(1)
  expect(prompts[0].text).toMatch(/^普通任务：检查登录逻辑(?:\n\n|$)/)

  // 结果留在弹窗；没有自动切换到会话列表。
  await expect(page.getByTestId('tab-files')).toHaveAttribute('aria-selected', 'true')
  await expect(dialog).toBeVisible()
  await dialog.getByTestId('chat-create-close').click()
  await expect(page.getByTestId('chat-creation-dialog')).toHaveCount(0)
})

function terminalInputs(base: E2EServer): Record<string, unknown>[] {
  const log = path.join(path.dirname(base.configPath), 'herdr', 'submitted.jsonl')
  return fs.existsSync(log)
    ? fs.readFileSync(log, 'utf8').trim().split('\n').map(line => JSON.parse(line))
    : []
}

test('P20 P25 P52 刷新等待中的页面不重建，原目标仅收到一次指示', { tag: '@time-contract' }, async ({ page }) => {
  const backend = server = await startServer({ herdr: { behavior: { block_prompt: true } } })
  const zones = await prepareZones(backend.baseURL)
  await prepareTask(backend.baseURL)
  await page.goto(backend.baseURL)
  const dialog = await openChatDialog(page, zones.normal)
  const input = '刷新不重复创建'
  await dialog.getByTestId('chat-input').fill(input)
  let creates = 0
  page.on('request', request => {
    if (request.method() === 'POST' && new URL(request.url()).pathname === '/api/chats') creates++
  })
  const receipt = page.waitForResponse(response => response.status() === 202 && new URL(response.url()).pathname === '/api/chats')
  await dialog.getByTestId('chat-create-submit').click()
  const { id } = await (await receipt).json()
  await expect(dialog.getByTestId('chat-create-stage')).toContainText('初始指示未提交，1 秒后重试（最多重试 5 次）')
  await page.reload()
  await expect(page.getByTestId('chat-creation-dialog')).toHaveCount(0)
  expect(terminalInputs(backend)).toEqual([])
  backend.setHerdrBehavior({})
  await expect.poll(async () => {
    const result = await page.request.get(`${backend.baseURL}/api/chats/${encodeURIComponent(id)}`)
    return result.json()
  }, { timeout: 10_000 }).toMatchObject({ id, stage: 'succeeded', inputSubmitted: true })
  expect(creates).toBe(1)
  expect(backend.herdrTabs()).toHaveLength(1)
  expect(callsOfKind('agent_start')).toHaveLength(1)
  const attempts = callsOfKind('agent_prompt')
  expect(attempts.length).toBeGreaterThanOrEqual(2)
  expect(attempts.length).toBeLessThanOrEqual(6)
  for (const attempt of attempts) expect(attempt.target).toBe(attempts[0].target)
  expect(terminalInputs(backend)).toEqual([{ target: attempts[0].target, text: input }])
})

test('P28 P50 原进程被替换时页面显示取消且不向新 owner 提交', { tag: '@time-contract' }, async ({ page }) => {
  const backend = server = await startServer({ herdr: { behavior: { block_prompt: true } } })
  const zones = await prepareZones(backend.baseURL)
  await prepareTask(backend.baseURL)
  await page.goto(backend.baseURL)
  const dialog = await openChatDialog(page, zones.normal)
  await dialog.getByTestId('chat-input').fill('不得送给替换进程')
  await dialog.getByTestId('chat-create-submit').click()
  await expect(dialog.getByTestId('chat-create-stage')).toContainText('初始指示未提交，1 秒后重试（最多重试 5 次）')
  backend.setHerdrBehavior({ launch_pid: 43128 })
  const failure = dialog.getByTestId('chat-create-failure')
  await expect(failure).toContainText('已取消重试')
  await expect(failure).toContainText('初始指示未提交，不会自动重发')
  expect(callsOfKind('agent_prompt')).toHaveLength(1)
  expect(callsOfKind('agent_start')).toHaveLength(1)
  expect(backend.herdrTabs()).toHaveLength(1)
  expect(terminalInputs(backend)).toEqual([])
})

test('P27 P50 外部输入已发生但响应不明确，页面不声称未发送', { tag: '@time-contract' }, async ({ page }) => {
  const backend = server = await startServer({ herdr: { behavior: { invalid_prompt_response: true } } })
  const zones = await prepareZones(backend.baseURL)
  await prepareTask(backend.baseURL)
  await page.goto(backend.baseURL)
  const dialog = await openChatDialog(page, zones.normal)
  await dialog.getByTestId('chat-input').fill('已写入但没有确认')
  await dialog.getByTestId('chat-create-submit').click()
  const failure = dialog.getByTestId('chat-create-failure')
  await expect(failure).toContainText('初始指示的投递结果未确认')
  await expect(failure).not.toContainText('初始指示未提交')
  await expect(dialog.getByTestId('chat-create-success')).toHaveCount(0)
  expect(terminalInputs(backend)).toEqual([{ target: callsOfKind('agent_prompt')[0].target, text: '已写入但没有确认' }])
  expect(callsOfKind('agent_prompt')).toHaveLength(1)
  expect(backend.herdrTabs()).toHaveLength(1)
})

test('P24 P25 P26 P50 明确未提交时页面显示重试等待，第六次阻塞后停止', { tag: '@time-contract' }, async ({ page }) => {
  server = await startServer({ herdr: { behavior: { block_prompt: true } } })
  const zones = await prepareZones(server.baseURL)
  await prepareTask(server.baseURL)
  await page.goto(server.baseURL)

  const dialog = await openChatDialog(page, zones.normal)
  await expect(dialog.getByTestId('chat-name-preview')).toHaveText(/^alpha · 审查 · [0-9a-f]{6}$/)
  await dialog.getByTestId('chat-input').fill('会被拒绝的指示')
  await dialog.getByTestId('chat-create-submit').click()

  const stage = dialog.getByTestId('chat-create-stage')
  await expect(stage).toContainText(/初始指示未提交，1 秒后重试（最多重试 5 次）：.*controlled modal/)
  expect(terminalInputs(server)).toEqual([])
  await expect(dialog.getByTestId('chat-create-submit')).toBeDisabled()
  await page.screenshot({ path: test.info().outputPath('initial-input-retrying.png'), fullPage: true })

  const failure = dialog.getByTestId('chat-create-failure')
  await expect(failure).toContainText('controlled modal', { timeout: 10_000 })
  await expect(failure).toContainText('初始指示未提交，不会自动重发')
  await expect(dialog.getByTestId('chat-create-success')).toHaveCount(0)
  await page.screenshot({ path: test.info().outputPath('initial-input-retries-exhausted.png'), fullPage: true })

  // The external host rejected all six attempts at the same target without creating a new Tab.
  const tabs = server.herdrTabs()
  expect(tabs).toHaveLength(1)
  await expect(dialog.getByTestId('chat-result-tab')).toHaveText(String(tabs[0].tab_id))
  await expect(dialog.getByTestId('chat-result-pane')).toHaveText(String(tabs[0].pane_id))
  const prompts = callsOfKind('agent_prompt')
  expect(prompts).toHaveLength(6)
  expect(prompts[0].text).toMatch(/^会被拒绝的指示(?:\n\n|$)/)
  for (const [index, prompt] of prompts.slice(1).entries()) {
    expect(prompt.text).toBe(prompts[0].text)
    expect(prompt.target).toBe(prompts[0].target)
    expect(Number(prompt.at) - Number(prompts[index].at)).toBeGreaterThanOrEqual(1_000)
  }
  expect(server.herdrCalls().filter((call) => call.kind === 'unsupported')).toHaveLength(0)
  expect(callsOfKind('agent_start')).toHaveLength(1)
  expect(server.herdrTabs()).toHaveLength(1)
  expect(terminalInputs(server)).toEqual([])

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
  await prepareTask(server.baseURL)
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

  // 预写完整业务正文：原生 Skill 必须在消息开头，slash 文本与多行 input 保真；不锁定后附系统指引措辞。
  const prompt =
    '固定指示第一行\n固定指示第二行：保留 /skill:teach 与 /code-review 字面'
  const input = '本次任务第一行\n第二行带引号 "quoted" 与 & 符号'
  const expected =
    /^\/skill:code-review 固定指示第一行\n固定指示第二行：保留 \/skill:teach 与 \/code-review 字面\n\n本次任务第一行\n第二行带引号 "quoted" 与 & 符号(?:\n\n|$)/
  await prepareTask(server.baseURL, {
    ...TASK,
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
  expect(prompts[0].text).toMatch(expected)
})
