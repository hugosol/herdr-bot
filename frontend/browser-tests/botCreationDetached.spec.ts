import { execFileSync } from 'node:child_process'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'

import { expect, test } from '@playwright/test'
import type { Locator, Page } from '@playwright/test'

import { startServer, type E2EServer } from './support'
import { zoneRowFor } from './ui'

const TEMPLATE = {
  name: '审查',
  botName: '{zone.name} · {template.name} · {bot.id}',
  agent: 'omp',
  model: 'default',
  thinking: 'default',
  skill: null,
  prompt: '',
}

type CreationView = {
  id: string
  stage: string
  inputSubmitted: boolean
  name: string
}

let server: E2EServer | null = null
let zoneRoot: string | null = null

test.afterEach(async () => {
  await server?.cleanup()
  server = null
  if (zoneRoot !== null) {
    fs.rmSync(zoneRoot, { recursive: true, force: true })
    zoneRoot = null
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

/** 预建普通目录并登记到真实服务。 */
async function prepareZone(baseURL: string) {
  zoneRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'herdr-bot detach zones '))
  const zone = path.join(zoneRoot, 'alpha')
  fs.mkdirSync(zone)
  const registered = await jsonPost(baseURL, '/api/zones', { path: zone })
  const paths: string[] = registered.zones.map((entry: { path: string }) => entry.path)
  return paths.find((candidate) => candidate.includes('alpha')) ?? zone
}

async function prepareTemplate(baseURL: string) {
  const saved = await jsonPost(baseURL, '/api/bot-templates', TEMPLATE)
  return saved.templates[0] as { id: string }
}

async function openBotDialog(page: Page, zonePath: string) {
  await page.getByTestId('tab-files').click()
  const row = zoneRowFor(page, zonePath)
  await expect(row).toHaveCount(1)
  await row.getByTestId('zone-create-bot').click()
  const dialog = page.getByTestId('bot-creation-dialog')
  await expect(dialog).toBeVisible()
  return dialog
}

/** 等待一次创建进入指定阶段；返回完整的公开状态。 */
async function waitForCreationStage(
  baseURL: string,
  id: string,
  stage: string,
  timeout = 20_000,
): Promise<CreationView> {
  const deadline = Date.now() + timeout
  while (Date.now() < deadline) {
    const response = await fetch(`${baseURL}/api/bots/${encodeURIComponent(id)}`)
    if (response.ok) {
      const view = (await response.json()) as CreationView
      if (view.stage === stage) return view
      if (view.stage === 'failed') {
        throw new Error(`创建在到达 ${stage} 之前失败：${JSON.stringify(view)}`)
      }
    }
    const { promise, resolve } = Promise.withResolvers<void>()
    setTimeout(resolve, 100)
    await promise
  }
  throw new Error(`创建没有在时限内到达 ${stage}：${id}`)
}

function callsOfKind(kind: string): Record<string, unknown>[] {
  return (server?.herdrCalls() ?? []).filter((call) => call.kind === kind)
}

function tabs(): Record<string, unknown>[] {
  return server?.herdrTabs() ?? []
}

/** 从页面提交一次创建，并返回后端 202 回执里的公开状态（页面自己的观察结果不算数）。 */
async function submitAndCapture(page: Page, dialog: Locator, input: string) {
  await dialog.getByTestId('bot-input').fill(input)
  const accepted = page.waitForResponse(
    (response) =>
      response.url().endsWith('/api/bots') &&
      response.request().method() === 'POST' &&
      response.status() === 202,
  )
  await dialog.getByTestId('bot-create-submit').click()
  const response = await accepted
  return (await response.json()) as CreationView
}

test('浏览器断线后，已接受的创建继续完成且页面不假报成功', async ({ page, context }) => {
  server = await startServer({
    herdr: { behavior: { agent_start_delay_ms: 2_500, prompt_delay_ms: 500 } },
  })
  const zone = await prepareZone(server.baseURL)
  await prepareTemplate(server.baseURL)
  await page.goto(server.baseURL)

  const dialog = await openBotDialog(page, zone)
  const accepted = await submitAndCapture(page, dialog, '断线后继续的任务')
  await expect(dialog.getByTestId('bot-create-stage')).toHaveText('正在启动')
  await expect
    .poll(() => callsOfKind('agent_start').length, { timeout: 10_000 })
    .toBe(1)
  await expect(dialog.getByTestId('bot-create-success')).toHaveCount(0)

  // 目标尚未就绪时断开观察：页面失去观察，但创建责任仍在后端。
  await context.setOffline(true)
  await expect(dialog.getByTestId('bot-observation-error')).toBeVisible({ timeout: 10_000 })
  await expect(dialog.getByTestId('bot-create-success')).toHaveCount(0)

  // 独立客户端（不经过这个离线页面）查询到同一次创建完成，且只有一次提交。
  const finished = await waitForCreationStage(server.baseURL, accepted.id, 'succeeded')
  expect(finished.inputSubmitted).toBe(true)
  expect(finished.name).toBe(accepted.name)
  await expect(dialog.getByTestId('bot-create-success')).toHaveCount(0)

  const prompts = callsOfKind('agent_prompt')
  expect(prompts).toHaveLength(1)
  expect(prompts[0].text).toBe('断线后继续的任务')
  expect(tabs()).toHaveLength(1)
  expect(tabs()[0].label).toBe(accepted.name)
  expect(
    (server.herdrCalls() ?? []).filter((call) => call.kind === 'unsupported'),
  ).toHaveLength(0)

  await context.setOffline(false)
})

test('刷新页面只失去观察，不取消创建，也不承诺找回结果', async ({ page }) => {
  server = await startServer({
    herdr: { behavior: { agent_start_delay_ms: 200, prompt_delay_ms: 2_500 } },
  })
  const zone = await prepareZone(server.baseURL)
  await prepareTemplate(server.baseURL)
  await page.goto(server.baseURL)

  const dialog = await openBotDialog(page, zone)
  const accepted = await submitAndCapture(page, dialog, '刷新后继续的任务')
  await expect(dialog.getByTestId('bot-create-stage')).toHaveText(
    '正在提交初始指示',
    { timeout: 10_000 },
  )
  await expect(dialog.getByTestId('bot-create-success')).toHaveCount(0)

  // 提交尚未确认时刷新：页面不再持有这次结果，也不显示成功。
  await page.reload()
  await expect(page.getByTestId('bot-creation-dialog')).toHaveCount(0)
  await expect(page.getByText('创建成功，初始指示已提交')).toHaveCount(0)

  // 后端仍完成同一次创建；刷新没有引起重复提交、重建或关闭。
  const finished = await waitForCreationStage(server.baseURL, accepted.id, 'succeeded')
  expect(finished.inputSubmitted).toBe(true)
  const prompts = callsOfKind('agent_prompt')
  expect(prompts).toHaveLength(1)
  expect(prompts[0].text).toBe('刷新后继续的任务')
  expect(tabs()).toHaveLength(1)
  expect(tabs()[0].label).toBe(accepted.name)
  await page.waitForTimeout(300)
  expect(callsOfKind('agent_prompt')).toHaveLength(1)
  expect(tabs()).toHaveLength(1)
})

test('关闭页面只失去观察，已接受的创建仍在后端完成', async ({ page }) => {
  server = await startServer({
    herdr: { behavior: { agent_start_delay_ms: 2_000, prompt_delay_ms: 400 } },
  })
  const zone = await prepareZone(server.baseURL)
  await prepareTemplate(server.baseURL)
  await page.goto(server.baseURL)

  const dialog = await openBotDialog(page, zone)
  const accepted = await submitAndCapture(page, dialog, '关闭页面后继续的任务')
  await expect(dialog.getByTestId('bot-create-stage')).toHaveText('正在启动')
  await expect
    .poll(() => callsOfKind('agent_start').length, { timeout: 10_000 })
    .toBe(1)
  await expect(dialog.getByTestId('bot-create-success')).toHaveCount(0)

  await page.close()

  const finished = await waitForCreationStage(server.baseURL, accepted.id, 'succeeded')
  expect(finished.inputSubmitted).toBe(true)
  const prompts = callsOfKind('agent_prompt')
  expect(prompts).toHaveLength(1)
  expect(prompts[0].text).toBe('关闭页面后继续的任务')
  expect(tabs()).toHaveLength(1)
  expect(tabs()[0].label).toBe(accepted.name)
  expect(
    (server.herdrCalls() ?? []).filter((call) => call.kind === 'unsupported'),
  ).toHaveLength(0)
})
