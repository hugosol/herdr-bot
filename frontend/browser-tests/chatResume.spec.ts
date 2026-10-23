import { expect, test } from '@playwright/test'
import fs from 'node:fs'
import path from 'node:path'
import { startServer, type E2EServer } from './support'

// S1/P20/P24: real page receives backend failures, not client-side fabricated
// success. Each external fault remains on the production adapter path.
test('delivered mobile page distinguishes lookup, identity and focus failures without launching', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 })
  server = await startServer({ herdr: {} })
  await register('native-browser', { cwd: 'C:/old', agentType: 'pi', sessionName: 'Failure Chat' })
  await page.goto(server.baseURL)
  await page.getByRole('button', { name: '查看历史会话' }).click()
  const row = page.getByRole('row').filter({ hasText: 'Failure Chat' })
  for (const [fault, code, text] of [
    ['fail_agent_list', 'herdr-command-failed', '查找／身份核实失败'],
    ['invalid_agent_list', 'herdr-unexpected-response', '不是单个 JSON'],
    ['error_agent_list', 'herdr-command-failed', 'query refused'],
    ['fail_agent_focus', 'herdr-command-failed', 'Herdr 聚焦失败'],
    ['unconfirmed_focus', 'herdr-unexpected-response', 'target navigation was not confirmed'],
    ['unreadable_identity', 'identity-verification-failed', 'native header unavailable'],
  ] as const) {
    const agents = fault === 'unreadable_identity'
      ? [{ ...target('w9:p8'), agent_session: { agent: 'pi', source: 'herdr:pi', kind: 'path', value: path.join(path.dirname(server.configPath), 'missing-native.jsonl') } }]
      : [target('w9:p8')]
    server.setHerdrBehavior({ agents, [fault]: true })
    const response = page.waitForResponse((response) => response.url().endsWith('/api/chats/resume') && response.request().method() === 'POST')
    await row.getByRole('button', { name: 'Resume', exact: true }).click()
    expect((await (await response).json()).code).toBe(code)
    await expect(row.getByRole('alert')).toContainText(text)
    await expect(row.getByRole('alert')).toContainText('尚未恢复')
    await expect(row.getByRole('button', { name: 'Resume', exact: true })).toBeEnabled()
  }
  expect(server.herdrCalls().every((call) => ['agent_list', 'agent_focus', 'agent_get'].includes(String(call.kind)))).toBe(true)
  expect(server.herdrTabs()).toEqual([])
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true)
  await page.screenshot({ path: test.info().outputPath('resume-mobile-failure.png'), fullPage: true })
})

let server: E2EServer | null = null
test.afterEach(async () => { await server?.cleanup(); server = null })
const target = (pane: string) => ({ agent: 'pi', name: 'not-the-native-id', agent_status: 'working', workspace_id: 'w9', tab_id: 'w9:t4', pane_id: pane, focused: false, agent_session: { source: 'herdr:pi', agent: 'pi', kind: 'id', value: 'native-browser' } })
async function register(sessionId: string, data: Record<string, unknown>) {
  const response = await fetch(`${server!.baseURL}/api/sessions/events`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ sessionId, kind: 'register', data }) })
  expect(response.status).toBe(200)
}

// S1 delivered page + S2 requests + S3 observed CLI. A hidden button for
// unregistered/unknown records or a frontend-only success cannot satisfy this.
test('every historical Chat offers Resume after unregister and process restart; multiple targets are selected without a picker', async ({ page }) => {
  server = await startServer({ herdr: { behavior: { agents: [target('w9:p8'), target('w9:p2')] } } })
  await register('native-browser', { cwd: 'C:/old', agentType: 'pi', sessionName: 'Connected Chat' })
  await register('unknown-old-record', { cwd: 'C:/old' })
  const removed = await fetch(`${server.baseURL}/api/sessions/events`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ sessionId: 'native-browser', kind: 'unregister', data: {} }) })
  expect(removed.status).toBe(200)
  await server.stop()
  const historyFile = path.join(path.dirname(server.configPath), 'session-history.json')
  const saved = JSON.parse(fs.readFileSync(historyFile, 'utf8'))
  // Old on-disk format deliberately lacks sessionFile/nameFromZone/Workspace.
  saved.push({ sessionId: 'legacy-native', sessionName: 'Legacy Chat', agentType: 'pi', cwd: 'C:/old', firstRecordedAt: 1 })
  fs.writeFileSync(historyFile, JSON.stringify(saved))
  await server.restart()
  for (const sessionId of ['native-browser', 'legacy-native', 'unknown-old-record']) {
    const reply = await fetch(`${server.baseURL}/api/chats/resume`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ sessionId }) })
    expect(reply.status).toBe(sessionId === 'unknown-old-record' ? 400 : sessionId === 'legacy-native' ? 502 : 200)
    const result = await reply.json()
    expect(sessionId === 'native-browser' ? result.outcome : result.code).toBe(sessionId === 'native-browser' ? 'located' : sessionId === 'legacy-native' ? 'invalid-cwd' : 'unsupported-agent')
  }
  await page.goto(server.baseURL)
  await page.getByRole('button', { name: '查看历史会话' }).click()
  const history = page.getByRole('region', { name: '历史会话' })
  const row = history.getByRole('row').filter({ hasText: 'Connected Chat' })
  await row.getByRole('button', { name: 'Resume', exact: true }).click()
  await expect(row.getByRole('status')).toContainText('找到 2 个匹配目标')
  await expect(row.getByRole('status')).toContainText('自动选择并定位')
  await expect(row.getByRole('status')).toContainText('不表示 Windows 窗口已置前')
  await expect(history.getByRole('combobox')).toHaveCount(0)
  expect(server.herdrCalls().find((call) => call.kind === 'agent_focus')?.argv).toEqual(['agent', 'focus', 'w9:p2'])
  expect(server.herdrTabs()).toEqual([])
  const legacy = history.getByRole('row').filter({ hasText: 'Legacy Chat' })
  await legacy.getByRole('button', { name: 'Resume', exact: true }).click()
  await expect(legacy.getByRole('alert')).toContainText('历史工作目录缺失')
  const unknown = history.getByRole('row').filter({ hasText: 'unknown-old-record' })
  await unknown.getByRole('button', { name: 'Resume', exact: true }).click()
  await expect(unknown.getByRole('alert')).toContainText('无法确定受支持的 agent 类型')
  await page.screenshot({ path: test.info().outputPath('resume-multiple-and-unknown.png'), fullPage: true })
})
