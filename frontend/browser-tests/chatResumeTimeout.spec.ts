import { expect, test } from '@playwright/test'
import fs from 'node:fs'
import path from 'node:path'
import { startServer, type E2EServer } from './support'

let server: E2EServer | null = null
test.afterEach(async () => { await server?.cleanup(); server = null })

// Approved S1/S2/S3. Withhold only the external native reference: the shipped
// page, TCP operation, CLI creation/polling and execution qualification stay real.
// P17-P19/P22: a ready shell is NOT confirmed history. Observe its retained
// external target, visible uncertainty and a fresh user lookup after it appears.
for (const kind of ['pi', 'omp']) {
  test(`${kind} startup timeout identifies retained target and a user retry locates it without another launch`, { tag: '@time-contract' }, async ({ page }) => {
    server = await startServer({ herdr: {} })
    const cwd = path.join(path.dirname(server.configPath), `${kind} timeout cwd`)
    fs.mkdirSync(cwd)
    const register = await fetch(`${server.baseURL}/api/sessions/events`, {
      method: 'POST', headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ sessionId: 'timeout-browser-native', kind: 'register', data: { cwd, agentType: kind, sessionName: `Unconfirmed ${kind}` } }),
    })
    expect(register.status).toBe(200)
    await page.goto(server.baseURL)
    await page.getByRole('button', { name: '查看历史会话' }).click()
    const row = page.getByRole('row').filter({ hasText: `Unconfirmed ${kind}` })
    const response = page.waitForResponse((r) => r.url().endsWith('/api/chats/resume') && r.request().method() === 'POST')
    await row.getByRole('button', { name: 'Resume', exact: true }).click()
    await expect.poll(() => server!.herdrCalls().filter((c) => c.kind === 'agent_get').length).toBeGreaterThan(0)
    await expect(row.getByRole('button', { name: '正在定位／恢复…' })).toBeDisabled()
    await expect(row.getByRole('status')).toHaveCount(0)
    const duplicate = await fetch(`${server.baseURL}/api/chats/resume`, {
      method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ sessionId: 'timeout-browser-native' }),
    })
    expect(duplicate.status).toBe(409)
    const reply = await response
    expect(reply.status()).toBe(502)
    const result = await reply.json()
    expect(result.outcome).toBe('unconfirmed')
    // P15/P20 existing-path ownership: selected Workspace fallback remains
    // visible when native loading is unconfirmed, not replaced by success.
    expect(result.degradations).toEqual(expect.arrayContaining([
      expect.objectContaining({ element: 'workspace', reason: expect.stringContaining('缺少原 Workspace'), strategy: expect.stringContaining('w-target') }),
    ]))
    await expect(row.getByRole('alert')).toContainText('缺少原 Workspace')
    await expect(row.getByRole('alert')).toContainText('默认 Workspace')
    await expect(row.getByRole('alert')).toContainText('启动结果未确认')
    await expect(row.getByRole('alert')).toContainText('不能据此判断未启动')
    await expect(row.getByRole('button', { name: 'Resume', exact: true })).toBeEnabled()
    const directory = path.join(path.dirname(server.configPath), 'herdr')
    const started = JSON.parse(fs.readFileSync(path.join(directory, 'started.jsonl'), 'utf8').trim())
    for (const value of [started.workspace_id, started.tab_id, started.pane_id, started.name]) {
      await expect(row.getByText(value, { exact: true })).toBeVisible()
    }
    expect(result.target).toEqual({ workspaceId: started.workspace_id, tabId: started.tab_id, paneId: started.pane_id })
    expect(result.agentName).toBe(started.name)
    expect(server.herdrTabs()).toHaveLength(1)
    expect(server.herdrCalls().filter((c) => c.kind === 'agent_focus')).toHaveLength(0)
    await page.screenshot({ path: test.info().outputPath(`${kind}-startup-unconfirmed.png`), fullPage: true })

    // New lookup failure must not conceal uncertainty with a replacement launch.
    server.setHerdrBehavior({ fail_agent_list: true })
    await row.getByRole('button', { name: 'Resume', exact: true }).click()
    await expect(row.getByRole('alert')).toContainText('Herdr 查找／身份核实')
    await expect(row.getByRole('button', { name: 'Resume', exact: true })).toBeEnabled()
    server.setHerdrBehavior({ agents: [{ ...started, agent_session: { agent: kind, source: `herdr:${kind}`, kind: 'id', value: 'timeout-browser-native' } }] })
    const retryResponse = page.waitForResponse((r) => r.url().endsWith('/api/chats/resume') && r.request().method() === 'POST')
    await row.getByRole('button', { name: 'Resume', exact: true }).click()
    const located = await (await retryResponse).json()
    expect(located.outcome).toBe('located')
    expect(located.target).toEqual(result.target)
    await expect(row.getByRole('status')).toHaveText('已切换到当前herdr焦点')
    await expect(row.locator('dl')).toHaveCount(0)
    expect(JSON.parse(fs.readFileSync(path.join(directory, 'focused.json'), 'utf8'))).toBe(started.pane_id)
    expect(server.herdrCalls().filter((c) => c.kind === 'agent_list')).toHaveLength(3)
    expect(server.herdrTabs()).toHaveLength(1)
    expect(server.herdrCalls().filter((c) => c.kind === 'agent_start')).toHaveLength(1)
    expect(server.herdrCalls().every((c) => ['agent_list', 'workspace_list', 'tab_create', 'agent_start', 'agent_get', 'agent_focus'].includes(String(c.kind)))).toBe(true)
    expect(fs.readFileSync(path.join(directory, 'started.jsonl'), 'utf8').trim().split('\n')).toHaveLength(1)
    await page.screenshot({ path: test.info().outputPath(`${kind}-retry-located-retained-target.png`), fullPage: true })
  })
}
