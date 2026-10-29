import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'

import { expect, test } from '@playwright/test'

import { lanIPv4, startManualServer, type E2EServer, type TaskView } from './support'
import {
  activate,
  expectDialogInsideViewport,
  expectNoHorizontalOverflow,
  jsonPost,
  saveTask,
  scrollToDialogAction,
  zoneRowFor,
} from './ui'

/**
 * 局域网浏览器访问（P45，显式手动验收：npm run test:browser:manual）。
 * 默认浏览器套件不收集此文件；手动配置授权真实网卡监听，子进程继承授权。
 *
 * 服务按交付配置绑定 `0.0.0.0`，真实浏览器从本机网卡 IPv4 打开页面，完成 Task 新增、读取、编辑、
 * 删除与 Task 创建；独立 HTTP 客户端经同一局域网地址读取同一份 Task。页面没有登录、账号或令牌步骤。
 *
 * 这证明的是**局域网地址可达**，不是“非本机来源可写会话”：连接仍来自本机网卡地址。Task 管理与创建
 * 不需要同机来源，而会话事件仍只接受本机来源，两者的区分由 `backend/tests/task_source_boundary.rs`
 * 在公开 HTTP 边界上验证。
 */

const HOST = lanIPv4()

const TASK = { name: '局域网 Bot',
chatName: '{zone.name} · {task.name} · {chat.id}',
agent: 'pi',
model: 'deepseek-flash',
thinking: 'high',
skill: 'code-review', prompt: '局域网第一行\n局域网第二行', extensions: [] }

const EDITED = { name: '局域网 Bot（改）',
chatName: '{zone.name} · {task.name} · {chat.id}',
agent: 'omp',
model: 'gpt-6.1-sol',
thinking: 'max',
skill: 'teach', prompt: '局域网编辑后的第一行\n局域网编辑后的第二行', extensions: [] }

const INPUT = '局域网任务第一行\n局域网任务第二行'

let server: E2EServer | null = null
let zoneRoot: string | null = null

test.use({ viewport: { width: 390, height: 844 }, isMobile: true, hasTouch: true })

test.afterEach(async () => {
  await server?.cleanup()
  server = null
  if (zoneRoot !== null) {
    fs.rmSync(zoneRoot, { recursive: true, force: true })
    zoneRoot = null
  }
})

function taskFields(view: TaskView) {
  const { id: _id, ...fields } = view
  return fields
}

test('局域网地址可管理共享 Task 并从 zone 发起 Chat，不额外要求账号或令牌', async ({ page }) => {
  test.skip(HOST === null, '本机没有可用的非回环 IPv4，无法构造真实局域网地址')
  if (HOST === null) return

  server = await startManualServer({
    listenAddr: '0.0.0.0:0',
    herdr: { behavior: { agent_start_delay_ms: 400, prompt_delay_ms: 400 } },
  })
  const lanURL = server.urlForHost(HOST)
  test.info().annotations.push({ type: '局域网入口', description: lanURL })

  // 不经页面，先确认这个局域网地址真的提供接口。
  const reachable = await fetch(`${lanURL}/api/tasks`)
  expect(reachable.status).toBe(200)

  // 浏览器发出的接口请求不携带任何认证头：本版没有账号、token 或权限系统。
  const apiHeaders: Record<string, string>[] = []
  page.on('request', (request) => {
    if (request.url().startsWith(`${lanURL}/api/`)) {
      apiHeaders.push(request.headers())
    }
  })

  await page.goto(`${lanURL}?layout=expanded`)
  expect(new URL(page.url()).origin).toBe(new URL(lanURL).origin)
  await expect(page.getByRole('heading', { name: 'herdr-bot' })).toBeVisible()
  expect(await page.locator('input[type="password"]').count(), '页面不应要求登录或密码').toBe(0)
  await expectNoHorizontalOverflow(page)

  // 登记一个真实服务器目录；浏览器与接口都经局域网地址访问同一进程。
  zoneRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'herdr-bot lan workflow '))
  const directory = path.join(zoneRoot, 'alpha')
  fs.mkdirSync(directory)
  const listed = await jsonPost(lanURL, '/api/zones', { path: directory })
  const zone = listed.zones[0].path as string

  // Task 新增：经局域网浏览器完成，独立客户端经同一地址读到相同字面值。
  await activate(page.getByTestId('tab-tasks'), true)
  await expect(page.getByTestId('task-empty')).toBeVisible()
  await activate(page.getByTestId('task-create-open'), true)
  const dialog = page.getByTestId('task-dialog')
  await expect(dialog).toBeVisible()
  await expectDialogInsideViewport(dialog)
  await page.getByTestId('task-name').fill(TASK.name)
  await page.getByTestId('task-chat-name').fill(TASK.chatName)
  await page.getByTestId('task-agent').selectOption(TASK.agent)
  await page.getByTestId('task-model').selectOption(TASK.model)
  await page.getByTestId('task-thinking').selectOption(TASK.thinking)
  await page.getByTestId('task-skill').selectOption(TASK.skill)
  await page.getByTestId('task-prompt').fill(TASK.prompt)
  await saveTask(page, true)
  await expect(page.getByTestId('task-dialog')).toHaveCount(0)

  const saved = await (await fetch(`${lanURL}/api/tasks`)).json()
  expect(saved.tasks).toHaveLength(1)
  expect(taskFields(saved.tasks[0])).toEqual(TASK)

  // 编辑：同一局域网路径保存后，独立客户端读到新值。
  const row = page.getByTestId('task-item')
  await activate(row.getByTestId('task-edit'), true)
  await expect(page.getByTestId('task-dialog')).toBeVisible()
  await page.getByTestId('task-name').fill(EDITED.name)
  await page.getByTestId('task-chat-name').fill(EDITED.chatName)
  await page.getByTestId('task-agent').selectOption(EDITED.agent)
  await page.getByTestId('task-model').selectOption(EDITED.model)
  await page.getByTestId('task-thinking').selectOption(EDITED.thinking)
  await page.getByTestId('task-skill').selectOption(EDITED.skill)
  await page.getByTestId('task-prompt').fill(EDITED.prompt)
  await saveTask(page, true)
  await expect(row).toContainText(EDITED.name)

  const edited = await (await fetch(`${lanURL}/api/tasks`)).json()
  expect(edited.tasks).toHaveLength(1)
  expect(taskFields(edited.tasks[0])).toEqual(EDITED)

  // 再保存一个不带 Skill / 固定 prompt 的创建用 Task：本次只验证局域网创建可达，
  // 初始化正文合成属于 04 的业务测试，不在本票重复认领。
  await activate(page.getByTestId('task-create-open'), true)
  await expect(page.getByTestId('task-dialog')).toBeVisible()
  await page.getByTestId('task-name').fill('局域网创建')
  await page.getByTestId('task-agent').selectOption('omp')
  await page.getByTestId('task-model').selectOption('default')
  await page.getByTestId('task-thinking').selectOption('default')
  await page.getByTestId('task-skill').selectOption('')
  await page.getByTestId('task-prompt').fill('')
  await saveTask(page, true)
  await expect(page.getByTestId('task-item')).toHaveCount(2)

  // 从 zone 发起 Chat：预览、提交、阶段、成功与定位都在局域网页面完成。
  await activate(page.getByTestId('tab-files'), true)
  const zoneRow = zoneRowFor(page, zone)
  await activate(zoneRow.getByTestId('zone-create-chat'), true)
  const creation = page.getByTestId('chat-creation-dialog')
  await expect(creation).toBeVisible()
  await expect(creation.getByTestId('chat-zone')).toContainText(zone)
  await creation.getByTestId('task').selectOption({ label: '局域网创建（omp）' })
  const preview = creation.getByTestId('chat-name-preview')
  await expect(preview).toHaveText(/^alpha · 局域网创建 · [0-9a-f]{6}$/)
  const previewName = (await preview.textContent()) ?? ''

  await creation.locator('#chat-workspace-target').selectOption('default')
  await creation.getByTestId('chat-input').fill(INPUT)
  await activate(creation.getByTestId('chat-create-submit'), true)
  await expect(creation.getByTestId('chat-create-stage')).toHaveText('正在启动')
  await expect(creation.getByTestId('chat-create-stage')).toHaveText('正在提交初始指示', {
    timeout: 10_000,
  })
  await expect(creation.getByTestId('chat-create-success')).toHaveText(
    '创建成功，初始指示已提交',
    { timeout: 10_000 },
  )
  await expect(creation.getByTestId('chat-result-name')).toHaveText(previewName)
  await expectDialogInsideViewport(creation)
  await expectNoHorizontalOverflow(page)

  const tabs = server.herdrTabs()
  expect(tabs).toHaveLength(1)
  expect(tabs[0].label).toBe(previewName)
  expect(tabs[0].cwd).toBe(zone)
  const prompts = server.herdrCalls().filter((call) => call.kind === 'agent_prompt')
  expect(prompts).toHaveLength(1)
  expect(prompts[0].text).toBe(INPUT)

  // 关闭后仍在文件列表；删除 Task 经局域网地址完成，独立客户端确认已不可用。
  const close = creation.getByTestId('chat-create-close')
  await scrollToDialogAction(creation, close)
  await close.tap()
  await expect(page.getByTestId('chat-creation-dialog')).toHaveCount(0)

  await activate(page.getByTestId('tab-tasks'), true)
  await page.getByTestId('task-item').getByTestId('task-delete').first().tap()
  await page.getByTestId('task-delete-confirm').tap()
  await expect(page.getByTestId('task-item')).toHaveCount(1)
  await page.getByTestId('task-item').getByTestId('task-delete').first().tap()
  await page.getByTestId('task-delete-confirm').tap()
  await expect(page.getByTestId('task-empty')).toBeVisible()
  const removed = await fetch(
    `${lanURL}/api/tasks/${encodeURIComponent(edited.tasks[0].id)}`,
  )
  expect(removed.status).toBe(404)

  // 整个流程没有认证头，也没有跳转到登录页。
  for (const headers of apiHeaders) {
    expect(headers.authorization, '浏览器请求不应需要 Authorization').toBeUndefined()
  }
  expect(new URL(page.url()).origin).toBe(new URL(lanURL).origin)
})
