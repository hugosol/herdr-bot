import { expect, test } from '@playwright/test'
import fs from 'node:fs'
import path from 'node:path'
import { startServer, type E2EServer } from './support'

let server: E2EServer | null = null
test.afterEach(async () => { await server?.cleanup(); server = null })

// S1/S2/S3, P7/P8: real delivered page and HTTP, restart on a real history file,
// observing created Tabs, not just the API's claimed workspace/fallback result.
test('Resume returns to the persisted original workspace and explains deleted-original fallback', async ({ page }) => {
  server = await startServer({ herdr: {} })
  const directory = path.dirname(server.configPath)
  const cwd = path.join(directory, 'Historical cwd with spaces')
  fs.mkdirSync(cwd)
  const native = path.join(cwd, 'conversation.jsonl')
  const bytes = `${JSON.stringify({ type: 'session', id: 'pi-workspace-browser-001', cwd })}\n`
  fs.writeFileSync(native, bytes)
  const reply = await fetch(`${server.baseURL}/api/sessions/events`, {
    method: 'POST', headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ sessionId: 'pi-workspace-browser-001', kind: 'register', data: {
      cwd, agentType: 'pi', sessionName: 'Remembered Workspace', sessionFile: native, originalWorkspaceId: 'w-other',
    } }),
  })
  expect(reply.status).toBe(200)
  await server.restart()
  server.setHerdrBehavior({ native_session: { agent: 'pi', source: 'herdr:pi', kind: 'path', value: native } })
  await page.goto(server.baseURL)
  await page.getByRole('button', { name: '查看历史会话' }).click()
  const row = page.getByRole('row').filter({ hasText: 'Remembered Workspace' })
  const originalResponse = page.waitForResponse(r => r.url().endsWith('/api/chats/resume') && r.request().method() === 'POST')
  await row.getByRole('button', { name: 'Resume', exact: true }).click()
  const original = await (await originalResponse).json()
  expect(original.outcome).toBe('restored')
  expect(original.fallback).toBe(false)
  await expect(row.getByRole('status')).toContainText('已在原 Workspace 恢复')
  expect(server.herdrTabs()[0].workspace).toBe('w-other')
  expect(server.herdrTabs()[0].cwd).toBe(cwd)
  await page.screenshot({ path: test.info().outputPath('original-workspace-restored.png'), fullPage: true })

  // Remove only the isolated controlled workspace; a same-label replacement
  // must not be mistaken for the original persistent ID.
  fs.writeFileSync(path.join(directory, 'herdr', 'workspaces.json'), JSON.stringify([
    { workspace_id: 'w-target', label: 'bot-playground' },
    { workspace_id: 'w-replacement', label: 'other', focused: true },
  ]))
  const fallbackResponse = page.waitForResponse(r => r.url().endsWith('/api/chats/resume') && r.request().method() === 'POST')
  await row.getByRole('button', { name: 'Resume', exact: true }).click()
  const fallback = await (await fallbackResponse).json()
  expect(fallback.outcome).toBe('restored')
  expect(fallback.fallback).toBe(true)
  await expect(row.getByRole('status')).toContainText('原 Workspace 已删除')
  await expect(row.getByRole('status')).toContainText('默认 Workspace')
  expect(server.herdrTabs().map(t => t.workspace)).toEqual(['w-other', 'w-target'])
  await page.screenshot({ path: test.info().outputPath('deleted-original-fallback.png'), fullPage: true })
  await server.restart()
  const history = await (await fetch(`${server.baseURL}/api/sessions/history`)).json()
  expect(history.sessions.map((s: { sessionId: string }) => s.sessionId)).toEqual(['pi-workspace-browser-001'])
  expect(history.sessions[0].originalWorkspaceId).toBe('w-other')
  expect(fs.readFileSync(native, 'utf8')).toBe(bytes)
})
