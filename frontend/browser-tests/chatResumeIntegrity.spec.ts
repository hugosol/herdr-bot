import fs from 'node:fs'
import path from 'node:path'
import { expect, test, type Page, type TestInfo, type Locator } from '@playwright/test'
import { startServer, type E2EServer } from './support'

let server: E2EServer | null = null
test.afterEach(async () => { await server?.cleanup(); server = null })
type Policy = 'filtered' | 'discovery'
type Scenario = {
  agent: string; policy: Policy; root: string; cwd: string; fixed: string
  optional: string; id: string; native: string; historyFile: string; name: string
  identity: { native_session: { agent: string; source: string; kind: string; value: string }; native_cwd: string }
}

async function post(endpoint: string, body: unknown) {
  return fetch(`${server!.baseURL}${endpoint}`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify(body) })
}
async function accepted(endpoint: string, body: unknown) {
  const response = await post(endpoint, body)
  expect(response.ok, await response.clone().text()).toBe(true)
  return response.json()
}

// Approved seams: S1 delivered page, S2 real HTTP/files/restart, S3 production
// CLI adapter with only external Herdr controlled. Never execute a fixture .ts.
async function prepare(agent: string, policy: Policy) {
  server = await startServer({ herdr: {} })
  const root = path.dirname(server.configPath)
  const cwd = path.join(root, 'isolated integrity cwd')
  fs.mkdirSync(cwd)
  const fixed = path.join(cwd, 'identity-fixed.ts')
  const optional = path.join(cwd, 'optional.ts')
  fs.writeFileSync(fixed, "throw new Error('do not import fixture')\n")
  fs.writeFileSync(optional, "throw new Error('do not import fixture')\n")
  fs.writeFileSync(server.metadataFile, `skills = []\n[[agents]]\nid = '${agent}'\nlabel = '${agent}'\n[[models]]\nid = 'default'\nlabel = 'Default'\n[[extensions]]\nid = 'identity-fixed'\nlabel = 'Identity fixed'\nagent = '${agent}'\nkind = 'fixed'\nentrypoint = '${fixed}'\n[[extensions]]\nid = 'optional'\nlabel = 'Optional marker'\nagent = '${agent}'\nkind = 'optional'\nentrypoint = '${optional}'\n`)
  const id = `${agent}-integrity-history`
  const native = path.join(cwd, 'original.jsonl')
  fs.writeFileSync(native, `${JSON.stringify({ type: 'session', version: 3, id, cwd })}\n`)
  const identity = { native_session: { agent, source: `herdr:${agent}`, kind: 'id', value: id }, native_cwd: cwd }
  const historyFile = path.join(root, 'session-history.json')
  const name = `Integrity ${agent}`
  if (policy === 'filtered') {
    server.setHerdrBehavior({ native_session: { agent, source: `herdr:${agent}`, kind: 'path', value: native } })
    await accepted('/api/zones', { path: cwd })
    const saved = await accepted('/api/tasks', { name, chatName: name, agent, model: 'default', thinking: 'default', prompt: 'INITIAL ONLY', extensions: ['optional'] })
    const taskId = saved.tasks[0].id
    const preview = await accepted('/api/chats/preview', { zone: cwd, taskId: taskId })
    const job = await accepted('/api/chats', { zone: cwd, taskId: taskId, chatId: preview.chatId, input: 'SEED ONLY' })
    await expect.poll(async () => (await (await fetch(`${server!.baseURL}/api/chats/${job.id}`)).json()).stage).toBe('succeeded')
    await expect.poll(async () => (await (await fetch(`${server!.baseURL}/api/sessions/history`)).json()).total).toBe(1)
    fs.unlinkSync(optional)
  } else {
    // Independently prewritten legacy history; no source is invented by reads.
    fs.writeFileSync(historyFile, JSON.stringify([{ sessionId: id, sessionName: name, agentType: agent, cwd, sessionFile: native, originalWorkspaceId: 'w-target', firstRecordedAt: 123 }]))
  }
  await server.restart()
  server.setHerdrBehavior(identity)
  return { agent, policy, root, cwd, fixed, optional, id, native, identity, historyFile, name }
}
async function openHistory(page: Page, scenario: Scenario) {
  await page.goto(server!.baseURL)
  await page.getByRole('button', { name: '查看历史会话' }).click()
  return page.getByRole('row').filter({ hasText: scenario.name })
}
async function clickResume(page: Page, row: Locator) {
  const response = page.waitForResponse(r => r.url().endsWith('/api/chats/resume') && r.request().method() === 'POST')
  await row.getByRole('button', { name: 'Resume', exact: true }).click()
  return response
}
async function saveEvidence(page: Page, info: TestInfo, label: string, evidence: unknown) {
  await page.screenshot({ path: info.outputPath(`${label}.png`), fullPage: true })
  const file = info.outputPath(`${label}.json`)
  fs.writeFileSync(file, JSON.stringify({ boundary: 'Controlled external Herdr; not real agent extension loading', evidence }, null, 2))
  await info.attach(label, { path: file, contentType: 'application/json' })
}

// P15 combined slice. Independent contract outcome: all three actual choices
// survive to confirmed success. Plausible violation: last fallback overwrites
// earlier reasons, or filtered mode silently becomes native discovery.
for (const agent of ['pi', 'omp']) {
  for (const policy of ['filtered', 'discovery'] as const) {
    test(`${agent} ${policy} combines exact ID and deleted Workspace without losing a degradation`, async ({ page }, info) => {
      const s = await prepare(agent, policy)
      fs.unlinkSync(s.native)
      fs.writeFileSync(path.join(s.root, 'herdr', 'workspaces.json'), JSON.stringify([{ workspace_id: 'w-fallback', label: 'bot-playground', focused: true }]))
      const historyBefore = fs.readFileSync(s.historyFile)
      const row = await openHistory(page, s)
      const offset = server!.herdrCalls().length
      const response = await clickResume(page, row)
      const result = await response.json()
      expect(response.status()).toBe(200)
      expect(result.outcome).toBe('restored')
      expect(result.degradations.map((d: { element: string }) => d.element)).toEqual(['session-file', policy === 'filtered' ? 'extension' : 'extensions', 'workspace'])
      const status = row.getByRole('status')
      for (const text of ['恢复存在降级', s.native, `精确原生 ID ${s.id}`, '原 Workspace 已删除', 'w-target', '唯一默认 Workspace', 'w-fallback', '已确认恢复同一']) await expect(status).toContainText(text)
      for (const item of result.degradations) {
        await expect(status).toContainText(item.reason)
        await expect(status).toContainText(item.strategy)
      }
      if (policy === 'filtered') {
        for (const text of ['Optional marker', 'optional', '入口不存在', '跳过', '仍关闭扩展自动发现']) await expect(status).toContainText(text)
        await expect(status).not.toContainText('原生默认扩展发现')
      } else {
        for (const text of ['缺少来源 Task', '原生默认扩展发现', '不显式追加 Task']) await expect(status).toContainText(text)
        await expect(status).not.toContainText('跳过此扩展')
      }
      expect(result.target.workspaceId).toBe('w-fallback')
      const calls = server!.herdrCalls().slice(offset)
      expect(calls.filter(c => c.kind === 'agent_start')).toHaveLength(1)
      expect(calls.find(c => c.kind === 'agent_start')!.args).toEqual([...(policy === 'filtered' ? ['--no-extensions', '--extension', s.fixed] : []), agent === 'pi' ? '--session' : '--resume', s.id])
      // History rendering may independently resolve Workspace labels.
      expect(calls.filter(c => c.kind !== 'workspace_list').map(c => c.kind)).toEqual(['agent_list', 'tab_create', 'agent_start', 'agent_get', 'agent_focus'])
      expect(fs.readFileSync(s.historyFile)).toEqual(historyBefore)
      expect(fs.existsSync(s.native)).toBe(false)
      await saveEvidence(page, info, `${agent}-${policy}-combined`, { result, calls, historyUnchanged: true, nativeStillAbsent: true })
    })
  }
}

// P16: independently specified refusal, wrong identity, absent identity and
// failed focus must remain failed/unconfirmed, never generic degraded success.
// Missing optional file is real; only Herdr supplies each external condition.
for (const agent of ['pi', 'omp']) {
  for (const fault of ['start', 'mismatch', 'confirm', 'focus']) {
    test(`${agent} filtered ${fault} keeps authoritative result and prior degradation on HTTP and page`, { tag: fault === 'confirm' ? '@time-contract' : [] }, async ({ page }, info) => {
      const s = await prepare(agent, 'filtered')
      server!.setHerdrBehavior({ ...s.identity,
        ...(fault === 'start' ? { fail_agent_start: true } : {}),
        ...(fault === 'mismatch' ? { native_session: { agent, source: `herdr:${agent}`, kind: 'id', value: 'different-native-chat' } } : {}),
        ...(fault === 'confirm' ? { native_session: null } : {}),
        ...(fault === 'focus' ? { fail_agent_focus: true } : {}),
      })
      const historyBefore = fs.readFileSync(s.historyFile)
      const nativeBefore = fs.readFileSync(s.native)
      const row = await openHistory(page, s)
      const offset = server!.herdrCalls().length
      const response = await clickResume(page, row)
      const result = await response.json()
      expect(response.status()).toBe(502)
      expect(result.outcome).toBe(fault === 'confirm' ? 'unconfirmed' : 'failed')
      expect(result.stage).toBe(fault === 'mismatch' ? 'confirm' : fault)
      expect(result.code).toBe(fault === 'confirm' ? 'restore-unconfirmed' : fault === 'mismatch' ? 'native-session-mismatch' : 'herdr-command-failed')
      expect(result.degradations).toHaveLength(1)
      expect(result.degradations[0].element).toBe('extension')
      const alert = row.getByRole('alert')
      for (const text of ['恢复存在降级', 'Optional marker', '入口不存在', '跳过', '仍关闭扩展自动发现', fault === 'confirm' ? '启动结果未确认' : '尚未确认恢复成功']) await expect(alert).toContainText(text)
      await expect(row.getByRole('status')).toHaveCount(0)
      await expect(alert).not.toContainText('已确认恢复同一')
      const calls = server!.herdrCalls().slice(offset)
      expect(calls.filter(c => c.kind === 'tab_create')).toHaveLength(1)
      expect(calls.filter(c => c.kind === 'agent_start')).toHaveLength(1)
      expect(calls.filter(c => c.kind === 'agent_focus')).toHaveLength(fault === 'focus' ? 1 : 0)
      expect(calls.every(c => ['workspace_list', 'agent_list', 'tab_create', 'agent_start', 'agent_get', 'agent_focus'].includes(String(c.kind)))).toBe(true)
      expect(result.target).toEqual({ workspaceId: 'w-target', tabId: server!.herdrTabs().at(-1)!.tab_id, paneId: server!.herdrTabs().at(-1)!.pane_id })
      if (fault === 'confirm') {
        expect(result.progress).toEqual({ tabCreated: true, startSubmitted: true, conversationConfirmed: false, focused: false })
        expect(calls.some(c => c.kind === 'agent_get')).toBe(true)
      }
      expect(fs.readFileSync(s.historyFile)).toEqual(historyBefore)
      expect(fs.readFileSync(s.native)).toEqual(nativeBefore)
      await saveEvidence(page, info, `${agent}-filtered-${fault}`, { result, calls, retainedTabs: server!.herdrTabs(), historyUnchanged: true, nativeUnchanged: true })
    })
  }
}

// P16: the contract permits reporting selected strategies, not future ones.
// A real native mismatch stops before environment selection; failed original
// lookup or missing default stops before a Workspace replacement is selected.
for (const agent of ['pi', 'omp']) {
  for (const stop of ['session', 'workspace-query', 'missing-default']) {
    test(`${agent} ${stop} does not invent unreached fallback strategies`, async ({ page }, info) => {
      const s = await prepare(agent, 'filtered')
      if (stop === 'session') {
        fs.writeFileSync(s.native, `${JSON.stringify({ type: 'session', version: 3, id: 'another-native-chat', cwd: s.cwd })}\n`)
      } else {
        fs.unlinkSync(s.native)
        if (stop === 'workspace-query') server!.setHerdrBehavior({ ...s.identity, fail_workspace_list: true })
        else fs.writeFileSync(path.join(s.root, 'herdr', 'workspaces.json'), JSON.stringify([{ workspace_id: 'w-other', label: 'Not the configured default' }]))
      }
      const row = await openHistory(page, s)
      const offset = server!.herdrCalls().length
      const response = await clickResume(page, row)
      const result = await response.json()
      expect(response.status()).toBe(502)
      expect(result.outcome).toBe('failed')
      expect(result.stage).toBe(stop === 'session' ? 'session' : 'workspace')
      expect(result.code).toBe(stop === 'session' ? 'native-session-mismatch' : stop === 'workspace-query' ? 'herdr-command-failed' : 'workspace-not-found')
      expect(result.degradations.map((d: { element: string }) => d.element)).toEqual(stop === 'session' ? [] : ['session-file', 'extension'])
      expect(result.fallback).toBe(false)
      const alert = row.getByRole('alert')
      await expect(alert).toContainText('尚未确认恢复成功')
      for (const text of ['已回退到', '原生默认扩展发现', '已确认恢复同一']) await expect(alert).not.toContainText(text)
      if (stop === 'session') {
        await expect(alert).not.toContainText('跳过此扩展')
        await expect(alert).not.toContainText('精确原生 ID')
      } else {
        for (const text of ['精确原生 ID', 'Optional marker', '入口不存在', '跳过']) await expect(alert).toContainText(text)
      }
      const calls = server!.herdrCalls().slice(offset)
      expect(calls.filter(c => c.kind === 'agent_list')).toHaveLength(1)
      expect(calls.every(c => ['agent_list', 'workspace_list'].includes(String(c.kind)))).toBe(true)
      await saveEvidence(page, info, `${agent}-${stop}-unreached`, { result, calls, noMutatingCalls: true })
    })
  }
}

// P20 on both new policies. A file gate establishes actual overlap at external
// start, not an arbitrary sleep. Independent duplicate HTTP must return while
// owner is blocked. Structured CLI timeout is uncertain despite process exit;
// later lookup/identity failures cannot turn that retained target into no match.
for (const agent of ['pi', 'omp']) {
  for (const policy of ['filtered', 'discovery'] as const) {
    test(`${agent} ${policy} rejects overlapping requests and retains structured-timeout target for retry lookup`, async ({ page }, info) => {
      const s = await prepare(agent, policy)
      const release = path.join(s.root, 'release-start')
      server!.setHerdrBehavior({ ...s.identity, timeout_agent_start: true, agent_start_gate: { wait_for_file: release, timeout_ms: 30000 } })
      const row = await openHistory(page, s)
      const offset = server!.herdrCalls().length
      const pending = clickResume(page, row)
      await expect.poll(() => server!.herdrCalls().slice(offset).filter(c => c.kind === 'agent_start').length).toBe(1)
      await expect(row.getByRole('button', { name: '正在定位／恢复…' })).toBeDisabled()
      await expect(row.getByRole('status')).toHaveCount(0)
      const duplicate = await fetch(`${server!.baseURL}/api/chats/resume`, {
        method: 'POST', headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ sessionId: s.id }), signal: AbortSignal.timeout(3000),
      })
      expect(duplicate.status).toBe(409)
      expect((await duplicate.json()).code).toBe('resume-in-progress')
      // The owner is still externally blocked when the duplicate has completed.
      expect(fs.existsSync(release)).toBe(false)
      expect(server!.herdrCalls().slice(offset).filter(c => c.kind === 'agent_list')).toHaveLength(1)
      fs.writeFileSync(release, 'release')
      const response = await pending
      const result = await response.json()
      expect(response.status()).toBe(502)
      expect(result.outcome).toBe('unconfirmed')
      expect(result.stage).toBe('start')
      expect(result.code).toBe('restore-unconfirmed')
      expect(result.progress).toEqual({ tabCreated: true, startSubmitted: true, conversationConfirmed: false, focused: false })
      expect(result.degradations.map((d: { element: string }) => d.element)).toEqual([policy === 'filtered' ? 'extension' : 'extensions'])
      const alert = row.getByRole('alert')
      for (const text of ['恢复存在降级', '启动结果未确认', '不能据此判断未启动', '不自动关闭', policy === 'filtered' ? 'Optional marker' : '原生默认扩展发现']) await expect(alert).toContainText(text)
      await expect(alert).not.toContainText('已确认恢复同一')
      const startedFile = path.join(s.root, 'herdr', 'started.jsonl')
      const started = JSON.parse(fs.readFileSync(startedFile, 'utf8').trim().split('\n').at(-1)!)
      expect(result.target).toEqual({ workspaceId: started.workspace_id, tabId: started.tab_id, paneId: started.pane_id })
      expect(result.agentName).toBe(started.name)
      for (const value of [started.workspace_id, started.tab_id, started.pane_id, started.name]) await expect(row.getByLabel('Resume 目标').getByText(value, { exact: true })).toBeVisible()
      await saveEvidence(page, info, `${agent}-${policy}-structured-timeout`, { result, calls: server!.herdrCalls().slice(offset), retainedTarget: started, duplicateStatus: duplicate.status })

      const retryFailures = []
      for (const fault of ['query', 'identity']) {
        // Bad candidate identities are skippable. A matched identity with an
        // invalid navigation target must still fail lookup without starting again.
        server!.setHerdrBehavior(fault === 'query' ? { fail_agent_list: true } : {
          agents: [{ ...started, agent_session: s.identity.native_session, pane_id: '' }],
        })
        const retry = await clickResume(page, row)
        const failure = await retry.json()
        expect(retry.status()).toBe(502)
        expect(failure.outcome).toBe('failed')
        expect(failure.stage).toBe('lookup')
        expect(failure.code).toBe(fault === 'query' ? 'herdr-command-failed' : 'identity-verification-failed')
        await expect(row.getByRole('alert')).toContainText('Herdr 查找／身份核实')
        await expect(row.getByRole('status')).toHaveCount(0)
        retryFailures.push(failure)
      }
      // P11: an existing native target does not reload even an edited source
      // environment, nor require a currently readable launch catalog.
      if (policy === 'filtered') {
        const { id: taskId, ...task } = (await server!.getTasks()).tasks[0]
        const edited = await fetch(`${server!.baseURL}/api/tasks/${taskId}`, { method: 'PUT', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ ...task, extensions: [], prompt: 'EDITED INITIAL NEVER REPLAY' }) })
        expect(edited.status).toBe(200)
      }
      fs.writeFileSync(server!.metadataFile, 'independently broken catalog !!!')
      server!.setHerdrBehavior({ agents: [{ ...started, agent_session: s.identity.native_session }] })
      const retry = await clickResume(page, row)
      const located = await retry.json()
      expect(retry.status()).toBe(200)
      expect(located.outcome).toBe('located')
      expect(located.target).toEqual(result.target)
      await expect(row.getByRole('status')).toHaveText('已切换到当前herdr焦点')
      const calls = server!.herdrCalls().slice(offset)
      expect(calls.filter(c => c.kind === 'agent_list')).toHaveLength(4)
      expect(calls.filter(c => c.kind === 'tab_create')).toHaveLength(1)
      expect(calls.filter(c => c.kind === 'agent_start')).toHaveLength(1)
      expect(calls.filter(c => c.kind === 'agent_focus')).toHaveLength(1)
      expect(calls.find(c => c.kind === 'agent_start')!.args).toEqual([...(policy === 'filtered' ? ['--no-extensions', '--extension', s.fixed] : []), agent === 'pi' ? '--session' : '--resume', s.native])
      expect(calls.every(c => ['workspace_list', 'agent_list', 'tab_create', 'agent_start', 'agent_focus'].includes(String(c.kind)))).toBe(true)
      expect(server!.herdrTabs().at(-1)!.pane_id).toBe(started.pane_id)
      await saveEvidence(page, info, `${agent}-${policy}-retry-located`, { located, retryFailures, calls, retainedTarget: started })
    })
  }
}
