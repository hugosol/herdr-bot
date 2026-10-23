import { expect, request, test, type Page, type TestInfo } from '@playwright/test'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { startServer, type E2EServer } from './support'

// Approved seams S1/S2: delivered browser, independent HTTP clients and an
// isolated backend process restart. Only external Herdr identity is controlled;
// tickets 05/06 own the real S4 identity chain. Never invoke shutdown here.
let server: E2EServer | null = null
let root: string

test.beforeEach(() => { root = fs.mkdtempSync(path.join(os.tmpdir(), 'cross-source-history-')) })
test.afterEach(async () => {
  await server?.cleanup(); server = null
  fs.rmSync(root, { recursive: true, force: true })
})
async function post(endpoint: string, body: unknown) {
  const response = await fetch(`${server!.baseURL}${endpoint}`, {
    method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify(body),
  })
  expect(response.ok, await response.clone().text()).toBe(true)
  return response.json()
}
async function evidence(info: TestInfo, name: string, value: unknown) {
  const file = info.outputPath(`${name}.json`)
  fs.writeFileSync(file, JSON.stringify(value, null, 2))
  await info.attach(name, { path: file, contentType: 'application/json' })
}
async function history() {
  const response = await fetch(`${server!.baseURL}/api/sessions/history`)
  expect(response.status).toBe(200)
  return response.json()
}
async function register(sessionId: string, data: Record<string, unknown>) {
  const receipt = await post('/api/sessions/events', { sessionId, kind: 'register', data })
  expect(receipt.historySaved).toBe(true)
}
async function bot(agent: string, name: string) {
  const saved = await post('/api/bots', { name, chatName: name, agent, model: 'default', thinking: 'default', skill: null, prompt: '', extensions: [] })
  return saved.bots.find((item: { name: string }) => item.name === name).id as string
}
async function create(zone: string, botId: string) {
  const preview = await post('/api/chats/preview', { zone, botId })
  const accepted = await post('/api/chats', { zone, botId, chatId: preview.chatId, input: '/settings' })
  await expect.poll(async () => {
    const response = await fetch(`${server!.baseURL}/api/chats/${accepted.id}`)
    return (await response.json()).stage
  }).toBe('succeeded')
}
// P29 design: clients synchronize at one release barrier. Concurrent fields of
// the shared ID commute (name-only and type-only, identical cwd); two ordered
// waves settle later name/cwd changes without inventing an order within a race.
// Lost read/merge/write transactions drop an ID or one of the shared fields.
test('independent clients preserve exact concurrent IDs and commuting fields across a process restart', async ({ page }, info) => {
  server = await startServer()
  const clients = await Promise.all(Array.from({ length: 8 }, () => request.newContext()))
  try {
    const send = async (client: number, sessionId: string, data: Record<string, unknown>) => {
      const response = await clients[client].post(`${server!.baseURL}/api/sessions/events`, { data: { sessionId, kind: 'register', data } })
      expect(response.status()).toBe(200)
      expect((await response.json()).historySaved).toBe(true)
    }
    await send(0, 'shared-16b2b3f0-17a8-4ffd-8000-000000000001', { cwd: 'C:/wave-one' })
    const firstTime = (await history()).sessions[0].firstRecordedAt
    // Each client reaches this barrier before any write is released.
    const barrier = Promise.withResolvers<void>()
    const work: [string, Record<string, unknown>][] = [
      ['parallel-f58d5000-0000-4000-8000-000000000001', { cwd: 'C:/same', sessionName: 'Parallel identical', agentType: 'pi' }],
      ['parallel-f58d5000-0000-4000-8000-000000000002', { cwd: 'C:/same', sessionName: 'Parallel identical', agentType: 'pi' }],
      ['parallel-f58d5000-0000-4000-8000-000000000003', { cwd: 'C:/same', sessionName: 'Parallel identical', agentType: 'omp' }],
      ['parallel-f58d5000-0000-4000-8000-000000000004', { cwd: 'C:/same', sessionName: 'Parallel identical', agentType: 'omp' }],
      ['parallel-f58d5000-0000-4000-8000-000000000005', { cwd: 'C:/same', sessionName: 'Parallel identical', agentType: 'pi' }],
      ['parallel-f58d5000-0000-4000-8000-000000000006', { cwd: 'C:/same', sessionName: 'Parallel identical', agentType: 'omp' }],
      ['shared-16b2b3f0-17a8-4ffd-8000-000000000001', { cwd: 'C:/wave-one', sessionName: 'Shared first literal' }],
      ['shared-16b2b3f0-17a8-4ffd-8000-000000000001', { cwd: 'C:/wave-one', agentType: 'omp' }],
    ]
    const pending = work.map(async ([id, data], i) => { await barrier.promise; await send(i, id, data) })
    barrier.resolve()
    await Promise.all(pending)
    const first = await history()
    expect(first.total).toBe(7)
    expect(first.sessions.find((r: { sessionId: string }) => r.sessionId.startsWith('shared-'))).toEqual({ sessionId: 'shared-16b2b3f0-17a8-4ffd-8000-000000000001', cwd: 'C:/wave-one', sessionName: 'Shared first literal', agentType: 'omp', firstRecordedAt: firstTime })
    await Promise.all([
      send(6, 'shared-16b2b3f0-17a8-4ffd-8000-000000000001', { cwd: 'C:/wave-two', sessionName: 'Shared final literal' }),
      send(7, 'shared-16b2b3f0-17a8-4ffd-8000-000000000001', { cwd: 'C:/wave-two', agentType: 'omp' }),
    ])
    const saved = await history()
    const expected = [
      { sessionId: 'parallel-f58d5000-0000-4000-8000-000000000001', cwd: 'C:/same', sessionName: 'Parallel identical', agentType: 'pi', firstRecordedAt: expect.any(Number) },
      { sessionId: 'parallel-f58d5000-0000-4000-8000-000000000002', cwd: 'C:/same', sessionName: 'Parallel identical', agentType: 'pi', firstRecordedAt: expect.any(Number) },
      { sessionId: 'parallel-f58d5000-0000-4000-8000-000000000003', cwd: 'C:/same', sessionName: 'Parallel identical', agentType: 'omp', firstRecordedAt: expect.any(Number) },
      { sessionId: 'parallel-f58d5000-0000-4000-8000-000000000004', cwd: 'C:/same', sessionName: 'Parallel identical', agentType: 'omp', firstRecordedAt: expect.any(Number) },
      { sessionId: 'parallel-f58d5000-0000-4000-8000-000000000005', cwd: 'C:/same', sessionName: 'Parallel identical', agentType: 'pi', firstRecordedAt: expect.any(Number) },
      { sessionId: 'parallel-f58d5000-0000-4000-8000-000000000006', cwd: 'C:/same', sessionName: 'Parallel identical', agentType: 'omp', firstRecordedAt: expect.any(Number) },
      { sessionId: 'shared-16b2b3f0-17a8-4ffd-8000-000000000001', cwd: 'C:/wave-two', sessionName: 'Shared final literal', agentType: 'omp', firstRecordedAt: firstTime },
    ]
    expect([...saved.sessions].sort((a, b) => a.sessionId.localeCompare(b.sessionId))).toEqual(expected)
    await server.restart()
    expect(await history()).toEqual(saved)
    await displayedNames(page, ['Parallel identical', 'Parallel identical', 'Parallel identical', 'Parallel identical', 'Parallel identical', 'Parallel identical', 'Shared final literal'])
    await page.screenshot({ path: info.outputPath('concurrent-history-after-restart.png'), fullPage: true })
    await evidence(info, 'concurrent-durable-history', saved)
  } finally { await Promise.all(clients.map(client => client.dispose())) }
})

async function currentSnapshot() {
  const response = await fetch(`${server!.baseURL}/api/sessions/events`)
  expect(response.status).toBe(200)
  const reader = response.body!.getReader()
  const decoder = new TextDecoder()
  let text = ''
  try {
    while (!text.includes('\n\n')) {
      const chunk = await reader.read()
      expect(chunk.done).toBe(false)
      text += decoder.decode(chunk.value, { stream: true }).replace(/\r/g, '')
    }
    return JSON.parse(text.split('\n').find(line => line.startsWith('data:'))!.slice(5))
  } finally { await reader.cancel() }
}

// P26 combined zone-write/connect-write fault partition. A real staging-path
// denial leaves the public register accepted. A failed zone rename must neither
// replace durable metadata nor mutate current registration. Captured production
// diagnostics prove the independent zone attempt actually reached its failure.
for (const agent of ['pi', 'omp'] as const) {
  for (const order of ['connect-zone', 'zone-connect'] as const) {
    for (const intent of ['monitoring', 'countdown'] as const) {
    test(`${agent} ${order} ${intent}: real dual-source write failures preserve current state and durable zone name`, async ({ page }, info) => {
      const id = `${agent}-failure-89ba2000-8000-4000-8000-000000000001`
      const behavior = { native_session: { source: `herdr:${agent}`, agent, kind: 'id', value: id }, native_cwd: 'C:/native-before-failure' }
      server = await startServer({ safeShutdown: true, herdr: { behavior } })
      fs.appendFileSync(server.configPath, '\n[shutdown]\ncountdown_seconds = 3600\n')
      await server.restart()
      const zones = await post('/api/zones', { path: root })
      const zone = zones.zones[0].path as string
      const original = await bot(agent, 'Durable zone before failure')
      const rejected = await bot(agent, 'Zone name that never saved')
      await create(zone, original)
      await expect.poll(async () => (await history()).sessions[0]?.sessionName).toBe('Durable zone before failure')
      await register(id, { cwd: 'C:/current-original', agentType: agent, sessionName: 'Current original literal' })
      await post('/api/sessions/events', { sessionId: id, kind: 'state', data: { value: 'working' } })
      await post('/api/shutdown/monitor', { sessionId: id, selected: true })
      if (intent === 'countdown') await post('/api/shutdown/start', undefined)
      const before = await currentSnapshot()
      expect(before.shutdown.phase).toBe(intent === 'monitoring' ? 'none' : 'counting-down')
      expect(before.shutdown.monitoredSessionIds).toEqual(intent === 'monitoring' ? [id] : [])
      const saved = await history()
      const gate = path.join(root, 'failure-release')
      server.setHerdrBehavior({ ...behavior, native_session_gate: { wait_for_file: gate, timeout_ms: 40000 } })
      await create(zone, rejected)
      await expect.poll(() => server!.herdrCalls().filter(c => c.kind === 'agent_get').length).toBe(2)
      const staging = path.join(path.dirname(server.configPath), 'session-history.json.tmp')
      fs.mkdirSync(staging)
      const failedConnect = async () => {
        const receipt = await post('/api/sessions/events', { sessionId: id, kind: 'register', data: { cwd: 'C:/current-after-failure', agentType: agent, sessionName: 'Current accepted despite history failure' } })
        expect(receipt.outcome).toBe('registered')
        expect(receipt.historySaved).toBe(false)
        expect(receipt.historyWarning.code).toBe('history-save-failed')
      }
      if (order === 'connect-zone') await failedConnect()
      fs.writeFileSync(gate, 'ready')
      await expect.poll(() => server!.diagnostics()).toContain(`session=${id}`)
      const warning = server.diagnostics().split('\n').find(line => line.includes(`session=${id}`) && line.includes('save-failed'))!
      expect(warning).toContain('creation=')
      if (order === 'zone-connect') await failedConnect()
      const current = await currentSnapshot()
      expect(current.sessions).toHaveLength(1)
      expect(current.sessions[0]).toMatchObject({ sessionId: id, sessionName: 'Current accepted despite history failure', cwd: 'C:/current-after-failure', online: true, state: 'working' })
      for (const field of ['phase', 'monitoredSessionIds', 'arrangementId', 'deadline']) {
        expect(current.shutdown[field]).toEqual(before.shutdown[field])
      }
      expect(await history()).toEqual(saved)
      fs.rmdirSync(staging)
      await register(id, { cwd: 'C:/recovered-current', agentType: agent, sessionName: 'Connect recovery must not replace saved zone' })
      const recovered = await history()
      expect(recovered.sessions).toEqual([{ sessionId: id, sessionName: 'Durable zone before failure', cwd: 'C:/recovered-current', agentType: agent, originalWorkspaceId: 'w-target', firstRecordedAt: saved.sessions[0].firstRecordedAt }])
      await evidence(info, 'actual-zone-write-warning', { warning })
      await evidence(info, 'combined-failure-history', recovered)
      const afterRecovery = await currentSnapshot()
      for (const field of ['phase', 'monitoredSessionIds', 'arrangementId', 'deadline']) {
        expect(afterRecovery.shutdown[field]).toEqual(before.shutdown[field])
      }
      await server.restart()
      expect(await history()).toEqual(recovered)
      const restarted = await currentSnapshot()
      expect(restarted.sessions).toEqual([])
      expect(restarted.shutdown.phase).toBe('none')
      expect(restarted.shutdown.monitoredSessionIds).toEqual([])
      await evidence(info, 'combined-current-and-shutdown-observations', { before, afterFailures: current, afterRecovery, afterRestart: restarted })
      // A live countdown intentionally owns a modal. Do not cancel it to reach
      // history: restart it while still active, prove intent is gone, then read.
      await displayedNames(page, ['Durable zone before failure'])
      await page.screenshot({ path: info.outputPath('combined-failure-final-name.png'), fullPage: true })
    })
    }
  }
}

async function displayedNames(page: Page, names: string[]) {
  await page.goto(server!.baseURL)
  await page.getByRole('button', { name: '查看历史会话' }).click()
  const rows = page.locator('.history-table tbody tr td:first-child')
  await expect(rows).toHaveCount(names.length)
  expect((await rows.allTextContents()).sort()).toEqual([...names].sort())
}

// P04/P09 design: full IDs and names below are independent literals. Last-arrival
// wins would replace the zone name on the final connect. Prefix/name/cwd grouping
// would lose the two decoys. A subsequent zone name and empty connect metadata
// distinguish persistent priority from a one-off arrival-order coincidence.
for (const agent of ['pi', 'omp'] as const) {
  for (const order of ['connect-zone', 'zone-connect', 'simultaneous'] as const) {
    test(`${agent} ${order}: full identity merges once and renamed zone name wins in browser and after restart`, async ({ page }, info) => {
      const id = `${agent}-a4878910-1be8-4d12-bccc-000000000001`
      const other = `${agent}-a4878910-1be8-4d12-bccc-000000000002`
      const crossAgent = `other-a4878910-1be8-4d12-bccc-000000000003`
      const gate = path.join(root, 'identity-ready')
      const behavior = { native_session: { source: `herdr:${agent}`, agent, kind: 'id', value: id }, native_cwd: 'C:/same-project' }
      server = await startServer({ herdr: { behavior: { ...behavior, native_session_gate: { wait_for_file: gate, timeout_ms: 40000 } } } })
      const zones = await post('/api/zones', { path: root })
      const zone = zones.zones[0].path as string
      const first = await bot(agent, 'Zone original literal')
      const renamed = await bot(agent, 'Zone renamed literal')
      const connect = () => register(id, { cwd: 'C:/same-project', sessionName: 'Connect original literal', agentType: agent })
      if (order === 'connect-zone') await connect()
      await create(zone, first)
      await expect.poll(() => server!.herdrCalls().filter(c => c.kind === 'agent_get').length).toBe(1)
      if (order === 'simultaneous') {
        // Two independent clients are in flight across a released external gate.
        // Neither arrival order is prescribed; zone priority is order-independent.
        const arrival = connect()
        fs.writeFileSync(gate, 'ready')
        await arrival
      } else {
        fs.writeFileSync(gate, 'ready')
      }
      await expect.poll(async () => (await history()).sessions[0]?.sessionName).toBe('Zone original literal')
      if (order === 'zone-connect') await connect()
      await register(id, { cwd: 'C:/same-project', sessionName: 'Connect later literal', agentType: agent })
      const firstRecordedAt = (await history()).sessions[0].firstRecordedAt
      expect(firstRecordedAt).toEqual(expect.any(Number))
      await register(other, { cwd: 'C:/same-project', sessionName: 'Connect-only initial literal', agentType: agent })
      await register(crossAgent, { cwd: 'C:/same-project', sessionName: 'Connect latest literal', agentType: agent === 'pi' ? 'omp' : 'pi' })
      await register(other, { cwd: 'C:/same-project', sessionName: 'Connect latest literal', agentType: agent })
      server.setHerdrBehavior(behavior)
      await create(zone, renamed)
      await expect.poll(async () => (await history()).sessions.find((r: { sessionId: string }) => r.sessionId === id)?.sessionName).toBe('Zone renamed literal')
      await register(id, { cwd: 'C:/same-project', sessionName: 'Connect must never win', agentType: agent })
      await register(id, { cwd: 'C:/same-project' })
      await register(other, { cwd: 'C:/same-project', sessionName: '   ', agentType: '' })
      const saved = await history()
      expect(saved.total).toBe(3)
      expect([...saved.sessions].sort((a, b) => a.sessionId.localeCompare(b.sessionId))).toEqual([
        { sessionId: id, sessionName: 'Zone renamed literal', agentType: agent, cwd: 'C:/same-project', originalWorkspaceId: 'w-target', firstRecordedAt },
        { sessionId: other, sessionName: 'Connect latest literal', agentType: agent, cwd: 'C:/same-project', firstRecordedAt: expect.any(Number) },
        { sessionId: crossAgent, sessionName: 'Connect latest literal', agentType: agent === 'pi' ? 'omp' : 'pi', cwd: 'C:/same-project', firstRecordedAt: expect.any(Number) },
      ].sort((a, b) => a.sessionId.localeCompare(b.sessionId)))
      await displayedNames(page, ['Zone renamed literal', 'Connect latest literal', 'Connect latest literal'])
      await page.screenshot({ path: info.outputPath('cross-source-final-names.png'), fullPage: true })
      await server.restart()
      expect(await history()).toEqual(saved)
      await displayedNames(page, ['Zone renamed literal', 'Connect latest literal', 'Connect latest literal'])
      await evidence(info, 'durable-history', saved)
    })
  }
}
