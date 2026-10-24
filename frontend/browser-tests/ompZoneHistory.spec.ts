import { expect, test, type Page } from '@playwright/test'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { setTimeout as delay } from 'node:timers/promises'
import { startServer, type E2EServer } from './support'
import { zoneRowFor } from './ui'

let server: E2EServer | null = null
let root = ''
test.beforeEach(() => { root = fs.mkdtempSync(path.join(os.tmpdir(), 'omp-zone-history-browser-')) })
test.afterEach(async () => {
  fs.writeFileSync(path.join(root, 'release'), 'cleanup')
  await delay(150)
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
  const zones = await post('/api/zones', { path: zone })
  await post('/api/bots', { name: 'OMP history browser', chatName: 'OMP browser {chat.id}', agent: 'omp', model: 'default', thinking: 'default', skill: null, prompt: '', extensions: [] })
  return zones.zones[0].path as string
}
async function createInPage(page: Page, zone: string) {
  await page.goto(server!.baseURL)
  await page.getByTestId('tab-files').click()
  await zoneRowFor(page, zone).getByTestId('zone-create-chat').click()
  const dialog = page.getByTestId('chat-creation-dialog')
  await dialog.locator('#chat-workspace-target').selectOption('default')
  await dialog.getByTestId('chat-input').fill('/settings')
  const name = await dialog.getByTestId('chat-name-preview').textContent()
  const accepted = page.waitForResponse(r => r.url().endsWith('/api/chats') && r.request().method() === 'POST')
  await dialog.getByTestId('chat-create-submit').click()
  const creation = await (await accepted).json()
  await expect(dialog.getByTestId('chat-create-success')).toHaveText('创建成功，初始指示已提交')
  await expect(dialog).not.toContainText(/历史收录|历史保存|identity-timeout|save-failed/)
  return { dialog, creation, name }
}
function native(id: string) { return { source: 'herdr:omp', agent: 'omp', kind: 'id', value: id } }

// P01/P05/P17: independent history is not registration. A pending external read
// must leave creation successful; closing the actual page must not cancel it.
test('OMP page closure during identity acquisition preserves history without connecting; unregister keeps history', { tag: '@time-contract' }, async ({ page, browser }) => {
  const release = path.join(root, 'release')
  server = await startServer({ herdr: { behavior: { native_session: native('omp-browser-exact-full-id'), native_cwd: 'C:/actual/native', native_session_gate: { wait_for_file: release, timeout_ms: 40000 } } } })
  const zone = await prepare()
  const { creation, name } = await createInPage(page, zone)
  await expect.poll(() => server!.herdrCalls().filter(c => c.kind === 'agent_get').length).toBe(1)
  expect((await history()).sessions).toEqual([])
  const requests: string[] = []
  page.on('request', r => { if (r.url().includes('/api/sessions/history')) requests.push(r.url()) })
  await page.waitForTimeout(300)
  expect(requests).toEqual([])
  await page.close()
  fs.writeFileSync(release, 'ready')
  await expect.poll(async () => (await history()).sessions.map((r: {sessionId:string}) => r.sessionId)).toEqual(['omp-browser-exact-full-id'])
  const fresh = await browser.newPage()
  try {
    await fresh.goto(server.baseURL)
    await expect(fresh.getByTestId('session-board')).not.toContainText(name!)
    await fresh.getByRole('button', { name: '查看历史会话' }).click()
    await expect(fresh.getByRole('region', { name: '历史会话' })).toContainText(name!)
    await post('/api/sessions/events', { sessionId: 'omp-browser-exact-full-id', kind: 'register', data: { cwd: 'C:/actual/native', agentType: 'omp', sessionName: 'Connect name must not override Chat' } })
    await fresh.getByRole('button', { name: '查看当前会话' }).click()
    await expect(fresh.getByTestId('session-board')).toContainText('Connect name must not override Chat')
    await fresh.getByRole('button', { name: '查看历史会话' }).click()
    await expect(fresh.getByRole('region', { name: '历史会话' })).toContainText(name!)
    await post('/api/sessions/events', { sessionId: 'omp-browser-exact-full-id', kind: 'unregister' })
    await fresh.getByRole('button', { name: '查看当前会话' }).click()
    await expect(fresh.getByTestId('session-board')).not.toContainText('Connect name must not override Chat')
    await fresh.getByRole('button', { name: '查看历史会话' }).click()
    await expect(fresh.getByRole('region', { name: '历史会话' })).toContainText(name!)
    expect((await history()).sessions).toHaveLength(1)
    expect(server.herdrCalls().filter(c => c.kind === 'agent_prompt')).toHaveLength(1)
    expect(server.diagnostics()).not.toContain(`creation=${creation.id}`)
    await fresh.screenshot({ path: test.info().outputPath('omp-history-after-unregister.png'), fullPage: true })
  } finally { await fresh.close() }
})

// P30: a temporarily unavailable exact header may arrive before the budget;
// taking only the physical first line (OMP title) would miss the literal ID.
test('OMP exact header becoming readable before 30 seconds records once', { tag: '@time-contract' }, async ({ page }) => {
  const file = path.join(root, 'late-native.jsonl')
  server = await startServer({ herdr: { behavior: { native_session: { source: 'herdr:omp', agent: 'omp', kind: 'path', value: file } } } })
  const { creation } = await createInPage(page, await prepare())
  await expect.poll(() => server!.herdrCalls().some(c => c.kind === 'agent_get')).toBe(true)
  await page.close()
  await delay(27000)
  fs.writeFileSync(file, '{"type":"title","title":"metadata"}\n{"type":"session","version":3,"id":"omp-header-before-deadline","cwd":"C:/late/native"}\nBODY MUST NOT BE IMPORTED\n')
  await expect.poll(async () => (await history()).sessions.map((r: {sessionId:string}) => r.sessionId), { timeout: 3000 }).toEqual(['omp-header-before-deadline'])
  const reads = server.herdrCalls().filter(c => c.kind === 'agent_get').length
  await delay(400)
  expect(server.herdrCalls().filter(c => c.kind === 'agent_get')).toHaveLength(reads)
  expect(server.diagnostics()).not.toContain(`creation=${creation.id}`)
})

for (const failure of ['missing', 'wrong-agent', 'unreadable-header'] as const) {
  // P19/P22/P30: actual delivered process logs must identify creation and reason.
  // Restoration after the warning must not restart reads or write late history.
  test(`OMP ${failure} identity expires with server-only correlated warning and no late retries`, { tag: '@time-contract' }, async ({ page }) => {
    const file = path.join(root, 'missing-native.jsonl')
    const reference = failure === 'missing' ? null : failure === 'wrong-agent'
      ? { source: 'herdr:pi', agent: 'pi', kind: 'id', value: 'must-not-guess-omp' }
      : { source: 'herdr:omp', agent: 'omp', kind: 'path', value: file }
    server = await startServer({ herdr: { behavior: { native_session: reference, native_cwd: 'C:/reported' } } })
    const { creation, dialog } = await createInPage(page, await prepare())
    await expect.poll(() => server!.diagnostics(), { timeout: 33000 }).toContain(`creation=${creation.id}`)
    const diagnostic = server.diagnostics().split('\n').find(line => line.includes(`creation=${creation.id}`))!
    expect(diagnostic).toContain('identity-timeout')
    expect(diagnostic).toContain('kind=omp')
    expect(diagnostic).toContain(failure === 'missing' ? 'not available' : failure === 'wrong-agent' ? 'does not match' : 'header unavailable')
    await expect(dialog.getByTestId('chat-create-success')).toHaveText('创建成功，初始指示已提交')
    await expect(dialog).not.toContainText(/历史收录|历史保存|identity-timeout|save-failed/)
    const reads = server.herdrCalls().filter(c => c.kind === 'agent_get').length
    server.setHerdrBehavior({ native_session: native('too-late-native-id'), native_cwd: 'C:/late' })
    await page.waitForTimeout(800)
    expect(server.herdrCalls().filter(c => c.kind === 'agent_get')).toHaveLength(reads)
    expect((await history()).sessions).toEqual([])
    expect(server.herdrCalls().filter(c => c.kind === 'agent_prompt')).toHaveLength(1)
    fs.writeFileSync(test.info().outputPath('actual-backend-warning.txt'), diagnostic + '\n')
    await test.info().attach('actual-backend-warning', { path: test.info().outputPath('actual-backend-warning.txt'), contentType: 'text/plain' })
  })
}

// P19/P22/P26/P30: real write failure, not a fake receipt. Restoring writable
// storage cannot autonomously retry; creation and live registration stay intact.
test('OMP history save failure leaves creation and current state intact and never retries the write', { tag: '@time-contract' }, async ({ page }) => {
  const release = path.join(root, 'release')
  server = await startServer({ herdr: { behavior: { native_session: native('omp-save-failure-id'), native_cwd: 'C:/native', native_session_gate: { wait_for_file: release, timeout_ms: 40000 } } } })
  const { creation, dialog } = await createInPage(page, await prepare())
  await post('/api/sessions/events', { sessionId: 'unrelated-live-omp', kind: 'register', data: { cwd: 'C:/current', sessionName: 'Current stays', agentType: 'omp' } })
  await post('/api/sessions/events', { sessionId: 'unrelated-live-omp', kind: 'state', data: { value: 'working' } })
  const historyFile = path.join(path.dirname(server.configPath), 'session-history.json')
  const original = fs.readFileSync(historyFile)
  fs.rmSync(historyFile); fs.mkdirSync(historyFile)
  fs.writeFileSync(release, 'ready')
  await expect.poll(() => server!.diagnostics()).toContain(`creation=${creation.id}`)
  const diagnostic = server.diagnostics().split('\n').find(line => line.includes(`creation=${creation.id}`))!
  expect(diagnostic).toContain('save-failed')
  expect(diagnostic).toContain('session=omp-save-failure-id')
  await expect(dialog.getByTestId('chat-create-success')).toHaveText('创建成功，初始指示已提交')
  await expect(dialog).not.toContainText(/历史收录|历史保存|save-failed/)
  fs.rmdirSync(historyFile); fs.writeFileSync(historyFile, original)
  const calls = server.herdrCalls().filter(c => c.kind === 'agent_get').length
  await page.waitForTimeout(800)
  expect(server.herdrCalls().filter(c => c.kind === 'agent_get')).toHaveLength(calls)
  expect((await history()).sessions.map((r: {sessionId:string}) => r.sessionId)).toEqual(['unrelated-live-omp'])
  await dialog.getByTestId('chat-create-close').click()
  await page.getByRole('tab', { name: 'chat', exact: true }).click()
  await expect(page.getByTestId('session-board')).toContainText('Current stays')
  fs.writeFileSync(test.info().outputPath('actual-backend-warning.txt'), diagnostic + '\n')
    await test.info().attach('actual-backend-warning', { path: test.info().outputPath('actual-backend-warning.txt'), contentType: 'text/plain' })
})
