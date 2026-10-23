import { expect, test } from '@playwright/test'
import fs from 'node:fs'
import path from 'node:path'
import { startServer, type E2EServer } from './support'

let server: E2EServer | null = null
test.afterEach(async () => { await server?.cleanup(); server = null })

// P14/P15: S1/S2/S3, original Workspace is present, only the file is missing.
// A silent ID fallback would still restore, but would omit the known reason and
// selected exact identity from the delivered page. Start refusal must stay failed.
test('unavailable Pi file visibly selects the exact ID and retains degradation on start refusal', async ({ page }) => {
  server = await startServer({ herdr: {} })
  const cwd = path.join(path.dirname(server.configPath), 'exact ID cwd')
  fs.mkdirSync(cwd)
  const missing = path.join(cwd, 'unavailable-native.jsonl')
  const register = await fetch(`${server.baseURL}/api/sessions/events`, {
    method: 'POST', headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ sessionId: 'pi-page-exact-id', kind: 'register', data: {
      cwd, agentType: 'pi', sessionName: 'Exact ID fallback', sessionFile: missing, originalWorkspaceId: 'w-target',
    } }),
  })
  expect(register.status).toBe(200)
  server.setHerdrBehavior({ fail_agent_start: true })
  await page.goto(server.baseURL)
  await page.getByRole('button', { name: '查看历史会话' }).click()
  const row = page.getByRole('row').filter({ hasText: 'Exact ID fallback' })
  const response = page.waitForResponse(r => r.url().endsWith('/api/chats/resume') && r.request().method() === 'POST')
  await row.getByRole('button', { name: 'Resume', exact: true }).click()
  const result = await (await response).json()
  expect(result.outcome).toBe('failed')
  expect(result.stage).toBe('start')
  expect(result.degradations).toHaveLength(1)
  expect(result.degradations[0].element).toBe('session-file')
  await expect(row.getByRole('alert')).toContainText(missing)
  await expect(row.getByRole('alert')).toContainText('精确原生 ID pi-page-exact-id')
  await expect(row.getByRole('alert')).toContainText('尚未确认恢复成功')
  expect(server.herdrTabs()).toHaveLength(1)
  const starts = server.herdrCalls().filter(call => call.kind === 'agent_start')
  expect(starts).toHaveLength(1)
  expect(starts[0].args).toEqual(expect.arrayContaining(['--session', 'pi-page-exact-id']))
  expect(server.herdrCalls().some(call => call.kind === 'agent_prompt')).toBe(false)
  expect(fs.existsSync(missing)).toBe(false)
  await page.screenshot({ path: test.info().outputPath('exact-id-fallback-start-refused.png'), fullPage: true })
})

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
  // P13/P15, S1/S2/S3: the file is usable, so only Workspace is degraded.
  // A generic fallback message without the known cause or actual destination
  // cannot distinguish a missing original location from a failed query.
  expect(result.degradations).toHaveLength(1)
  expect(result.degradations[0].element).toBe('workspace')
  expect(result.degradations[0].reason).toContain('缺少原 Workspace')
  expect(result.degradations[0].strategy).toContain('w-target')
  await expect(row.getByRole('status')).toContainText('缺少原 Workspace')
  await expect(row.getByRole('status')).toContainText('w-target')
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

  // P11: even unusable latest startup configuration cannot block locating an
  // existing identity. Same-name/same-cwd decoy is deliberately a different ID.
  fs.writeFileSync(server.metadataFile, 'not valid TOML [')
  server.setHerdrBehavior({ agents: [
    { agent: 'pi', name: 'Historical Pi', cwd, workspace_id: 'w-decoy', tab_id: 'w-decoy:t1', pane_id: 'w-decoy:p1', agent_session: { agent: 'pi', source: 'herdr:pi', kind: 'id', value: 'different-pi-native' } },
    { agent: 'pi', name: 'Historical Pi', cwd, workspace_id: result.target.workspaceId, tab_id: result.target.tabId, pane_id: result.target.paneId, agent_session: { agent: 'pi', source: 'herdr:pi', kind: 'id', value: 'pi-browser-exact' } },
  ] })
  const locatedResponse = page.waitForResponse(r => r.url().endsWith('/api/chats/resume') && r.request().method() === 'POST')
  await row.getByRole('button', { name: 'Resume', exact: true }).click()
  const located = await (await locatedResponse).json()
  expect(located.outcome).toBe('located')
  expect(located.target).toEqual(result.target)
  await expect(row.getByRole('status')).toContainText('已切换到当前herdr焦点')
  expect(server.herdrTabs()).toHaveLength(1)
  expect(server.herdrCalls().filter(call => call.kind === 'agent_start')).toHaveLength(1)
  expect(server.herdrCalls().some(call => call.kind === 'agent_prompt')).toBe(false)
})
