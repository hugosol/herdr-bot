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
    // Unreadable candidates are skipped. Refused workspace evidence must make
    // the subsequent cwd fallback fail visibly, never focus a guessed identity.
    ['unreadable_identity', 'herdr-command-failed', 'Herdr'],
  ] as const) {
    const agents = fault === 'unreadable_identity'
      ? [{ ...target('w9:p8'), agent_session: { agent: 'pi', source: 'herdr:pi', kind: 'path', value: path.join(path.dirname(server.configPath), 'missing-native.jsonl') } }]
      : [target('w9:p8')]
    server.setHerdrBehavior({ agents, [fault]: true, ...(fault === 'unreadable_identity' ? { fail_workspace_list: true } : {}) })
    const response = page.waitForResponse((response) => response.url().endsWith('/api/chats/resume') && response.request().method() === 'POST')
    await row.getByRole('button', { name: 'Resume', exact: true }).click()
    expect((await (await response).json()).code).toBe(code)
    await expect(row.getByRole('alert')).toContainText(text)
    await expect(row.getByRole('alert')).toContainText(fault === 'unreadable_identity' ? '尚未确认恢复成功' : '尚未恢复')
    await expect(row.getByRole('button', { name: 'Resume', exact: true })).toBeEnabled()
  }
  expect(server.herdrCalls().some(call => ['tab_create', 'agent_start', 'agent_prompt'].includes(String(call.kind)))).toBe(false)
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
  server = await startServer({ herdr: { behavior: { agents: [target('w9:p8'), target('w9:p2')], fail_tab_create: true } } })
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
    expect(sessionId === 'native-browser' ? result.outcome : result.code).toBe(sessionId === 'native-browser' ? 'located' : sessionId === 'legacy-native' ? 'herdr-command-failed' : 'unsupported-agent')
  }
  await page.goto(server.baseURL)
  await page.getByRole('button', { name: '查看历史会话' }).click()
  const history = page.getByRole('region', { name: '历史会话' })
  const row = history.getByRole('row').filter({ hasText: 'Connected Chat' })
  await row.getByRole('button', { name: 'Resume', exact: true }).click()
  await expect(row.getByRole('status')).toHaveText('已切换到当前herdr焦点')
  await expect(row.getByRole('definition')).toHaveCount(0)
  await expect(row.locator('dl')).toHaveCount(0)
  await expect(history.getByRole('combobox')).toHaveCount(0)
  expect(server.herdrCalls().find((call) => call.kind === 'agent_focus')?.argv).toEqual(['agent', 'focus', 'w9:p2'])
  expect(server.herdrTabs()).toEqual([])
  const legacy = history.getByRole('row').filter({ hasText: 'Legacy Chat' })
  await legacy.getByRole('button', { name: 'Resume', exact: true }).click()
  await expect(legacy.getByRole('alert')).toContainText('Tab')
  const unknown = history.getByRole('row').filter({ hasText: 'unknown-old-record' })
  await unknown.getByRole('button', { name: 'Resume', exact: true }).click()
  await expect(unknown.getByRole('alert')).toContainText('无法确定受支持的 agent 类型')
  await page.screenshot({ path: test.info().outputPath('resume-multiple-and-unknown.png'), fullPage: true })
})

for (const agent of ['omp', 'pi']) {
  test(`${agent} deleted cwd renders default-workspace degradation and actual directory without rewriting history`, async ({ page }) => {
    server = await startServer({ herdr: { workspaces: [
      { workspace_id: 'w-original', label: 'Original workspace' },
      { workspace_id: 'w-default', label: 'bot-playground' },
      { workspace_id: 'w-focused', label: 'Focused decoy', focused: true },
    ] } })
    const root = path.dirname(server.configPath)
    const deleted = path.join(root, 'deleted historical checkout')
    fs.mkdirSync(deleted)
    fs.rmdirSync(deleted)
    const actual = path.join(root, 'Herdr default actual directory')
    fs.mkdirSync(actual)
    const native = path.join(root, 'preserved native conversation.jsonl')
    const id = `${agent}-deleted-cwd-exact-native`
    const nativeBytes = `${JSON.stringify({ type: 'session', version: 3, id, cwd: deleted })}\n`
    fs.writeFileSync(native, nativeBytes)
    const historyFile = path.join(root, 'session-history.json')
    const bytes = `  ${JSON.stringify([{ sessionId: id, sessionName: 'Deleted cwd history', agentType: agent, cwd: deleted, sessionFile: native, originalWorkspaceId: 'w-original', firstRecordedAt: 123, memo: 'Keep exact bytes', futureMetadata: { opaque: true } }])}\n`
    fs.writeFileSync(historyFile, bytes)
    await server.restart()
    server.setHerdrBehavior({ default_cwd: actual, native_session: { agent, source: `herdr:${agent}`, kind: 'path', value: native } })
    await page.goto(server.baseURL)
    await page.getByRole('button', { name: '查看历史会话' }).click()
    const row = page.getByRole('row').filter({ hasText: 'Deleted cwd history' })
    const response = page.waitForResponse(r => r.url().endsWith('/api/chats/resume') && r.request().method() === 'POST')
    await row.getByRole('button', { name: 'Resume', exact: true }).click()
    const reply = await response
    expect(reply.status()).toBe(200)
    const result = await reply.json()
    expect(result.outcome).toBe('restored')
    expect(path.resolve(result.actualCwd).toLowerCase()).toBe(path.resolve(actual).toLowerCase())
    await expect(row.getByRole('status')).toContainText('恢复存在降级')
    await expect(row.getByRole('status')).toContainText(result.actualCwd)
    const calls = server.herdrCalls()
    const tab = calls.find(c => c.kind === 'tab_create')!
    expect(tab.workspace).toBe('w-default')
    expect(tab.argv).not.toContain('--cwd')
    expect(calls.find(c => c.kind === 'agent_start')!.args).toEqual([agent === 'pi' ? '--session' : '--resume', native])
    expect(calls.some(c => c.kind === 'agent_prompt')).toBe(false)
    expect(fs.readFileSync(historyFile, 'utf8')).toBe(bytes)
    expect(fs.readFileSync(native, 'utf8')).toBe(nativeBytes)
    await page.screenshot({ path: test.info().outputPath(`${agent}-deleted-cwd-degradation.png`), fullPage: true })
  })
}
