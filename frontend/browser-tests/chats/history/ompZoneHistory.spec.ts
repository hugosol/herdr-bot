import { expect, test } from '@playwright/test'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { setTimeout as delay } from 'node:timers/promises'
import { startServer, type E2EServer } from '../../support/support'
import { beginZoneChat, currentSnapshot, keepControlSessionOnline, launchReporter, readCreation } from './zoneHistoryLaunch'

// Parallel cases run in separate worker processes; each keeps its own server and cleanup hooks.
test.describe.configure({ mode: 'parallel' })

let server: E2EServer | null = null
let root = ''
test.beforeEach(() => { root = fs.mkdtempSync(path.join(os.tmpdir(), 'omp-zone-history-browser-')) })
test.afterEach(async () => {
  try { await server?.cleanup() }
  finally {
    server = null
    fs.rmSync(root, { recursive: true, force: true })
  }
})
async function post(endpoint: string, body: unknown) {
  const response = await fetch(`${server!.baseURL}${endpoint}`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify(body) })
  expect(response.status).toBeGreaterThanOrEqual(200)
  expect(response.status).toBeLessThan(300)
  return response.json()
}
async function history() { return fetch(`${server!.baseURL}/api/sessions/history`).then(r => r.json()) }
async function prepare() {
  const zone = path.join(root, 'zone'); fs.mkdirSync(zone)
  const zones = await post('/api/zones', { id: 'test/zone', path: zone })
  await post('/api/tasks', { name: 'OMP history browser', chatName: 'OMP browser {chat.id}', agent: 'omp', model: 'default', thinking: 'default', skill: null, prompt: '' })
  return zones.zones[0] as { id: string; path: string }
}
function native(id: string) { return { source: 'herdr:omp', agent: 'omp', kind: 'id', value: id } }

// P01/P05/P17: close the page with an exact launch still initializing. A later
// matched ready event must complete creation and persist history without registering.
test('OMP page closure during initialization preserves history without connecting; unregister keeps history', { tag: '@time-contract' }, async ({ page, browser }) => {
  const sessionId = 'omp-browser-exact-full-id'
  server = await startServer({ herdr: { behavior: { manual_launch_events: true, native_session: native(sessionId) } } })
  const zone = await prepare()
  const requests: string[] = []
  page.on('request', r => { if (r.url().includes('/api/sessions/history')) requests.push(r.url()) })
  const { dialog, id, name } = await beginZoneChat(page, server, zone.id)
  const report = launchReporter(server, 'omp', zone.path, sessionId)
  await report('launch_identity')
  await expect(dialog.getByTestId('chat-create-stage')).toHaveText('正在初始化输入')
  await expect(dialog).not.toContainText(/历史收录|历史保存/)
  expect(server.herdrCalls().filter(c => c.kind === 'agent_prompt')).toHaveLength(0)
  expect((await history()).sessions).toEqual([])
  await page.waitForTimeout(300)
  expect(requests).toEqual([])
  await page.close()
  await report('launch_ready')
  await expect.poll(() => readCreation(server!, id)).toMatchObject({ stage: 'succeeded', inputSubmitted: true })
  await expect.poll(async () => (await history()).sessions).toMatchObject([{ sessionId, cwd: zone.path }])
  expect((await currentSnapshot(server)).sessions).toEqual([])
  const fresh = await browser.newPage()
  try {
    await fresh.goto(server.baseURL)
    await expect(fresh.getByTestId('session-board')).not.toContainText(name)
    await fresh.getByRole('button', { name: '查看历史会话' }).click()
    await expect(fresh.getByRole('region', { name: '历史会话' })).toContainText(name)
    await post('/api/sessions/events', { sessionId, kind: 'register', data: { cwd: zone.path, agentType: 'omp', sessionName: 'Connect name must not override Chat' } })
    await fresh.getByRole('button', { name: '查看当前会话' }).click()
    await expect(fresh.getByTestId('session-board')).toContainText('Connect name must not override Chat')
    await fresh.getByRole('button', { name: '查看历史会话' }).click()
    await expect(fresh.getByRole('region', { name: '历史会话' })).toContainText(name)
    await post('/api/sessions/events', { sessionId, kind: 'unregister' })
    await fresh.getByRole('button', { name: '查看当前会话' }).click()
    await expect(fresh.getByTestId('session-board')).not.toContainText('Connect name must not override Chat')
    await fresh.getByRole('button', { name: '查看历史会话' }).click()
    await expect(fresh.getByRole('region', { name: '历史会话' })).toContainText(name)
    expect((await history()).sessions).toHaveLength(1)
    expect(server.herdrCalls().filter(c => c.kind === 'agent_prompt')).toHaveLength(1)
    expect(server.diagnostics()).not.toContain(`creation=${id}`)
    await fresh.screenshot({ path: test.info().outputPath('omp-history-after-unregister.png'), fullPage: true })
  } finally { await fresh.close() }
})

// P30: a temporarily unavailable exact header may arrive before the budget;
// taking only the physical first line (OMP title) would miss the literal ID.
test('OMP exact header becoming readable at 29 seconds records once', { tag: '@time-contract' }, async ({ page }) => {
  const file = path.join(root, 'late-native.jsonl')
  const sessionId = 'omp-header-before-deadline'
  server = await startServer({ herdr: { behavior: { manual_launch_events: true, native_session: { source: 'herdr:omp', agent: 'omp', kind: 'path', value: file } } } })
  const zone = await prepare()
  const { id } = await beginZoneChat(page, server, zone.id)
  const startedAt = server.herdrCalls().find(c => c.kind === 'agent_start')?.at as number
  expect(startedAt).toBeGreaterThan(0)
  const report = launchReporter(server, 'omp', zone.path, sessionId, file)
  await page.close()
  await delay(Math.max(0, startedAt + 28_500 - Date.now()))
  expect(await readCreation(server, id)).toMatchObject({ stage: 'initializing', inputSubmitted: false })
  expect((await history()).sessions).toEqual([])
  expect(server.herdrCalls().filter(c => c.kind === 'agent_prompt')).toHaveLength(0)
  const bytes = '{"type":"title","title":"metadata"}\n{"type":"session","version":3,"id":"omp-header-before-deadline","cwd":"C:/old/header-directory"}\nBODY MUST NOT BE IMPORTED\n'
  await delay(Math.max(0, startedAt + 29_000 - Date.now()))
  fs.writeFileSync(file, bytes)
  await report('launch_identity')
  await report('launch_ready')
  await expect.poll(() => readCreation(server!, id)).toMatchObject({ stage: 'succeeded', inputSubmitted: true })
  await expect.poll(async () => (await history()).sessions).toMatchObject([{ sessionId, cwd: zone.path }])
  const reads = server.herdrCalls().filter(c => c.kind === 'agent_get').length
  await delay(400)
  expect(server.herdrCalls().filter(c => c.kind === 'agent_get')).toHaveLength(reads)
  expect(server.herdrCalls().filter(c => c.kind === 'agent_prompt')).toHaveLength(1)
  expect(fs.readFileSync(file, 'utf8')).toBe(bytes)
  expect(server.diagnostics()).not.toContain(`creation=${id}`)
})

for (const failure of ['missing', 'wrong-agent', 'unreadable-header'] as const) {
  // P19/P22/P30: the correlated creation fails at startup, not in history.
  // Even valid evidence arriving afterward cannot reopen the launch.
  test(`OMP ${failure} identity fails initialization without prompt, late retry or shutdown changes`, { tag: '@time-contract' }, async ({ page }, info) => {
    const file = path.join(root, 'missing-native.jsonl')
    const sessionId = 'omp-exact-but-too-late'
    const reference = failure === 'missing' ? null : failure === 'wrong-agent'
      ? { source: 'herdr:pi', agent: 'pi', kind: 'id', value: sessionId }
      : { source: 'herdr:omp', agent: 'omp', kind: 'path', value: file }
    server = await startServer({ safeShutdown: true, herdr: { behavior: { manual_launch_events: true, native_session: reference } } })
    await keepControlSessionOnline(server)
    const zone = await prepare()
    const { id, dialog } = await beginZoneChat(page, server, zone.id)
    const report = launchReporter(server, 'omp', zone.path, sessionId, failure === 'unreadable-header' ? file : undefined)
    await post('/api/sessions/events', { sessionId: 'omp-control', kind: 'register', data: { cwd: zone.path, agentType: 'omp' } })
    await post('/api/sessions/events', { sessionId: 'omp-control', kind: 'state', data: { value: 'working' } })
    await post('/api/shutdown/monitor', { sessionId: 'omp-control', selected: true })
    const before = await currentSnapshot(server)
    expect(before.shutdown).toMatchObject({ phase: 'none', monitoredSessionIds: ['omp-control'] })
    const saved = await history()
    expect(saved.sessions).toMatchObject([{ sessionId: 'omp-control' }])

    if (failure === 'missing') {
      await report('launch_identity')
      await expect(dialog.getByTestId('chat-create-stage')).toHaveText('正在初始化输入')
    } else {
      await report('launch_identity', 409)
    }
    const failed = dialog.getByTestId('chat-create-failure')
    await expect(failed).toContainText('创建失败（启动 agent）', { timeout: failure === 'missing' ? 35_000 : 10_000 })
    await expect(failed).toContainText('初始指示尚未提交；不会自动重发')
    await expect(dialog.getByTestId('chat-create-success')).toHaveCount(0)
    await expect(dialog).not.toContainText(/历史收录|历史保存|identity-timeout|save-failed/)
    const terminal = await readCreation(server, id)
    const code = failure === 'missing' ? 'herdr-timeout' : 'herdr-unexpected-response'
    const reason = failure === 'missing' ? 'framework ready' : failure === 'wrong-agent'
      ? 'native reference agent/source does not match this creation'
      : 'native header unavailable'
    expect(terminal).toMatchObject({ id, stage: 'failed', inputSubmitted: false, failure: { stage: 'start', code, message: expect.stringContaining(reason) } })
    const reads = server.herdrCalls().filter(c => c.kind === 'agent_get').length
    if (failure === 'unreadable-header') fs.writeFileSync(file, JSON.stringify({ type: 'session', id: sessionId, cwd: zone.path }) + '\n')
    server.setHerdrBehavior({ native_session: failure === 'unreadable-header' ? reference : native(sessionId) })
    await report('launch_identity', 409)
    await report('launch_ready', 409)
    await page.waitForTimeout(800)
    expect(server.herdrCalls().filter(c => c.kind === 'agent_get')).toHaveLength(reads)
    expect(server.herdrCalls().filter(c => c.kind === 'agent_prompt' || c.kind === 'tab_close')).toHaveLength(0)
    expect(await readCreation(server, id)).toEqual(terminal)
    expect(await history()).toEqual(saved)
    const after = await currentSnapshot(server)
    expect(after.sessions).toEqual(before.sessions)
    expect(after.shutdown).toMatchObject({ phase: 'none', monitoredSessionIds: ['omp-control'] })
    await info.attach('terminal-creation', { body: JSON.stringify(terminal), contentType: 'application/json' })
    await info.attach('delivered-server-diagnostics', { body: server.diagnostics(), contentType: 'text/plain' })
  })
}

// P19/P22/P26/P30: real write failure, not a fake receipt. Restoring writable
// storage cannot autonomously retry; creation and live registration stay intact.
test('OMP history save failure leaves creation and current state intact and never retries the write', { tag: '@time-contract' }, async ({ page }) => {
  const sessionId = 'omp-save-failure-id'
  const control = 'unrelated-live-omp'
  server = await startServer({ safeShutdown: true, herdr: { behavior: { manual_launch_events: true, native_session: native(sessionId) } } })
  const zone = await prepare()
  const { id, dialog } = await beginZoneChat(page, server, zone.id)
  const report = launchReporter(server, 'omp', zone.path, sessionId)
  await report('launch_identity')
  await post('/api/sessions/events', { sessionId: control, kind: 'register', data: { cwd: zone.path, sessionName: 'Current stays', agentType: 'omp' } })
  await post('/api/sessions/events', { sessionId: control, kind: 'state', data: { value: 'working' } })
  await post('/api/shutdown/monitor', { sessionId: control, selected: true })
  const before = await currentSnapshot(server)
  expect(before.shutdown).toMatchObject({ phase: 'none', monitoredSessionIds: [control] })
  const saved = await history()
  expect(saved.sessions).toMatchObject([{ sessionId: control }])
  const historyFile = path.join(path.dirname(server.configPath), 'session-history.json')
  const original = fs.readFileSync(historyFile)
  fs.rmSync(historyFile)
  fs.mkdirSync(historyFile)
  const warning = `zone-history creation=${id} kind=omp session=${sessionId} save-failed:`
  try {
    expect(server.herdrCalls().filter(c => c.kind === 'agent_prompt')).toHaveLength(0)
    await report('launch_ready')
    await expect(dialog.getByTestId('chat-create-success')).toHaveText('创建成功，初始指示已提交')
    await expect(dialog).not.toContainText(/历史收录|历史保存|save-failed/)
    await expect.poll(() => server!.diagnostics()).toContain(warning)
  } finally {
    fs.rmdirSync(historyFile)
    fs.writeFileSync(historyFile, original)
  }
  const calls = server.herdrCalls().filter(c => c.kind === 'agent_get').length
  await page.waitForTimeout(800)
  expect(server.herdrCalls().filter(c => c.kind === 'agent_get')).toHaveLength(calls)
  expect(server.herdrCalls().filter(c => c.kind === 'agent_prompt')).toHaveLength(1)
  expect(server.diagnostics().split(warning)).toHaveLength(2)
  expect(await history()).toEqual(saved)
  const after = await currentSnapshot(server)
  expect(after.sessions).toEqual(before.sessions)
  expect(after.shutdown).toMatchObject({ phase: 'none', monitoredSessionIds: [control] })
  await dialog.getByTestId('chat-create-close').click()
  await page.getByRole('tab', { name: 'chat', exact: true }).click()
  await expect(page.getByTestId('session-board')).toContainText('Current stays')
  await test.info().attach('actual-backend-warning', { body: server.diagnostics().split('\n').find(line => line.includes(warning))!, contentType: 'text/plain' })
})
