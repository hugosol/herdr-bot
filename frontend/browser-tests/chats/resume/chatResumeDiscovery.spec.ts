import { expect, test, type Page } from '@playwright/test'
import fs from 'node:fs'
import path from 'node:path'
import { startServer, type E2EServer } from '../../support/support'

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
const preset = (agent: string) => ({ name: 'Same name', chatName: 'Same name', agent, model: 'default', thinking: 'default', prompt: '' })

// Approved public HTTP, delivered page, real disk/process and CLI adapter seams.
// P7/P15: a fixed-only fallback or a guessed source would expose explicit flags.
// This checks submitted discovery policy, NOT real native extension loading.
for (const agent of ['pi', 'omp']) {
  for (const scenario of ['legacy', 'external', 'deleted']) {
    test(`${agent} ${scenario} displays native discovery reason without binding a decoy Task`, async ({ page }) => {
      server = await startServer({ herdr: {} })
      const root = path.dirname(server.configPath)
      const id = `${agent}-independent-discovery-001`
      const file = path.join(root, `${id}.jsonl`)
      fs.writeFileSync(file, `${JSON.stringify({ type: 'session', version: 3, id, cwd: root })}\n`)
      const name = `${agent} ${scenario} history`
      const source = (await post('/api/tasks', preset(agent))).tasks[0].id
      server.setHerdrBehavior({ native_session: { agent, source: `herdr:${agent}`, kind: 'path', value: file } })
      const historyFile = path.join(root, 'session-history.json')
      if (scenario === 'legacy') {
        fs.writeFileSync(historyFile, JSON.stringify([{ sessionId: id, sessionName: name, agentType: agent, cwd: root, sessionFile: file, firstRecordedAt: 123, originalWorkspaceId: 'w-target' }]))
      } else if (scenario === 'external') {
        await post('/api/sessions/events', { sessionId: id, kind: 'register', data: { sessionName: name, agentType: agent, cwd: root, sessionFile: file, originalWorkspaceId: 'w-target', sourceTaskId: source } })
      } else {
        const zone = (await post('/api/zones', { id: 'test/zone', path: root })).zones[0].id
        const preview = await post('/api/chats/preview', { zone, taskId: source })
        const created = await post('/api/chats', { zone, taskId: source, chatId: preview.chatId, input: 'INITIAL ONLY' })
        await expect.poll(async () => (await (await fetch(`${server!.baseURL}/api/chats/${created.id}`)).json()).stage).toBe('succeeded')
        await expect.poll(async () => (await (await fetch(`${server!.baseURL}/api/sessions/history`)).json()).total).toBe(1)
        expect((await fetch(`${server.baseURL}/api/tasks/${source}`, { method: 'DELETE' })).status).toBe(200)
        await post('/api/tasks', preset(agent))
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
      expect(result.degradations[0]).toEqual({ element: 'extensions', reason: expect.stringContaining(scenario === 'deleted' ? '已删除' : '缺少来源 Task'), strategy: expect.stringContaining('原生默认扩展发现') })
      await expect(row.getByRole('status')).toContainText(result.degradations[0].reason)
      await expect(row.getByRole('status')).toContainText('原生默认扩展发现')
      await expect(row.getByRole('status')).toContainText('不显式追加 Task 固定扩展或 Suite 资源')
      const calls = server.herdrCalls().slice(offset)
      expect(calls.find(call => call.kind === 'agent_start')!.args).toEqual([agent === 'pi' ? '--session' : '--resume', file])
      expect(calls.some(call => ['agent_prompt', 'agent_input'].includes(String(call.kind)))).toBe(false)
      expect(fs.readFileSync(historyFile)).toEqual(before)
      expect(fs.readFileSync(file)).toEqual(nativeBefore)
      await page.screenshot({ path: test.info().outputPath(`${agent}-${scenario}-native-discovery.png`), fullPage: true })
    })
  }

  // S1 / B1 (P34): corrupt source config is not deletion. The real page
  // must keep the same policy and exact reason after either success or failure.
  for (const mechanism of ['tasks-corrupt', 'tasks-unreadable', 'catalog-parse', 'catalog-unreadable', 'suite-invalid', 'suite-resource']) {
    const unavailable = mechanism === 'suite-resource'
    const outcome = unavailable ? 'rejects the declared resource without starting' : 'displays native discovery'
    test(`${agent} ${mechanism} ${outcome} and preserves the source`, async ({ page }) => {
      server = await startServer({ herdr: {} })
      const root = path.dirname(server.configPath)
      const suiteFile = path.join(root, 'meta-data', 'suites.json')
      const invalidSuite = mechanism === 'suite-invalid'
      const hasSuite = invalidSuite || unavailable
      const entry = path.join(path.dirname(suiteFile), 'declared-suite-extension.ts')
      const suite = {
        id: `original-suite-${agent}`, name: `Original Suite · ${agent === 'omp' ? 'OMP' : 'Pi'}`, harness: agent,
        skills: [], extensions: [entry], mcp: [], ...(agent === 'pi' ? { prompts: [], codemode: 'on' } : { lsp: {} }),
      }
      if (hasSuite) {
        fs.mkdirSync(path.dirname(suiteFile), { recursive: true })
        fs.writeFileSync(entry, 'export default function () {}\n')
        fs.writeFileSync(suiteFile, JSON.stringify({ suites: [suite] }))
      }
      const source = (await post('/api/tasks', { ...preset(agent), ...(hasSuite ? { suite: suite.id } : {}) })).tasks[0].id
      const id = `${agent}-error-independent-001`
      const native = path.join(root, `${id}.jsonl`)
      fs.writeFileSync(native, `${JSON.stringify({ type: 'session', version: 3, id, cwd: root })}\n`)
      const historyFile = path.join(root, 'session-history.json')
      const bytes = JSON.stringify([{ sessionId: id, sessionName: 'Source read error', agentType: agent, cwd: root, sessionFile: native, firstRecordedAt: 123, originalWorkspaceId: 'w-target', sourceTaskId: source }])
      fs.writeFileSync(historyFile, bytes)
      const affected = invalidSuite ? suiteFile : mechanism.startsWith('tasks') ? server.storageFile : server.metadataFile
      const damaged = '{ independently invalid !!!'
      const authorityBefore = unavailable ? fs.readFileSync(suiteFile) : null
      const nativeBefore = fs.readFileSync(native)
      if (unavailable) {
        fs.unlinkSync(entry)
      } else if (mechanism.endsWith('unreadable')) {
        fs.unlinkSync(affected)
        fs.mkdirSync(affected)
        fs.writeFileSync(path.join(affected, 'preserve.txt'), 'Do not repair')
      } else fs.writeFileSync(affected, damaged)
      const failFocus = mechanism === 'catalog-unreadable'
      server.setHerdrBehavior({ native_session: { agent, source: `herdr:${agent}`, kind: 'path', value: native }, fail_agent_focus: failFocus })
      const { row, response } = await resumeOnPage(page, 'Source read error')
      if (unavailable) {
        expect(response.status()).toBe(502)
        expect(await response.json()).toMatchObject({ outcome: 'failed', stage: 'settings', target: null, degradations: [] })
        await expect(row.getByRole('alert')).toContainText('declared-suite-extension.ts')
        await expect(row.getByRole('alert')).not.toContainText('原生默认扩展发现')
        expect(server.herdrTabs()).toHaveLength(0)
        expect(server.herdrCalls().some(call => ['tab_create', 'agent_start', 'agent_prompt', 'agent_input'].includes(String(call.kind)))).toBe(false)
        expect(fs.readFileSync(suiteFile)).toEqual(authorityBefore)
        expect(fs.readFileSync(historyFile, 'utf8')).toBe(bytes)
        expect(fs.readFileSync(native)).toEqual(nativeBefore)
        await page.screenshot({ path: test.info().outputPath(`${agent}-${mechanism}.png`), fullPage: true })
        return
      }
      expect(response.status()).toBe(failFocus ? 502 : 200)
      const result = await response.json()
      expect(result.outcome).toBe(failFocus ? 'failed' : 'restored')
      if (failFocus) expect(result.stage).toBe('focus')
      const feedback = row.getByRole(failFocus ? 'alert' : 'status')
      for (const text of [source, invalidSuite ? 'Suite 配置' : 'env config', '原生默认扩展发现', '默认资源范围可能变化', '不代表恢复了原 Suite', failFocus ? '尚未确认恢复成功' : '已确认恢复同一']) {
        await expect(feedback).toContainText(text)
      }
      expect(server.herdrTabs()).toHaveLength(1)
      expect(server.herdrCalls().filter(call => call.kind === 'agent_start').map(call => call.args)).toEqual([[agent === 'pi' ? '--session' : '--resume', native]])
      expect(server.herdrCalls().some(call => ['agent_prompt', 'agent_input'].includes(String(call.kind)))).toBe(false)
      expect(fs.readFileSync(historyFile, 'utf8')).toBe(bytes)
      expect(fs.readFileSync(mechanism.endsWith('unreadable') ? path.join(affected, 'preserve.txt') : affected, 'utf8')).toBe(mechanism.endsWith('unreadable') ? 'Do not repair' : damaged)
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
