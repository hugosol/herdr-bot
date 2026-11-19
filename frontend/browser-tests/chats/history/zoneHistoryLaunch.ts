import { expect, type Page } from '@playwright/test'
import fs from 'node:fs'
import type { E2EServer } from '../../support/support'
import { zoneRowFor } from '../../support/ui'

export async function beginZoneChat(page: Page, server: E2EServer, zone: string) {
  await page.goto(server.baseURL)
  await page.getByTestId('tab-files').click()
  await zoneRowFor(page, zone).getByTestId('zone-create-chat').click()
  const dialog = page.getByTestId('chat-creation-dialog')
  await dialog.locator('#chat-workspace-target').selectOption('default')
  await dialog.getByTestId('chat-input').fill('/settings')
  const submit = dialog.getByTestId('chat-create-submit')
  await expect(submit).toBeEnabled()
  const previewedName = await dialog.getByTestId('chat-name-preview').textContent()
  const accepted = page.waitForResponse(r => r.url().endsWith('/api/chats') && r.request().method() === 'POST')
  await submit.click()
  const response = await accepted
  expect(response.status(), await response.text()).toBe(202)
  const { id, name } = await response.json() as { id: string; name: string }
  expect(name).toBe(previewedName)
  await expect(dialog.getByTestId('chat-create-stage')).toHaveText('正在初始化输入')
  await expect(dialog.getByTestId('chat-create-success')).toHaveCount(0)
  expect(server.herdrCalls().filter(c => c.kind === 'agent_prompt')).toHaveLength(0)
  return { dialog, id, name }
}

export function launchReporter(server: E2EServer, agentType: 'omp' | 'pi', cwd: string, sessionId: string, sessionFile?: string) {
  const argv = server.herdrCalls().find(c => c.kind === 'tab_create')?.argv as string[]
  const env = (key: string) => argv.find(arg => arg.startsWith(`${key}=`))?.slice(key.length + 1)
  const launchId = env('HERDR_BOT_LAUNCH_ID')
  const targetId = env('HERDR_BOT_LAUNCH_TARGET_ID')
  expect(launchId).toBeTruthy()
  expect(targetId).toBeTruthy()
  return async (kind: 'launch_identity' | 'launch_ready', status = 200) => {
    const response = await fetch(`${server.baseURL}/api/sessions/events`, {
      method: 'POST', headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ kind, sessionId, data: { launchId, targetId, agentType, cwd, pid: 43127, sessionFile } }),
    })
    expect(response.status, await response.text()).toBe(status)
  }
}

export async function readCreation(server: E2EServer, id: string) {
  const response = await fetch(`${server.baseURL}/api/chats/${id}`)
  expect(response.status).toBe(200)
  return response.json()
}

export async function keepControlSessionOnline(server: E2EServer) {
  // A control session should outlast the real launch deadline without heartbeats
  // so either guard can regress independently of the other.
  const config = fs.readFileSync(server.configPath, 'utf8')
  fs.writeFileSync(server.configPath, config.replace('[sessions]', '[sessions]\nonline_timeout_seconds = 60'))
  await server.restart()
}

export async function currentSnapshot(server: E2EServer) {
  const response = await fetch(`${server.baseURL}/api/sessions/events`)
  expect(response.status).toBe(200)
  const reader = response.body!.getReader()
  const decoder = new TextDecoder()
  let text = ''
  try {
    while (!text.includes('\n\n')) {
      const chunk = await reader.read()
      expect(chunk.done).toBe(false)
      text += decoder.decode(chunk.value, { stream: true }).replace(/\r/g, '')
    }
    return JSON.parse(text.split('\n').find(line => line.startsWith('data:'))!.slice(5))
  } finally { await reader.cancel() }
}
