import { expect, test } from '@playwright/test'
import fs from 'node:fs'
import path from 'node:path'
import { startServer, type E2EServer } from './support'

let server: E2EServer | null = null
test.afterEach(async () => { await server?.cleanup(); server = null })

// Approved S1/S2/S3; P21/P22. Independent old-format identities include a
// same-name/same-cwd decoy and the other agent. A resume writer or a register
// deduplicator keyed by display name would corrupt this literal set. Keep the
// real disk, shipped process restart, page and HTTP path in the exercised path;
// only the external Herdr environment is controlled. This is NOT H-Pi/H-OMP.
for (const kind of ['pi', 'omp']) {
  // P22 cross-path regression from real Herdr 0.9.5: its own readiness
  // deadline returns exit 1 + error.code=timeout, not an adapter timer expiry.
  // Treating all nonzero CLI exits as definite failure violates uncertainty;
  // observe the real adapter's HTTP receipt and the delivered page together.
  test(`${kind} structured Herdr startup timeout stays unconfirmed on HTTP and page`, async ({ page }) => {
    server = await startServer({ herdr: { behavior: { timeout_agent_start: true } } })
    const cwd = path.dirname(server.configPath)
    const register = await fetch(`${server.baseURL}/api/sessions/events`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ sessionId: 'structured-timeout-native', kind: 'register', data: { cwd, agentType: kind, sessionName: 'Structured timeout' } }) })
    expect(register.status).toBe(200)
    await page.goto(server.baseURL)
    await page.getByRole('button', { name: '查看历史会话' }).click()
    const row = page.getByRole('row').filter({ hasText: 'Structured timeout' })
    const response = page.waitForResponse(r => r.url().endsWith('/api/chats/resume') && r.request().method() === 'POST')
    await row.getByRole('button', { name: 'Resume', exact: true }).click()
    const reply = await response
    expect(reply.status()).toBe(502)
    const result = await reply.json()
    expect(result.outcome).toBe('unconfirmed')
    expect(result.code).toBe('restore-unconfirmed')
    expect(result.stage).toBe('start')
    expect(result.progress).toEqual({ tabCreated: true, startSubmitted: true, conversationConfirmed: false, focused: false })
    const started = JSON.parse(fs.readFileSync(path.join(cwd, 'herdr', 'started.jsonl'), 'utf8').trim())
    expect(result.target).toEqual({ workspaceId: started.workspace_id, tabId: started.tab_id, paneId: started.pane_id })
    expect(result.agentName).toBe(started.name)
    await expect(row.getByRole('alert')).toContainText('启动结果未确认')
    await expect(row.getByRole('status')).toHaveCount(0)
    expect(server.herdrTabs()).toHaveLength(1)
    expect(server.herdrCalls().map(c => c.kind)).toEqual(['agent_list', 'workspace_list', 'tab_create', 'agent_start'])
    await page.screenshot({ path: test.info().outputPath(`${kind}-structured-startup-unconfirmed.png`), fullPage: true })
  })

  test(`${kind} restored history survives normal connect, removal and process restart with consistent page and HTTP results`, async ({ page }) => {
    server = await startServer({ herdr: {} })
    const root = path.dirname(server.configPath)
    const cwd = path.join(root, 'Shared historical cwd with spaces')
    fs.mkdirSync(cwd)
    const native = path.join(cwd, `${kind} old conversation.jsonl`)
    const selected = kind === 'pi' ? 'pi-contract-old-001' : 'omp-contract-old-002'
    const nativeBytes = `${JSON.stringify({ type: 'session', version: 3, id: selected, cwd })}\n${JSON.stringify({ type: 'message', id: 'known-history', message: { role: 'user', content: 'Dedicated history marker; no new input permitted.' } })}\n`
    fs.writeFileSync(native, nativeBytes)
    const historyFile = path.join(root, 'session-history.json')
    const records = [
      { sessionId: 'pi-contract-old-001', agentType: 'pi', sessionName: 'Same historical name', cwd, firstRecordedAt: 123, ...(kind === 'pi' ? { sessionFile: native } : {}) },
      { sessionId: 'omp-contract-old-002', agentType: 'omp', sessionName: 'Same historical name', cwd, firstRecordedAt: 456, ...(kind === 'omp' ? { sessionFile: native } : {}) },
      { sessionId: 'same-name-different-native-003', agentType: kind, sessionName: 'Same historical name', cwd, firstRecordedAt: 789 },
    ]
    fs.writeFileSync(historyFile, JSON.stringify(records))
    await server.restart()
    const readHistory = async () => {
      const reply = await fetch(`${server!.baseURL}/api/sessions/history`)
      expect(reply.status).toBe(200)
      const body = await reply.json()
      expect(body.total).toBe(3)
      expect(body.sessions.map((s: { sessionId: string }) => s.sessionId).sort()).toEqual([
        'omp-contract-old-002', 'pi-contract-old-001', 'same-name-different-native-003',
      ])
      expect(body.sessions.find((s: { sessionId: string }) => s.sessionId === selected).firstRecordedAt).toBe(kind === 'pi' ? 123 : 456)
      return body
    }
    await readHistory()
    server.setHerdrBehavior({ native_session: { agent: kind, source: `herdr:${kind}`, kind: 'path', value: native } })
    await page.goto(server.baseURL)
    await page.getByRole('button', { name: '查看历史会话' }).click()
    // Names deliberately collide; the independently seeded first-recorded
    // timestamp identifies the public row without requiring IDs to be displayed.
    const row = page.getByRole('row').filter({ has: page.locator(`time[datetime="${kind === 'pi' ? '1970-01-01T00:00:00.123Z' : '1970-01-01T00:00:00.456Z'}"]`) })
    const restoredResponse = page.waitForResponse(r => r.url().endsWith('/api/chats/resume') && r.request().method() === 'POST')
    await row.getByRole('button', { name: 'Resume', exact: true }).click()
    const restoredReply = await restoredResponse
    expect(restoredReply.status()).toBe(200)
    const restored = await restoredReply.json()
    expect(restored.outcome).toBe('restored')
    await expect(row.getByRole('status')).toContainText(kind === 'pi' ? '已确认恢复同一 Pi 原生对话' : '已确认恢复同一 OMP 原生对话')
    await expect(row.getByRole('status')).toContainText('不表示 Windows 窗口已置前')
    const started = JSON.parse(fs.readFileSync(path.join(root, 'herdr', 'started.jsonl'), 'utf8').trim())
    expect(restored.target).toEqual({ workspaceId: started.workspace_id, tabId: started.tab_id, paneId: started.pane_id })
    expect(JSON.parse(fs.readFileSync(path.join(root, 'herdr', 'focused.json'), 'utf8'))).toBe(started.pane_id)
    expect(server.herdrCalls().map(c => c.kind)).toEqual(['agent_list', 'workspace_list', 'tab_create', 'agent_start', 'agent_get', 'agent_focus'])
    await readHistory()
    expect(JSON.parse(fs.readFileSync(historyFile, 'utf8'))).toEqual(records)

    const event = async (eventKind: string, data: Record<string, unknown> = {}) => {
      const reply = await fetch(`${server!.baseURL}/api/sessions/events`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ sessionId: selected, kind: eventKind, data }) })
      expect(reply.status).toBe(200)
    }
    await event('register', { agentType: kind, cwd, sessionFile: native, sessionName: 'Connected restored conversation', originalWorkspaceId: started.workspace_id })
    await readHistory()
    await event('unregister')
    await readHistory()
    await server.restart()
    await readHistory()
    // A registration is intentionally absent in the fresh process. Resume must
    // still locate the loaded native conversation, without creating a copy.
    server.setHerdrBehavior({ agents: [{ ...started, agent_session: { agent: kind, source: `herdr:${kind}`, kind: 'path', value: native } }] })
    const direct = async () => fetch(`${server!.baseURL}/api/chats/resume`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ sessionId: selected }) })
    const locatedReply = await direct()
    expect(locatedReply.status).toBe(200)
    const located = await locatedReply.json()
    expect(located.outcome).toBe('located')
    expect(located.target).toEqual(restored.target)
    await page.goto(server.baseURL)
    await page.getByRole('button', { name: '查看历史会话' }).click()
    const locatedResponse = page.waitForResponse(r => r.url().endsWith('/api/chats/resume') && r.request().method() === 'POST')
    await row.getByRole('button', { name: 'Resume', exact: true }).click()
    expect(await (await locatedResponse).json()).toEqual(located)
    await expect(row.getByRole('status')).toContainText('已定位已有对话')
    expect(JSON.parse(fs.readFileSync(path.join(root, 'herdr', 'focused.json'), 'utf8'))).toBe(started.pane_id)
    await page.screenshot({ path: test.info().outputPath(`${kind}-cross-path-located.png`), fullPage: true })

    // An actual refused external focus must agree across the two public paths;
    // neither is allowed to reinterpret it as success or start a replacement.
    server.setHerdrBehavior({ agents: [{ ...started, agent_session: { agent: kind, source: `herdr:${kind}`, kind: 'id', value: selected } }], fail_agent_focus: true })
    const failedReply = await direct()
    expect(failedReply.status).toBe(502)
    const failed = await failedReply.json()
    expect(failed.outcome).toBe('failed')
    expect(failed.stage).toBe('focus')
    const failedResponse = page.waitForResponse(r => r.url().endsWith('/api/chats/resume') && r.request().method() === 'POST')
    await row.getByRole('button', { name: 'Resume', exact: true }).click()
    const pageFailure = await failedResponse
    expect(pageFailure.status()).toBe(502)
    expect(await pageFailure.json()).toEqual(failed)
    await expect(row.getByRole('alert')).toContainText('聚焦')
    await expect(row.getByRole('status')).toHaveCount(0)
    expect(server.herdrTabs()).toHaveLength(1)
    expect(server.herdrCalls().filter(c => c.kind === 'agent_start')).toHaveLength(1)
    expect(server.herdrCalls().some(c => c.kind === 'agent_prompt')).toBe(false)
    expect(fs.readFileSync(native, 'utf8')).toBe(nativeBytes)
    await readHistory()
    await page.screenshot({ path: test.info().outputPath(`${kind}-cross-path-focus-failed.png`), fullPage: true })
  })
}
