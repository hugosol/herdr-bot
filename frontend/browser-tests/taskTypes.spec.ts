import fs from 'node:fs'
import path from 'node:path'
import { spawnSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'
import { expect, test } from '@playwright/test'
import { startServer, type E2EServer } from './support'

let server: E2EServer | null = null
test.afterEach(async () => { await server?.cleanup(); server = null })

// P5.01 / S1,S2: changing Task purpose preserves its host and selects only
// the corresponding non-cancellable role extension.
test('P5.01 Pi Task types preserve agent and non-cancellable role extensions', async ({ page }, info) => {
  server = await startServer()
  await page.goto(server.baseURL)
  await page.getByTestId('tab-tasks').click()
  await page.getByTestId('task-create-open').click()
  await expect(page.getByTestId('task-type')).toHaveValue('normal')
  await page.getByTestId('task-name').fill('类型切换')
  await page.getByTestId('task-agent').selectOption('pi')
  await page.getByTestId('task-type').selectOption('manager')
  await expect(page.getByTestId('task-agent')).toHaveValue('pi')
  await expect(page.getByTestId('task-agent').locator('option[value=pi]')).toBeEnabled()
  await page.getByTestId('task-next').click()
  await expect(page.getByTestId('task-fixed-extensions')).toContainText('manager-chat')
  await expect(page.getByTestId('task-fixed-extensions')).not.toContainText('node-chat')
  await expect(page.getByTestId('extension-pi-manager-chat')).toHaveCount(0)
  await expect(page.getByTestId('extension-pi-node-chat')).toHaveCount(0)
  await page.getByTestId('task-next').click()
  await page.getByTestId('save-task').click()
  await expect(page.getByTestId('task-dialog')).toBeHidden()
  let saved = (await server.getTasks()).tasks[0]
  expect(saved).toMatchObject({ taskType: 'manager', agent: 'pi', extensions: [] })
  await expect(page.getByTestId('task-item')).toContainText('manager-chat')
  await expect(page.getByTestId('task-item')).not.toContainText('node-chat')
  for (const role of ['node', 'normal']) {
    await page.getByTestId('task-edit').click()
    await page.getByTestId('task-type').selectOption(role)
    await expect(page.getByTestId('task-agent')).toHaveValue('pi')
    await expect(page.getByTestId('task-agent').locator('option[value=pi]')).toBeEnabled()
    await page.getByTestId('task-next').click()
    const fixed = page.getByTestId('task-fixed-extensions')
    await expect(fixed).not.toContainText('manager-chat')
    if (role === 'node') await expect(fixed).toContainText('node-chat')
    else await expect(fixed).not.toContainText('node-chat')
    await page.getByTestId('task-next').click()
    await page.getByTestId('save-task').click()
    await expect(page.getByTestId('task-dialog')).toBeHidden()
    saved = (await server.getTasks()).tasks[0]
    expect(saved).toMatchObject({ taskType: role, agent: 'pi', extensions: [] })
  }
  await page.screenshot({ path: info.outputPath('task-types.png'), fullPage: true })
  fs.writeFileSync(info.outputPath('task-types.json'), JSON.stringify(await server.getTasks(), null, 2))
})

// P5.01: observe effective Task environment at the existing HTTP launch boundary.
// Controlled external calls prove assembly only, not role tools/Prompt execution.
test('P5.01 role changes select exactly one fixed role environment', async ({}, info) => {
  server = await startServer({ herdr: {} })
  const root = path.dirname(server.configPath)
  const zonePath = path.join(root, 'role-zone')
  fs.mkdirSync(zonePath)
  const zoneResponse = await fetch(`${server.baseURL}/api/zones`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ path: zonePath }) })
  const zone = (await zoneResponse.json()).zones[0].path
  let id: string | undefined
  for (const role of ['manager', 'node', 'normal']) {
    const response = await fetch(`${server.baseURL}/api/tasks${id ? `/${id}` : ''}`, { method: id ? 'PUT' : 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ name: '角色', chatName: '{chat.id}', taskType: role, agent: 'omp', model: 'default', thinking: 'default', skill: null, prompt: '', extensions: [] }) })
    expect(response.status).toBe(200)
    id = (await response.json()).tasks[0].id
    const preview = await (await fetch(`${server.baseURL}/api/chats/preview`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ taskId: id, zone }) })).json()
    const created = await fetch(`${server.baseURL}/api/chats`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ taskId: id, zone, chatId: preview.chatId, input: '检查角色环境装配' }) })
    expect(created.status).toBe(202)
    const job = await created.json()
    await expect.poll(async () => (await (await fetch(`${server!.baseURL}/api/chats/${job.id}`)).json()).stage).toBe('succeeded')
    const entries = role === 'manager' ? ['omp-herdr-agent-state', 'omp-model-cost', 'omp-manager-chat']
      : role === 'node' ? ['omp-herdr-agent-state', 'omp-model-cost', 'omp-node-chat'] : ['omp-herdr-agent-state', 'omp-model-cost']
    expect(server.herdrCalls().filter(call => call.kind === 'agent_start').at(-1)!.args).toEqual(['--no-extensions', ...entries.flatMap(entry => ['--extension', path.join(root, 'extensions', `${entry}.ts`)])])
  }
  fs.writeFileSync(info.outputPath('role-environments-NOT-native-loading.json'), JSON.stringify({ calls: server.herdrCalls(), tasks: await server.getTasks() }, null, 2))
})

// P6.01 / S4,S2,S3: independent old literals. Losing any trigger/env field,
// guessing a role from name/prompt, or changing a Chat's source must disagree.
// Real shipped backend/CLI migration/restart/Resume; external Herdr is controlled,
// so this proves the Resume boundary, NOT native host or extension execution.
test('P6.01 explicit old Task migration preserves fields and Chat source through restart and Resume @time-contract', async ({ page }, info) => {
  server = await startServer({ herdr: {} })
  const root = path.dirname(server.configPath)
  const old = [
    { id: 'stable-manager-id', name: 'manager', chatName: '{task.name} · {zone.name}', agent: 'omp', model: 'gpt-6.1-sol', thinking: 'max', skill: 'code-review', prompt: 'manager\nDO NOT REPLAY', extensions: ['omp-codebase-tools', 'omp-connect-bot'] },
    { id: 'stable-node-id', name: 'node', chatName: '原名 {chat.id}', agent: 'pi', model: 'deepseek-flash', thinking: 'low', skill: 'teach', prompt: 'node\nDO NOT REPLAY', extensions: ['pi-codebase-tools'] },
  ]
  const history = old.map((task, index) => {
    const native = path.join(root, `${task.agent}-old.jsonl`)
    fs.writeFileSync(native, JSON.stringify({ type: 'session', version: 3, id: `native-${task.agent}`, cwd: root }) + '\n')
    return { sessionId: `native-${task.agent}`, sessionName: `Migrated ${task.agent}`, agentType: task.agent, cwd: root, sessionFile: native, firstRecordedAt: 123 + index, originalWorkspaceId: 'w-target', sourceTaskId: task.id, memo: '关联与备忘不变' }
  })
  const historyFile = path.join(root, 'session-history.json')
  const historyBytes = JSON.stringify(history, null, 2) + '\n'
  const oldBytes = JSON.stringify({ tasks: old }, null, 2) + '\n'
  fs.writeFileSync(historyFile, historyBytes)
  fs.writeFileSync(server.storageFile, oldBytes)
  const incompatible = await fetch(`${server.baseURL}/api/tasks`)
  expect(incompatible.status).toBe(500)
  expect((await incompatible.json()).code).toBe('tasks-corrupt')
  expect(fs.readFileSync(server.storageFile, 'utf8')).toBe(oldBytes)
  await server.stop()
  const script = fileURLToPath(new URL('../../scripts/migrate-task-types.py', import.meta.url))
  const migrationArgs = [script, '--root', server.cwd, '--config', server.configPath]
  const result = spawnSync('python', migrationArgs, { encoding: 'utf8', windowsHide: true })
  expect(result.status, result.stderr).toBe(0)
  const expected = [{ ...old[0], taskType: 'normal' }, { ...old[1], taskType: 'normal' }]
  expect(JSON.parse(fs.readFileSync(server.storageFile, 'utf8'))).toEqual({ tasks: expected })
  expect(fs.readFileSync(`${server.storageFile}.pre-task-types`, 'utf8')).toBe(oldBytes)
  expect(fs.readFileSync(historyFile, 'utf8')).toBe(historyBytes)
  const migratedBytes = fs.readFileSync(server.storageFile, 'utf8')
  expect(spawnSync('python', migrationArgs, { encoding: 'utf8', windowsHide: true }).status).toBe(0)
  expect(fs.readFileSync(server.storageFile, 'utf8')).toBe(migratedBytes)
  await server.restart()
  expect((await server.getTasks()).tasks).toEqual(expected)
  await page.goto(server.baseURL)
  await page.getByRole('button', { name: '查看历史会话' }).click()
  const outcomes = []
  for (const item of history) {
    server.setHerdrBehavior({ native_session: { agent: item.agentType, source: `herdr:${item.agentType}`, kind: 'path', value: item.sessionFile } })
    const row = page.getByRole('row').filter({ hasText: item.sessionName })
    const response = page.waitForResponse(r => r.url().endsWith('/api/chats/resume') && r.request().method() === 'POST')
    await row.getByRole('button', { name: 'Resume', exact: true }).click()
    const reply = await response
    const body = await reply.json()
    expect(reply.status()).toBe(200)
    expect(body.outcome).toBe('restored')
    outcomes.push(body)
    const start = server.herdrCalls().filter(call => call.kind === 'agent_start').at(-1)!
    const entries = item.agentType === 'omp'
      ? ['omp-herdr-agent-state', 'omp-model-cost', 'omp-codebase-tools', 'omp-connect-bot']
      : ['pi-herdr-agent-state', 'pi-model-cost', 'pi-codebase-tools']
    expect(start.args).toEqual(['--no-extensions', ...entries.flatMap(id => ['--extension', path.join(root, 'extensions', `${id}.ts`)]), item.agentType === 'omp' ? '--resume' : '--session', item.sessionFile])
    expect(fs.readFileSync(historyFile, 'utf8')).toBe(historyBytes)
  }
  // Observe a bounded 300 ms after both completed Resume requests; do not claim
  // that finite absence proves no future external behavior.
  await page.waitForTimeout(300)
  expect(server.herdrCalls().some(call => ['agent_prompt', 'agent_input'].includes(String(call.kind)))).toBe(false)
  const evidence = { scope: 'Actual CLI migration and backend restart; controlled external Herdr, not native execution', old, expected, history, outcomes, noReplayObservationMs: 300, calls: server.herdrCalls(), migration: result.stdout }
  fs.writeFileSync(info.outputPath('task-migration-and-resume.json'), JSON.stringify(evidence, null, 2))
  await page.screenshot({ path: info.outputPath('migrated-chat-resume.png'), fullPage: true })
})
