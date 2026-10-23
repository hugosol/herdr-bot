import { expect, test, type Page } from '@playwright/test'
import fs from 'node:fs'
import path from 'node:path'
import { startServer, type E2EServer } from './support'

let server: E2EServer | null = null
test.afterEach(async () => { await server?.cleanup(); server = null })

async function post(endpoint: string, body: unknown) {
  const response = await fetch(`${server!.baseURL}${endpoint}`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify(body) })
  expect(response.ok, await response.clone().text()).toBe(true)
  return response.json()
}
async function resumeOnPage(page: Page, name: string) {
  await page.goto(server!.baseURL)
  await page.getByRole('button', { name: '查看历史会话' }).click()
  const row = page.getByRole('row').filter({ hasText: name })
  const response = page.waitForResponse(r => r.url().endsWith('/api/chats/resume') && r.request().method() === 'POST')
  await row.getByRole('button', { name: 'Resume', exact: true }).click()
  return { row, response: await response }
}
const preset = (agent: string) => ({ name: 'Same name', chatName: 'Same name', agent, model: 'default', thinking: 'default', prompt: '', extensions: [] })

// Approved public HTTP, delivered page, real disk/process and CLI adapter seams.
// P7/P15: a fixed-only fallback or a guessed source would expose explicit flags.
// This checks submitted discovery policy, NOT real native extension loading.
for (const agent of ['pi', 'omp']) {
  for (const scenario of ['legacy', 'external', 'deleted']) {
    test(`${agent} ${scenario} displays native discovery reason without binding a decoy Bot`, async ({ page }) => {
      server = await startServer({ herdr: {} })
      const root = path.dirname(server.configPath)
      const id = `${agent}-independent-discovery-001`
      const file = path.join(root, `${id}.jsonl`)
      fs.writeFileSync(file, `${JSON.stringify({ type: 'session', version: 3, id, cwd: root })}\n`)
      const name = `${agent} ${scenario} history`
      const source = (await post('/api/bots', preset(agent))).bots[0].id
      server.setHerdrBehavior({ native_session: { agent, source: `herdr:${agent}`, kind: 'path', value: file } })
      const historyFile = path.join(root, 'session-history.json')
      if (scenario === 'legacy') {
        fs.writeFileSync(historyFile, JSON.stringify([{ sessionId: id, sessionName: name, agentType: agent, cwd: root, sessionFile: file, firstRecordedAt: 123, originalWorkspaceId: 'w-target' }]))
      } else if (scenario === 'external') {
        await post('/api/sessions/events', { sessionId: id, kind: 'register', data: { sessionName: name, agentType: agent, cwd: root, sessionFile: file, originalWorkspaceId: 'w-target', sourceBotId: source } })
      } else {
        const zone = (await post('/api/zones', { path: root })).zones[0].path
        const preview = await post('/api/chats/preview', { zone, botId: source })
        const created = await post('/api/chats', { zone, botId: source, chatId: preview.chatId, input: 'INITIAL ONLY' })
        await expect.poll(async () => (await (await fetch(`${server!.baseURL}/api/chats/${created.id}`)).json()).stage).toBe('succeeded')
        await expect.poll(async () => (await (await fetch(`${server!.baseURL}/api/sessions/history`)).json()).total).toBe(1)
        expect((await fetch(`${server.baseURL}/api/bots/${source}`, { method: 'DELETE' })).status).toBe(200)
        await post('/api/bots', preset(agent))
      }
      const before = fs.readFileSync(historyFile)
      const nativeBefore = fs.readFileSync(file)
      await server.restart()
      const offset = server.herdrCalls().length
      const { row, response } = await resumeOnPage(page, scenario === 'deleted' ? 'Same name' : name)
      expect(response.status()).toBe(200)
      const result = await response.json()
      expect(result.outcome).toBe('restored')
      expect(result.degradations).toHaveLength(1)
      expect(result.degradations[0]).toEqual({ element: 'extensions', reason: expect.stringContaining(scenario === 'deleted' ? '已删除' : '缺少来源 Bot'), strategy: expect.stringContaining('原生默认扩展发现') })
      await expect(row.getByRole('status')).toContainText(result.degradations[0].reason)
      await expect(row.getByRole('status')).toContainText('原生默认扩展发现')
      await expect(row.getByRole('status')).toContainText('不显式追加 Bot 固定／可选扩展')
      const calls = server.herdrCalls().slice(offset)
      expect(calls.find(call => call.kind === 'agent_start')!.args).toEqual([agent === 'pi' ? '--session' : '--resume', file])
      expect(calls.some(call => ['agent_prompt', 'agent_input'].includes(String(call.kind)))).toBe(false)
      expect(fs.readFileSync(historyFile)).toEqual(before)
      expect(fs.readFileSync(file)).toEqual(nativeBefore)
      await page.screenshot({ path: test.info().outputPath(`${agent}-${scenario}-native-discovery.png`), fullPage: true })
    })
  }

  // P17: independent real parse and OS-read failures, not a mocked Bot result.
  // A catch-all deleted-Bot interpretation would wrongly create a Tab/start.
  for (const mechanism of ['bots-corrupt', 'bots-unreadable', 'catalog-parse', 'catalog-unreadable']) {
    test(`${agent} ${mechanism} stays failed on HTTP and page without repair or launch`, async ({ page }) => {
      server = await startServer({ herdr: {} })
      const root = path.dirname(server.configPath)
      const source = (await post('/api/bots', preset(agent))).bots[0].id
      const id = `${agent}-error-independent-001`
      const native = path.join(root, `${id}.jsonl`)
      fs.writeFileSync(native, `${JSON.stringify({ type: 'session', version: 3, id, cwd: root })}\n`)
      const historyFile = path.join(root, 'session-history.json')
      const bytes = JSON.stringify([{ sessionId: id, sessionName: 'Source read error', agentType: agent, cwd: root, sessionFile: native, firstRecordedAt: 123, originalWorkspaceId: 'w-target', sourceBotId: source }])
      fs.writeFileSync(historyFile, bytes)
      const affected = mechanism.startsWith('bots') ? server.storageFile : server.metadataFile
      if (mechanism.endsWith('unreadable')) {
        fs.unlinkSync(affected)
        fs.mkdirSync(affected)
        fs.writeFileSync(path.join(affected, 'preserve.txt'), 'Do not repair')
      } else fs.writeFileSync(affected, '{ independently invalid !!!')
      const { row, response } = await resumeOnPage(page, 'Source read error')
      expect(response.status()).toBe(502)
      const result = await response.json()
      expect(result.outcome).toBe('failed')
      expect(result.stage).toBe('settings')
      expect(result.code).toBe(mechanism.startsWith('bots') ? mechanism : 'invalid-agent-settings')
      expect(result.degradations).toEqual([])
      await expect(row.getByRole('alert')).toContainText('尚未确认恢复成功')
      await expect(row.getByRole('alert')).toContainText(mechanism.startsWith('bots') ? 'Bot存储文件' : '选项元数据')
      await expect(row.getByRole('status')).toHaveCount(0)
      expect(server.herdrTabs()).toHaveLength(0)
      expect(server.herdrCalls().some(call => ['agent_start', 'agent_prompt', 'agent_input'].includes(String(call.kind)))).toBe(false)
      expect(fs.readFileSync(historyFile, 'utf8')).toBe(bytes)
      expect(fs.readFileSync(mechanism.endsWith('unreadable') ? path.join(affected, 'preserve.txt') : affected, 'utf8')).toBe(mechanism.endsWith('unreadable') ? 'Do not repair' : '{ independently invalid !!!')
      await page.screenshot({ path: test.info().outputPath(`${agent}-${mechanism}.png`), fullPage: true })
    })
  }
}

// P8: names are transient display data keyed by persisted ID, even after rename,
// failed lookup and restart. A read-repair writer changes bytes or mtime.
test('history reads and page resolve renamed Workspace without persistent enrichment', async ({ page }) => {
  server = await startServer({ herdr: {} })
  const root = path.dirname(server.configPath)
  const historyFile = path.join(root, 'session-history.json')
  const bytes = JSON.stringify([{ sessionId: 'read-only-native-001', sessionName: 'Read only legacy', agentType: 'pi', cwd: root, firstRecordedAt: 123, originalWorkspaceId: 'w-stable' }])
  fs.writeFileSync(historyFile, bytes)
  const modified = fs.statSync(historyFile).mtimeMs
  for (const label of ['Original name', 'Renamed name', null]) {
    fs.writeFileSync(path.join(root, 'herdr', 'workspaces.json'), JSON.stringify([{ workspace_id: 'w-stable', label: label ?? 'Unavailable' }, { workspace_id: 'decoy', label: 'Original name' }]))
    server.setHerdrBehavior({ fail_workspace_list: label === null })
    await server.restart()
    for (let i = 0; i < 3; i++) {
      const response = await fetch(`${server.baseURL}/api/sessions/history`)
      expect(response.status).toBe(200)
      const row = (await response.json()).sessions[0]
      expect(row.originalWorkspaceId).toBe('w-stable')
      expect(row.originalWorkspaceName).toBe(label ?? undefined)
    }
    await page.goto(server.baseURL)
    await page.getByRole('button', { name: '查看历史会话' }).click()
    await expect(page.getByRole('row').filter({ hasText: 'Read only legacy' })).toContainText(label ?? 'w-stable')
    expect(fs.readFileSync(historyFile, 'utf8')).toBe(bytes)
    expect(fs.statSync(historyFile).mtimeMs).toBe(modified)
    expect(fs.existsSync(server.storageFile)).toBe(false)
  }
  await page.screenshot({ path: test.info().outputPath('read-only-workspace-id-fallback.png'), fullPage: true })
})
