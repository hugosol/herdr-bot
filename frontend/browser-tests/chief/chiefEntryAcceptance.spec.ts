import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { createHash } from 'node:crypto'
import { fileURLToPath } from 'node:url'
import { zoneRowFor } from '../support/ui'
import { expect, test, type Page } from '@playwright/test'
import { startServer, type E2EServer } from '../support/support'

// Approved S1/S2; external Herdr is controlled, never a P7/model oracle.
// P29 violation: claiming only after registration or per zone/Task lets both
// overlapping delivered forms launch. A real CLI file gate holds the winner
// before native identity/registration while independent browser/API callers race.
// P30 violation: treating offline as absent or retaining after expiry disagrees
// with public snapshots and actual delivered-entry interactions at both thresholds.
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../..')
const hosts = (process.env.CHIEF_ENTRY_HOSTS ?? 'omp,pi').split(',') as ('omp' | 'pi')[]
const envKeys = ['HOME', 'USERPROFILE', 'APPDATA', 'LOCALAPPDATA', 'XDG_CONFIG_HOME', 'XDG_DATA_HOME', 'XDG_STATE_HOME', 'PI_CODING_AGENT_DIR', 'OMP_CODING_AGENT_DIR']
let server: E2EServer | undefined
let home: string
let original: Record<string, string | undefined>
let report: Record<string, unknown>
let output: string | undefined
let bodyReads: Promise<void>[] = []
let observed = new WeakSet<Page>()

function observe(page: Page) {
  if (observed.has(page)) return
  observed.add(page)
  page.on('response', response => {
    if (!response.url().includes('/api/')) return
    const item: Record<string, unknown> = { url: response.url(), status: response.status(), method: response.request().method(), request: response.request().postData(), at: performance.now() }
    ;(report.browserHttp as unknown[]).push(item)
    if (!response.headers()['content-type']?.includes('text/event-stream')) {
      bodyReads.push(response.text().then(body => { item.response = body }, error => { item.bodyError = String(error) }))
    }
  })
}

test.use(process.env.CHIEF_ENTRY_CHROME ? { channel: undefined, launchOptions: { executablePath: process.env.CHIEF_ENTRY_CHROME } } : {})
test.beforeEach(async ({ browser }) => {
  void browser.version()
  home = fs.mkdtempSync(path.join(os.tmpdir(), 'chief-acceptance-home-'))
  original = Object.fromEntries(envKeys.map(key => [key, process.env[key]]))
  for (const key of envKeys) {
    process.env[key] = path.join(home, key)
    fs.mkdirSync(process.env[key]!)
  }
  report = { passed: false, observations: [], http: [], browserHttp: [], snapshots: [], controlledHerdr: true, modelCalls: 0 }
  bodyReads = []
  observed = new WeakSet<Page>()
  output = undefined
})
test.afterEach(async ({ browser }, info) => {
  const errors: string[] = []
  try {
    await Promise.all(bodyReads)
    report.browser = { version: browser.version(), executable: process.env.CHIEF_ENTRY_CHROME ?? 'chrome channel' }
    report.calls = server?.herdrCalls()
    report.tabs = server?.herdrTabs()
    report.diagnostics = server?.diagnostics()
    if (server) {
      report.configuration = fs.readFileSync(server.configPath, 'utf8')
      report.metadata = fs.readFileSync(server.metadataFile, 'utf8')
    }
    report.status = info.status
    try {
      await server?.cleanup()
      report.backendExitConfirmed = true
    } catch (error) { errors.push(String(error)) }
    report.cleanupErrors = errors
    report.passed = info.status === 'passed' && errors.length === 0
    const target = process.env.CARGO_TARGET_DIR ?? path.join(root, 'backend/target')
    report.artifacts = [path.join(target, 'debug/herdr-bot.exe'), path.join(target, 'debug/herdr-fixture.exe'), ...['backend/static/index.html', 'backend/Cargo.lock', 'frontend/bun.lock', 'frontend/browser-tests/chief/chiefEntryAcceptance.spec.ts', 'backend/src/chief.rs', 'backend/src/sessions/mod.rs', 'backend/src/tasks/catalog.rs', 'meta-data/task-options.toml'].map(file => path.join(root, file))].map(file => ({ file, sha256: createHash('sha256').update(fs.readFileSync(file)).digest('hex') }))
    const evidence = info.outputPath('chief-entry-acceptance.json')
    fs.writeFileSync(evidence, JSON.stringify(report, null, 2))
    await info.attach('chief-entry-acceptance', { path: evidence, contentType: 'application/json' })
    if (output) {
      fs.mkdirSync(path.dirname(output), { recursive: true })
      fs.writeFileSync(output, JSON.stringify(report, null, 2))
    }
  } finally {
    server = undefined
    for (const key of envKeys) {
      if (original[key] === undefined) delete process.env[key]
      else process.env[key] = original[key]
    }
    if (errors.length === 0) fs.rmSync(home, { recursive: true, force: true })
  }
  expect(errors).toEqual([])
})

async function request(endpoint: string, body?: unknown, status = 200) {
  const response = await fetch(server!.baseURL + endpoint, body === undefined ? undefined : {
    method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify(body),
  })
  const result = await response.json()
  ;(report.http as unknown[]).push({ endpoint, body, status: response.status, result, at: Date.now() })
  expect(response.status, JSON.stringify(result)).toBe(status)
  return result
}
async function prepare(host: 'omp' | 'pi', promise: string) {
  if (process.env.CHIEF_ENTRY_EVIDENCE_DIR) output = path.join(process.env.CHIEF_ENTRY_EVIDENCE_DIR, `${host}-${promise}.json`)
  report.host = host
  report.promise = promise
  server = await startServer({ herdr: {} })
  const tasks: string[] = []
  for (const name of ['alpha', 'beta']) {
    const cwd = path.join(server.cwd, name)
    fs.mkdirSync(cwd)
    await request('/api/zones', { id: `entry/${name}`, path: cwd })
    const result = await request('/api/tasks', { name: `entry-${name}`, taskType: 'chief', agent: host, chatName: '{chat.id}', model: 'default', thinking: 'default', skill: null, prompt: '', suite: null })
    tasks.push(result.tasks.find((task: { name: string }) => task.name === `entry-${name}`).id)
  }
  return tasks
}
async function form(page: Page, task: string, zone: string, chief = true) {
  observe(page)
  await page.goto(server!.baseURL)
  if (chief) await page.getByRole('button', { name: 'chief bot', exact: true }).click()
  else {
    await page.getByTestId('tab-files').click()
    await zoneRowFor(page, zone).getByTestId('zone-create-chat').click()
  }
  const dialog = page.getByTestId('chat-creation-dialog')
  await expect(dialog).toBeVisible()
  await dialog.getByTestId('task').selectOption(task)
  await dialog.getByTestId('chat-zone').selectOption(zone)
  await dialog.locator('#chat-workspace-target').selectOption('default')
  await dialog.getByTestId('chat-input').fill('只读 chief 入口竞争验收')
  await expect(dialog.getByTestId('chat-create-submit')).toBeEnabled()
  return dialog
}
async function snapshot() {
  const response = await fetch(server!.baseURL + '/api/sessions/events', { signal: AbortSignal.timeout(10000) })
  const reader = response.body!.getReader()
  let content = ''
  try {
    while (!content.includes('\n\n')) {
      const next = await reader.read()
      expect(next.done).toBe(false)
      content += new TextDecoder().decode(next.value).replace(/\r/g, '')
    }
    const value = JSON.parse(content.split('\n').find(line => line.startsWith('data:'))!.slice(5))
    ;(report.snapshots as unknown[]).push({ at: performance.now(), value })
    return value
  } finally { await reader.cancel() }
}
async function register(host: string, id: string, cwd: string, native: string) {
  return request('/api/sessions/events', { sessionId: id, kind: 'register', data: { agentType: host, cwd, sessionFile: native, sessionName: id } })
}

for (const host of hosts) {
  test(`${host} P29 browsers and public calls share the pre-registration chief claim`, async ({ page, browser }, info) => {
    const tasks = await prepare(host, 'P29')
    // Give a competing registration/resume genuine chief provenance. An
    // arbitrary made-up ID would merely be an ordinary session, not this race.
    const prior = `${host}-prior-chief`
    const priorCwd = path.join(server!.cwd, 'beta')
    const priorFile = path.join(server!.cwd, 'prior-chief.jsonl')
    fs.writeFileSync(priorFile, JSON.stringify({ type: 'session', version: 3, id: prior, cwd: priorCwd }) + '\n')
    server!.setHerdrBehavior({ native_session: { source: `herdr:${host}`, agent: host, kind: 'path', value: priorFile } })
    const priorPreview = await request('/api/chats/preview', { taskId: tasks[1], zone: 'entry/beta' })
    const priorStart = await request('/api/chats', { taskId: tasks[1], zone: 'entry/beta', chatId: priorPreview.chatId, input: '原 chief 身份预置' }, 202)
    await expect.poll(async () => (await request('/api/chats/' + priorStart.id)).stage, { timeout: 20000 }).toBe('succeeded')
    await register(host, prior, priorCwd, priorFile)
    await request('/api/sessions/events', { sessionId: prior, kind: 'unregister' })
    const baselineStarts = server!.herdrCalls().filter(call => call.kind === 'agent_start').length
    const context = await browser.newContext()
    const gate = path.join(server!.cwd, 'release-chief-start')
    try {
      const other = await context.newPage()
      const first = await form(page, tasks[0], 'entry/alpha')
      const second = await form(other, tasks[1], 'entry/beta', false)
      const native = path.join(server!.cwd, 'native-race.jsonl')
      // The winner's cwd is supplied by the actual launch; header is a controlled
      // external native identity, not evidence that an actual host consumed tools.
      const id = `${host}-entry-race`
      fs.writeFileSync(native, JSON.stringify({ type: 'session', version: 3, id, cwd: path.join(server!.cwd, 'alpha') }) + '\n')
      server!.setHerdrBehavior({ tab_create_gate: { wait_for_file: gate, timeout_ms: 30000 }, native_session: { source: `herdr:${host}`, agent: host, kind: 'path', value: native } })
      const replies = [page, other].map(p => p.waitForResponse(r => r.url().endsWith('/api/chats') && r.request().method() === 'POST'))
      await Promise.all([first.getByTestId('chat-create-submit').click(), second.getByTestId('chat-create-submit').click()])
      const responses = await Promise.all(replies)
      expect(responses.map(r => r.status()).sort()).toEqual([202, 409])
      const accepted = await responses.find(r => r.status() === 202)!.json()
      const winnerBody = responses.find(r => r.status() === 202)!.request().postDataJSON()
      fs.writeFileSync(native, JSON.stringify({ type: 'session', version: 3, id, cwd: path.join(server!.cwd, winnerBody.zone.endsWith('alpha') ? 'alpha' : 'beta') }) + '\n')
      await expect.poll(() => server!.herdrCalls().some(call => call.kind === 'tab_create')).toBe(true)
      const claim = (await request('/api/chief')).current
      expect(claim.creationId).toBe(accepted.id)
      expect(claim.sessionId).toBeUndefined()
      for (const p of [page, other]) {
        await p.getByTestId('chat-create-close').click()
        await p.getByRole('button', { name: 'chief bot', exact: true }).click()
        await expect(p.getByTestId('chat-creation-dialog')).toHaveCount(0)
        await expect(p.getByRole('alert')).toContainText('不会另建 Chat')
      }
      const previews = await Promise.all(tasks.map((taskId, index) => request('/api/chats/preview', { taskId, zone: `entry/${index ? 'beta' : 'alpha'}` })))
      await Promise.all([
        ...tasks.map((taskId, index) => request('/api/chats', { taskId, zone: `entry/${index ? 'beta' : 'alpha'}`, chatId: previews[index].chatId, input: '并发公开调用' }, 409)),
        request('/api/sessions/events', { sessionId: prior, kind: 'register', data: { agentType: host, cwd: priorCwd, sessionFile: priorFile } }, 409),
        request('/api/chats/resume', { sessionId: prior }, 409),
      ])
      fs.writeFileSync(gate, 'release')
      await expect.poll(async () => (await request('/api/chats/' + accepted.id)).stage, { timeout: 20000 }).toBe('succeeded')
      const cwd = path.join(server!.cwd, winnerBody.zone.endsWith('alpha') ? 'alpha' : 'beta')
      await register(host, id, cwd, native)
      expect((await request('/api/chief/qualify', { sessionId: id })).current.sessionId).toBe(id)
      await request('/api/chief/qualify', { sessionId: prior }, 409)
      expect(server!.herdrCalls().filter(call => call.kind === 'agent_start')).toHaveLength(baselineStarts + 1)
      expect((await request('/api/chief')).current.sessionId).toBe(id)
      await page.screenshot({ path: info.outputPath('exclusive-chief.png'), fullPage: true })
    } finally {
      fs.writeFileSync(gate, 'release-even-on-failure')
      await context.close()
    }
  })

  test(`${host} P30 offline retention restores, expires and unregisters with real page feedback`, { tag: '@time-contract' }, async ({ page, browser }, info) => {
    test.setTimeout(180000)
    const [taskId] = await prepare(host, 'P30')
    await server!.stop()
    fs.writeFileSync(server!.configPath, fs.readFileSync(server!.configPath, 'utf8').replace('[sessions]', '[sessions]\nonline_timeout_seconds = 10\noffline_retention_seconds = 30'))
    await server!.restart()
    // Actual shipped defaults (10s online, 30s offline) and 5s maintenance scan.
    // Real monotonic observation controls the last accepted public report; no
    // browser-only clock or private backend state mutation stands in for time.
    report.budgets = { onlineSeconds: 10, offlineSeconds: 30, maintenanceSeconds: 5, startupSeconds: 30, assertionSeconds: 10, thresholdObservationSeconds: 46 }
    const context = await browser.newContext()
    try {
      const other = await context.newPage()
      const cwd = path.join(server!.cwd, 'alpha')
      const id = `${host}-entry-lifecycle`
      const native = path.join(server!.cwd, 'native-lifecycle.jsonl')
      fs.writeFileSync(native, JSON.stringify({ type: 'session', version: 3, id, cwd }) + '\n')
      server!.setHerdrBehavior({ native_session: { source: `herdr:${host}`, agent: host, kind: 'path', value: native } })
      const preview = await request('/api/chats/preview', { taskId, zone: 'entry/alpha' })
      const started = await request('/api/chats', { taskId, zone: 'entry/alpha', chatId: preview.chatId, input: '生命周期验收' }, 202)
      await expect.poll(async () => (await request('/api/chats/' + started.id)).stage, { timeout: 20000 }).toBe('succeeded')
      await register(host, id, cwd, native)
      let lastReport = performance.now()
      server!.setHerdrBehavior({ agents: [{ agent: host, workspace_id: 'w-target', tab_id: 'retained-tab', pane_id: 'retained-pane', agent_session: { source: `herdr:${host}`, agent: host, kind: 'id', value: id } }] })
      for (const p of [page, other]) { observe(p); await p.goto(server!.baseURL) }
      expect((await snapshot()).sessions.find((row: { sessionId: string }) => row.sessionId === id).online).toBe(true)
      await expect.poll(async () => (await snapshot()).sessions.find((row: { sessionId: string }) => row.sessionId === id)?.online, { timeout: 16000, intervals: [250] }).toBe(false)
      const offlineAt = performance.now() - lastReport
      expect(offlineAt).toBeGreaterThan(10000)
      expect(offlineAt).toBeLessThan(40000)
      for (const p of [page, other]) {
        const located = p.waitForResponse(response => response.url().endsWith('/api/chats/resume'))
        await p.getByRole('button', { name: 'chief bot', exact: true }).click()
        const reply = await located
        expect(reply.status()).toBe(200)
        const value = await reply.json()
        expect(value.outcome).toBe('located')
        await expect(p.getByText(value.message, { exact: true })).toBeVisible()
        await expect(p.getByTestId('chat-creation-dialog')).toHaveCount(0)
      }
      expect((await request('/api/chief')).current.sessionId).toBe(id)
      await request('/api/chats', { taskId, zone: 'entry/alpha', chatId: (await request('/api/chats/preview', { taskId, zone: 'entry/alpha' })).chatId, input: '不得另建' }, 409)
      await request('/api/sessions/events', { sessionId: id, kind: 'heartbeat' })
      lastReport = performance.now()
      expect((await snapshot()).sessions.find((row: { sessionId: string }) => row.sessionId === id).online).toBe(true)
      expect((await request('/api/chief/qualify', { sessionId: id })).current.sessionId).toBe(id)
      await expect.poll(async () => (await request('/api/chief')).current, { timeout: 46000, intervals: [250] }).toBeNull()
      const removedAt = performance.now() - lastReport
      expect(removedAt).toBeGreaterThan(40000)
      expect((await snapshot()).sessions.some((row: { sessionId: string }) => row.sessionId === id)).toBe(false)
      await request('/api/chief/qualify', { sessionId: id }, 409)
      for (const p of [page, other]) {
        await p.getByRole('button', { name: 'chief bot', exact: true }).click()
        const available = p.getByTestId('chat-creation-dialog')
        await expect(available).toBeVisible()
        await available.getByTestId('task').selectOption(taskId)
        await available.getByTestId('chat-zone').selectOption('entry/alpha')
        await available.getByTestId('chat-input').fill('可发起但本次不提交')
        await expect(available.getByTestId('chat-create-submit')).toBeEnabled()
        await available.getByTestId('chat-create-close').click()
      }
      // Explicit user recovery is permitted after removal. This is navigation of
      // a controlled still-existing process, not a claim about process exit.
      await request('/api/chats/resume', { sessionId: id })
      await register(host, id, cwd, native)
      expect((await request('/api/chief')).current.sessionId).toBe(id)
      await request('/api/sessions/events', { sessionId: id, kind: 'unregister' })
      expect((await request('/api/chief')).current).toBeNull()
      await page.getByRole('button', { name: 'chief bot', exact: true }).click()
      const available = page.getByTestId('chat-creation-dialog')
      await expect(available).toBeVisible()
      await available.getByTestId('task').selectOption(taskId)
      await available.getByTestId('chat-zone').selectOption('entry/alpha')
      await available.getByTestId('chat-input').fill('注销后可创建但本次不提交')
      await expect(available.getByTestId('chat-create-submit')).toBeEnabled()
      ;(report.observations as unknown[]).push({ offlineAtMs: offlineAt, removedAtMs: removedAt, beforeAndAfterHeartbeat: true, explicitUnregister: true, processExitInferred: false })
      await page.screenshot({ path: info.outputPath('chief-available-after-unregister.png'), fullPage: true })
    } finally { await context.close() }
  })
}
