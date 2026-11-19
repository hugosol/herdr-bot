import { expect, test } from '@playwright/test'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { startServer, type E2EServer } from '../../support/support'
import { beginZoneChat, currentSnapshot, keepControlSessionOnline, launchReporter, readCreation } from './zoneHistoryLaunch'

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
async function prepare(behavior: Record<string, unknown>, monitor = false) {
  root = fs.mkdtempSync(path.join(os.tmpdir(), 'pi-zone-history-'))
  server = await startServer({ safeShutdown: monitor, herdr: { behavior: { ...behavior, manual_launch_events: true } } })
  if (monitor) await keepControlSessionOnline(server)
  const zones = await post('/api/zones', { id: 'test/zone', path: root })
  await post('/api/tasks', { name: 'Pi history browser', chatName: 'Pi Browser Chat', agent: 'pi', model: 'default', thinking: 'default', skill: null, prompt: '' })
  return zones.zones[0] as { id: string; path: string }
}
function nativeId(value: string) { return { source: 'herdr:pi', agent: 'pi', kind: 'id', value } }
function nativePath(value: string) { return { source: 'herdr:pi', agent: 'pi', kind: 'path', value } }

// S1/S2/S4, P01/P05/P17/P26: the page closes while the launch still lacks
// its exact identity and ready. Completion and history remain backend-owned.
test('Pi without connect survives page close and history is independent of current registration', async ({ page, browser }, testInfo) => {
  const sessionId = 'pi-browser-exact-id'
  const zone = await prepare({ native_session: nativeId(sessionId) })
  const historyRequests: string[] = []
  page.on('request', r => { if (r.url().includes('/api/sessions/history')) historyRequests.push(r.url()) })
  const { dialog, id } = await beginZoneChat(page, server!, zone.id)
  const report = launchReporter(server!, 'pi', zone.path, sessionId)
  await expect(dialog).not.toContainText(/历史|收录/)
  expect(historyRequests).toEqual([])
  expect((await history()).sessions).toEqual([])
  await page.close()
  await report('launch_identity')
  expect(await readCreation(server!, id)).toMatchObject({ stage: 'initializing', inputSubmitted: false })
  expect(server!.herdrCalls().filter(c => c.kind === 'agent_prompt')).toHaveLength(0)
  await report('launch_ready')
  await expect.poll(() => readCreation(server!, id)).toMatchObject({ stage: 'succeeded', inputSubmitted: true })
  await expect.poll(async () => (await history()).sessions).toMatchObject([{ sessionId, cwd: zone.path }])
  expect((await currentSnapshot(server!)).sessions).toEqual([])
  const reopened = await browser.newPage()
  try {
    await reopened.goto(server!.baseURL)
    await expect(reopened.getByTestId('session-board')).not.toContainText('Pi Browser Chat')
    await reopened.getByRole('button', { name: '查看历史会话' }).click()
    await expect(reopened.getByRole('row').filter({ hasText: 'Pi Browser Chat' })).toBeVisible()
    await reopened.screenshot({ path: testInfo.outputPath('pi-history-without-connect.png'), fullPage: true })
    await post('/api/sessions/events', { sessionId, kind: 'register', data: { cwd: zone.path, sessionName: 'Connect display name', agentType: 'pi' } })
    await reopened.getByRole('button', { name: '查看当前会话' }).click()
    await expect(reopened.getByTestId('session-board')).toContainText('Connect display name')
    await reopened.getByRole('button', { name: '查看历史会话' }).click()
    await expect(reopened.getByRole('row').filter({ hasText: 'Pi Browser Chat' })).toBeVisible()
    await post('/api/sessions/events', { sessionId, kind: 'unregister' })
    await reopened.getByRole('button', { name: '查看当前会话' }).click()
    await expect(reopened.getByTestId('session-board')).not.toContainText('Connect display name')
    await reopened.getByRole('button', { name: '查看历史会话' }).click()
    await expect(reopened.getByRole('row').filter({ hasText: 'Pi Browser Chat' })).toBeVisible()
    expect(server!.herdrCalls().filter(c => c.kind === 'agent_prompt')).toHaveLength(1)
  } finally { await reopened.close() }
})

// P19/P22/P30: startup failures belong to the accepted creation, not history.
for (const failure of ['missing-identity', 'unreadable-header'] as const) {
  test(`Pi ${failure} fails initialization without prompt, late retry or shutdown changes`, { tag: '@time-contract' }, async ({ page }, info) => {
    const sessionId = 'pi-exact-but-too-late'
    const missing = failure === 'missing-identity'
    const zone = await prepare({ native_session: nativeId(sessionId) }, true)
    const file = path.join(root!, 'unavailable-native.jsonl')
    const control = 'pi-control'
    const { id, dialog } = await beginZoneChat(page, server!, zone.id)
    const report = launchReporter(server!, 'pi', zone.path, sessionId, missing ? undefined : file)
    await post('/api/sessions/events', { sessionId: control, kind: 'register', data: { cwd: zone.path, agentType: 'pi' } })
    await post('/api/sessions/events', { sessionId: control, kind: 'state', data: { value: 'working' } })
    await post('/api/shutdown/monitor', { sessionId: control, selected: true })
    const before = await currentSnapshot(server!)
    expect(before.shutdown).toMatchObject({ phase: 'none', monitoredSessionIds: [control] })
    const saved = await history()
    expect(saved.sessions).toMatchObject([{ sessionId: control }])

    if (missing) {
      // An out-of-order ready report cannot authorize input without identity.
      await report('launch_ready')
      await expect(dialog.getByTestId('chat-create-stage')).toHaveText('正在初始化输入')
    } else {
      server!.setHerdrBehavior({ native_session: nativePath(file) })
      await report('launch_identity', 409)
    }
    const failed = dialog.getByTestId('chat-create-failure')
    await expect(failed).toContainText('创建失败（启动 agent）', { timeout: missing ? 35_000 : 10_000 })
    await expect(failed).toContainText('初始指示尚未提交；不会自动重发')
    await expect(dialog.getByTestId('chat-create-success')).toHaveCount(0)
    await expect(dialog).not.toContainText(/历史|收录|保存失败/)
    const terminal = await readCreation(server!, id)
    expect(terminal).toMatchObject({ id, stage: 'failed', inputSubmitted: false, failure: {
      stage: 'start',
      code: missing ? 'herdr-timeout' : 'herdr-unexpected-response',
      message: expect.stringContaining(missing ? 'framework ready' : 'native header unavailable'),
    } })

    const reads = server!.herdrCalls().filter(c => c.kind === 'agent_get').length
    if (!missing) fs.writeFileSync(file, JSON.stringify({ type: 'session', id: sessionId, cwd: zone.path }) + '\n')
    await report('launch_identity', 409)
    await report('launch_ready', 409)
    await page.waitForTimeout(600)
    expect(server!.herdrCalls().filter(c => c.kind === 'agent_get')).toHaveLength(reads)
    expect(server!.herdrCalls().filter(c => c.kind === 'agent_prompt' || c.kind === 'tab_close')).toHaveLength(0)
    expect(await readCreation(server!, id)).toEqual(terminal)
    expect(await history()).toEqual(saved)
    const after = await currentSnapshot(server!)
    expect(after.sessions).toEqual(before.sessions)
    expect(after.shutdown).toMatchObject({ phase: 'none', monitoredSessionIds: [control] })
    await info.attach('terminal-creation', { body: JSON.stringify(terminal), contentType: 'application/json' })
    await info.attach('delivered-server-diagnostics', { body: server!.diagnostics(), contentType: 'text/plain' })
  })
}

// The disk fault must reach the real writer but never affect the accepted prompt.
test('Pi save-failed is correlated in server diagnostics only and ends without retry', { tag: '@time-contract' }, async ({ page }, info) => {
  const sessionId = 'pi-browser-save-failure'
  const zone = await prepare({ native_session: nativeId(sessionId) })
  const { id, dialog } = await beginZoneChat(page, server!, zone.id)
  const report = launchReporter(server!, 'pi', zone.path, sessionId)
  await report('launch_identity')
  const before = await currentSnapshot(server!)
  const historyFile = path.join(path.dirname(server!.storageFile), 'session-history.json')
  // Keep empty history readable while denying publication at its destination.
  fs.writeFileSync(historyFile, '[]')
  const protectedPath = process.platform === 'win32' ? historyFile : path.dirname(historyFile)
  const originalMode = fs.statSync(protectedPath).mode
  fs.chmodSync(protectedPath, process.platform === 'win32' ? 0o444 : 0o555)
  const warning = `zone-history creation=${id} kind=pi session=${sessionId} save-failed:`
  try {
    expect(server!.herdrCalls().filter(c => c.kind === 'agent_prompt')).toHaveLength(0)
    await report('launch_ready')
    await expect(dialog.getByTestId('chat-create-success')).toHaveText('创建成功，初始指示已提交')
    await expect(dialog).not.toContainText(/历史|收录|超时|保存失败|save-failed/)
    await expect.poll(() => server!.diagnostics()).toContain(warning)
    const response = await fetch(`${server!.baseURL}/api/sessions/history`)
    expect(response.status).toBe(200)
    expect((await response.json()).sessions).toEqual([])
    expect(fs.readFileSync(historyFile, 'utf8')).toBe('[]')
  } finally { fs.chmodSync(protectedPath, originalMode) }
  const reads = server!.herdrCalls().filter(c => c.kind === 'agent_get').length
  await page.waitForTimeout(600)
  expect((await history()).sessions).toEqual([])
  expect(fs.readFileSync(historyFile, 'utf8')).toBe('[]')
  expect(server!.herdrCalls().filter(c => c.kind === 'agent_get')).toHaveLength(reads)
  expect(server!.herdrCalls().filter(c => c.kind === 'agent_prompt')).toHaveLength(1)
  const after = await currentSnapshot(server!)
  expect(after.sessions).toEqual(before.sessions)
  expect(after.shutdown).toMatchObject({ phase: 'none', monitoredSessionIds: [] })
  expect(server!.diagnostics().split(warning)).toHaveLength(2)
  await info.attach('delivered-server-diagnostics', { body: server!.diagnostics(), contentType: 'text/plain' })
})
