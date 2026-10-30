import { expect, test, type Page } from '@playwright/test'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { startServer, type E2EServer } from './support'
import { zoneRowFor } from './ui'

// Parallel cases run in separate worker processes; each keeps its own server and cleanup hooks.
test.describe.configure({ mode: 'parallel' })

let server: E2EServer | null = null
let root: string | null = null
test.afterEach(async () => {
  try { await server?.cleanup() }
  finally {
    server = null
    if (root) fs.rmSync(root, { recursive: true, force: true })
    root = null
  }
})
async function post(endpoint: string, body: unknown) {
  const response = await fetch(`${server!.baseURL}${endpoint}`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify(body) })
  expect(response.ok, await response.clone().text()).toBe(true)
  return response.json()
}
async function history() { return fetch(`${server!.baseURL}/api/sessions/history`).then(r => r.json()) }
async function prepare(behavior: Record<string, unknown>) {
  root = fs.mkdtempSync(path.join(os.tmpdir(), 'pi-zone-history-'))
  server = await startServer({ herdr: { behavior } })
  const zones = await post('/api/zones', { path: root })
  const saved = await post('/api/tasks', { name: 'Pi history browser', chatName: 'Pi Browser Chat', agent: 'pi', model: 'default', thinking: 'default', skill: null, prompt: '', extensions: [] })
  return { zone: zones.zones[0].path as string, task: saved.tasks.find((t: {name:string}) => t.name === 'Pi history browser').id as string }
}
async function createInPage(page: Page, zone: string) {
  await page.goto(server!.baseURL)
  await page.getByTestId('tab-files').click()
  await zoneRowFor(page, zone).getByTestId('zone-create-chat').click()
  const dialog = page.getByTestId('chat-creation-dialog')
  await dialog.locator('#chat-workspace-target').selectOption('default')
  await dialog.getByTestId('chat-input').fill('/settings')
  await dialog.getByTestId('chat-create-submit').click()
  await expect(dialog.getByTestId('chat-create-success')).toHaveText('创建成功，初始指示已提交')
  return dialog
}

// S1/S2/S4, P01/P05/P17/P26: success while exact identity is unresolved;
// close the page, release identity, observe both independent views afterward.
// The adapter controls Herdr only; browser, HTTP, persistence and SSE are real.
test('Pi without connect survives page close and history is independent of current registration', async ({ page, browser }, testInfo) => {
  const { zone } = await prepare({})
  const release = path.join(root!, 'release')
  server!.setHerdrBehavior({ native_session_gate: { wait_for_file: release, timeout_ms: 60_000 } })
  const historyRequests: string[] = []
  page.on('request', r => { if (r.url().includes('/api/sessions/history')) historyRequests.push(r.url()) })
  const dialog = await createInPage(page, zone)
  await expect.poll(() => server!.herdrCalls().some(c => c.kind === 'agent_get')).toBe(true)
  await expect(dialog).not.toContainText(/历史|收录/)
  expect(historyRequests).toEqual([])
  expect((await history()).sessions).toEqual([])
  await page.close()
  server!.setHerdrBehavior({ native_session: { source: 'herdr:pi', agent: 'pi', kind: 'id', value: 'pi-browser-exact-id' }, native_cwd: 'C:/browser/native' })
  fs.writeFileSync(release, 'release')
  await expect.poll(async () => (await history()).sessions.map((r: {sessionId:string}) => r.sessionId)).toEqual(['pi-browser-exact-id'])
  const reopened = await browser.newPage()
  try {
    await reopened.goto(server!.baseURL)
    await expect(reopened.getByTestId('session-board')).not.toContainText('Pi Browser Chat')
    await reopened.getByRole('button', { name: '查看历史会话' }).click()
    await expect(reopened.getByRole('row').filter({ hasText: 'Pi Browser Chat' })).toBeVisible()
    await reopened.screenshot({ path: testInfo.outputPath('pi-history-without-connect.png'), fullPage: true })
    await post('/api/sessions/events', { sessionId: 'pi-browser-exact-id', kind: 'register', data: { cwd: 'C:/browser/native', sessionName: 'Connect display name', agentType: 'pi' } })
    await reopened.getByRole('button', { name: '查看当前会话' }).click()
    await expect(reopened.getByTestId('session-board')).toContainText('Connect display name')
    await reopened.getByRole('button', { name: '查看历史会话' }).click()
    await expect(reopened.getByRole('row').filter({ hasText: 'Pi Browser Chat' })).toBeVisible()
    await post('/api/sessions/events', { sessionId: 'pi-browser-exact-id', kind: 'unregister' })
    await reopened.getByRole('button', { name: '查看当前会话' }).click()
    await expect(reopened.getByTestId('session-board')).not.toContainText('Connect display name')
    await reopened.getByRole('button', { name: '查看历史会话' }).click()
    await expect(reopened.getByRole('row').filter({ hasText: 'Pi Browser Chat' })).toBeVisible()
    expect(server!.herdrCalls().filter(c => c.kind === 'agent_prompt')).toHaveLength(1)
  } finally { await reopened.close() }
})

// The caller never receives a cleanup handle when initialization rejects. The
// fixture must still remove only the temporary tree allocated by that attempt.
test('isolated browser server removes its temporary data when initialization rejects', async () => {
  const marker = `startup-cleanup-${process.pid}-${Date.now()}`
  const ownedDirectories = () => fs.readdirSync(os.tmpdir(), { withFileTypes: true })
    .filter(entry => entry.isDirectory() && entry.name.startsWith('herdr-bot browser '))
    .map(entry => path.join(os.tmpdir(), entry.name))
    .filter(directory => {
      try { return fs.readFileSync(path.join(directory, 'herdr-bot.toml'), 'utf8').includes(marker) }
      catch (error) {
        if ((error as NodeJS.ErrnoException).code === 'ENOENT') return false
        throw error
      }
    })
  try {
    // A quote makes this attempt's real TOML invalid; it does not alter any
    // executable or shared fixture, and the backend must exit before listening.
    await expect(startServer({ herdr: { workspaceName: `${marker}'` } })).rejects.toThrow('提前退出')
    expect(ownedDirectories()).toEqual([])
  } finally {
    for (const directory of ownedDirectories()) fs.rmSync(directory, { recursive: true, force: true })
  }
})

// A pending external query owns a Windows executable handle. Stopping only
// the backend parent leaves that child alive and makes cleanup fail with EPERM.
test('isolated browser server cleanup also stops its pending Herdr query child', async ({ page }) => {
  const { zone } = await prepare({})
  server!.setHerdrBehavior({ native_session_gate: { wait_for_file: path.join(root!, 'never-release'), timeout_ms: 60_000 } })
  await createInPage(page, zone)
  await expect.poll(() => server!.herdrCalls().some(c => c.kind === 'agent_get')).toBe(true)
  const owned = server!
  // Keep afterEach ownership if cleanup fails; never kill a recycled record PID.
  await owned.cleanup()
  server = null
  expect(fs.existsSync(path.dirname(owned.configPath))).toBe(false)
})

// P19/P22/P30: collect warnings from the delivered executable, not a mocked
// logging function. Restored storage/identity without a new event stays absent.
for (const failure of ['missing-identity', 'unreadable-header', 'save-failed'] as const) {
  test(`Pi ${failure} is correlated in server diagnostics only and ends without retry`, { tag: '@time-contract' }, async ({ page }, testInfo) => {
    const { zone } = await prepare({})
    const native = path.join(root!, 'unavailable-native.jsonl')
    const historyFile = path.join(path.dirname(server!.storageFile), 'session-history.json')
    let restoreHistory: (() => void) | undefined
    let reads = 0
    try {
      if (failure === 'save-failed') {
        // Start with readable empty history, then deny actual destination publication.
        fs.writeFileSync(historyFile, '[]')
        const protectedPath = process.platform === 'win32' ? historyFile : path.dirname(historyFile)
        const originalMode = fs.statSync(protectedPath).mode
        fs.chmodSync(protectedPath, process.platform === 'win32' ? 0o444 : 0o555)
        restoreHistory = () => fs.chmodSync(protectedPath, originalMode)
        server!.setHerdrBehavior({ native_session: { source: 'herdr:pi', agent: 'pi', kind: 'id', value: 'pi-browser-save-failure' }, native_cwd: 'C:/failure' })
      } else if (failure === 'unreadable-header') {
        server!.setHerdrBehavior({ native_session: { source: 'herdr:pi', agent: 'pi', kind: 'path', value: native } })
      }
      const dialog = await createInPage(page, zone)
      const agent = (server!.herdrCalls().find(c => c.kind === 'agent_start')!.name) as string
      await expect.poll(() => server!.diagnostics(), { timeout: 35_000 }).toContain(failure === 'save-failed' ? 'save-failed' : 'identity-timeout')
      const logs = server!.diagnostics()
      expect(logs).toContain(`agent=${agent}`)
      expect(logs).toMatch(/creation=bc-[0-9a-f]+/)
      if (failure === 'unreadable-header') expect(logs).toContain('native header unavailable')
      await expect(dialog.getByTestId('chat-create-success')).toBeVisible()
      await expect(dialog).not.toContainText(/历史|收录|超时|保存失败/)
      await testInfo.attach('delivered-server-diagnostics', { body: logs, contentType: 'text/plain' })
      console.log(`PI-DIAGNOSTICS ${failure}: ${logs.split('\n').filter(l => l.includes('zone-history')).join('\n')}`)
      reads = server!.herdrCalls().filter(c => c.kind === 'agent_get').length
      if (failure === 'save-failed') {
        const response = await fetch(`${server!.baseURL}/api/sessions/history`)
        expect(response.status).toBe(200)
        expect((await response.json()).sessions).toEqual([])
        expect(fs.readFileSync(historyFile, 'utf8')).toBe('[]')
      }
    } finally { restoreHistory?.() }
    fs.writeFileSync(native, '{"type":"session","id":"pi-after-failure","cwd":"C:/too-late"}\n')
    server!.setHerdrBehavior({ native_session: { source: 'herdr:pi', agent: 'pi', kind: 'path', value: native } })
    await page.waitForTimeout(600)
    expect((await history()).sessions).toEqual([])
    expect(server!.herdrCalls().filter(c => c.kind === 'agent_get')).toHaveLength(reads)
    expect(server!.herdrCalls().filter(c => c.kind === 'agent_prompt')).toHaveLength(1)
  })
}
