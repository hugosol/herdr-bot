import { expect, test } from '@playwright/test'
import fs from 'node:fs'
import path from 'node:path'
import { startServer, type E2EServer } from './support'

let server: E2EServer | null = null
test.afterEach(async () => { await server?.cleanup(); server = null })

// S1/S2/S3: no browser mocks; the page sends the real Resume request and
// receives the same confirmed/failure result an independent HTTP caller sees.
test('Pi Resume shows confirmed default-workspace fallback and visible failure, never a Bot launch', async ({ page }) => {
  server = await startServer({ herdr: {} })
  const cwd = path.join(path.dirname(server.configPath), 'Pi history cwd with spaces')
  fs.mkdirSync(cwd)
  const native = path.join(cwd, 'old pi conversation.jsonl')
  fs.writeFileSync(native, `${JSON.stringify({ type: 'session', id: 'pi-browser-exact', cwd })}\n`)
  const bot = await fetch(`${server.baseURL}/api/bots`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ name: 'Different Bot', chatName: 'New Bot Chat', agent: 'pi', model: 'gpt-6.1-sol', thinking: 'high', prompt: 'BOT INITIAL MUST NEVER BE SUBMITTED', extensions: [] }) })
  expect(bot.status).toBe(200)
  const register = await fetch(`${server.baseURL}/api/sessions/events`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ sessionId: 'pi-browser-exact', kind: 'register', data: { cwd, agentType: 'pi', sessionName: 'Historical Pi', sessionFile: native } }) })
  expect(register.status).toBe(200)
  server.setHerdrBehavior({ native_session: { agent: 'pi', source: 'herdr:pi', kind: 'path', value: native } })
  await page.goto(server.baseURL)
  await page.getByRole('button', { name: '查看历史会话' }).click()
  const row = page.getByRole('row').filter({ hasText: 'Historical Pi' })
  const response = page.waitForResponse((r) => r.url().endsWith('/api/chats/resume') && r.request().method() === 'POST')
  await row.getByRole('button', { name: 'Resume', exact: true }).click()
  const result = await (await response).json()
  expect(result.outcome).toBe('restored')
  await expect(row.getByRole('status')).toContainText('默认 Workspace')
  await expect(row.getByRole('status')).toContainText('已确认恢复同一 Pi 原生对话')
  await expect(row.getByRole('status')).toContainText('不表示 Windows 窗口已置前')
  const calls = server.herdrCalls()
  expect(calls.some(call => call.kind === 'agent_prompt')).toBe(false)
  for (const value of Object.values(result.target)) await expect(row.locator('dl')).toContainText(String(value))
  expect(server.herdrTabs()[0].cwd).toBe(cwd)
  expect((calls.find((c) => c.kind === 'agent_start')!.args as string[])).toContain('--session')
  expect((calls.find((c) => c.kind === 'agent_start')!.args as string[])).not.toContain('--model')
  await page.screenshot({ path: test.info().outputPath('pi-resume-confirmed-fallback.png'), fullPage: true })

  server.setHerdrBehavior({ fail_workspace_list: true })
  await row.getByRole('button', { name: 'Resume', exact: true }).click()
  await expect(row.getByRole('alert')).toContainText('workspace')
  await expect(row.getByRole('alert')).toContainText('尚未确认恢复成功')
  expect(server.herdrTabs()).toHaveLength(1)
  await page.screenshot({ path: test.info().outputPath('pi-resume-failed.png'), fullPage: true })
})
