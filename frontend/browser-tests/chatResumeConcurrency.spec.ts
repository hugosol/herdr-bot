import { expect, test } from '@playwright/test'
import fs from 'node:fs'
import path from 'node:path'
import { startServer, type E2EServer } from './support'

let server: E2EServer | null = null
test.afterEach(async () => { await server?.cleanup(); server = null })

// P16/S1/S2/S3: an independent HTTP caller owns a held Resume; the delivered
// browser must report the backend rejection, not infer success from its button.
// The external record, not button state, establishes no duplicate startup.
test('browser presents authoritative busy rejection and only runs again on a fresh user action', async ({ page }) => {
  server = await startServer({ herdr: {} })
  const cwd = path.join(path.dirname(server.configPath), 'concurrent historical cwd')
  fs.mkdirSync(cwd)
  const native = path.join(cwd, 'native.jsonl')
  fs.writeFileSync(native, `${JSON.stringify({ type: 'session', id: 'concurrent-browser', cwd })}\n`)
  const registered = await fetch(`${server.baseURL}/api/sessions/events`, {
    method: 'POST', headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ sessionId: 'concurrent-browser', kind: 'register', data: { cwd, agentType: 'pi', sessionName: 'Concurrent Chat', sessionFile: native } }),
  })
  expect(registered.status).toBe(200)
  const release = path.join(cwd, 'release-lookup')
  server.setHerdrBehavior({
    agent_list_gate: { wait_for_file: release },
    native_session: { source: 'herdr:pi', agent: 'pi', kind: 'path', value: native },
  })
  await page.goto(server.baseURL)
  await page.getByRole('button', { name: '查看历史会话' }).click()
  const row = page.getByRole('row').filter({ hasText: 'Concurrent Chat' })
  const firstRequest = fetch(`${server.baseURL}/api/chats/resume`, {
    method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ sessionId: 'concurrent-browser' }),
  })
  try {
    await expect.poll(() => server!.herdrCalls().filter((call) => call.kind === 'agent_list').length).toBe(1)
    const response = page.waitForResponse((reply) => reply.url().endsWith('/api/chats/resume') && reply.request().method() === 'POST')
    await row.getByRole('button', { name: 'Resume', exact: true }).click()
    const rejection = await response
    expect(rejection.status()).toBe(409)
    const result = await rejection.json()
    expect(result.code).toBe('resume-in-progress')
    await expect(row.getByRole('alert')).toHaveText(result.message)
    await expect(row.getByRole('alert')).toContainText('不会排队')
    await expect(row.getByRole('status')).toHaveCount(0)
    expect(server.herdrCalls().some(call => ['tab_create', 'agent_start', 'agent_prompt', 'agent_focus'].includes(String(call.kind)))).toBe(false)
    expect(server.herdrTabs()).toHaveLength(0)
    await page.screenshot({ path: test.info().outputPath('resume-authoritative-busy.png'), fullPage: true })
  } finally { fs.writeFileSync(release, 'release') }
  const first = await firstRequest
  expect(first.status).toBe(200)
  expect((await first.json()).outcome).toBe('restored')
  expect(server.herdrTabs()).toHaveLength(1)
  expect(server.herdrCalls().some(call => call.kind === 'agent_prompt')).toBe(false)
  // The rejected browser request stays rejected even after the owner succeeds.
  await expect(row.getByRole('alert')).toContainText('不会排队')
  await expect(row.getByRole('status')).toHaveCount(0)
  server.setHerdrBehavior({ agents: [{
    name: 'existing', agent: 'pi', workspace_id: 'w-existing', tab_id: 'w-existing:t1', pane_id: 'w-existing:p1', focused: false,
    agent_session: { source: 'herdr:pi', agent: 'pi', kind: 'id', value: 'concurrent-browser' },
  }] })
  const response = page.waitForResponse((reply) => reply.url().endsWith('/api/chats/resume') && reply.request().method() === 'POST')
  await row.getByRole('button', { name: 'Resume', exact: true }).click()
  const fresh = await response
  expect(fresh.status()).toBe(200)
  const result = await fresh.json()
  expect(result.outcome).toBe('located')
  await expect(row.getByRole('status')).toHaveText('已切换到当前herdr焦点')
  await expect(row.locator('dl')).toHaveCount(0)
  await expect(row.getByRole('alert')).toHaveCount(0)
  expect(server.herdrCalls().filter((call) => call.kind === 'agent_list')).toHaveLength(2)
  expect(server.herdrCalls().filter((call) => call.kind === 'agent_start')).toHaveLength(1)
  expect(server.herdrTabs()).toHaveLength(1)
  await page.screenshot({ path: test.info().outputPath('resume-after-busy-new-action.png'), fullPage: true })
})
