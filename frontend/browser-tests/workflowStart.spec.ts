import fs from 'node:fs'
import path from 'node:path'
import { test, expect } from '@playwright/test'
import { startServer, type E2EServer } from './support'
import { expectNoHorizontalOverflow } from './ui'
let server: E2EServer | null = null
test.afterEach(async () => { await server?.cleanup(); server = null })
// S1 actual delivered page, S2 HTTP, S3 controlled external recorder, S4 files.
// P12/P68 violation: refresh replays POST/start. Observe 750ms after each
// refresh/reentry and retain external calls; no infinite absence claim.
async function fixture() {
  server = await startServer({ separateCwd: true, herdr: { behavior: { native_session: { source: 'herdr:omp', agent: 'omp', kind: 'id', value: 'browser-manager-04' } } } })
  const zonePath = path.join(path.dirname(server.configPath), 'bot-playground')
  fs.mkdirSync(zonePath)
  let response = await fetch(`${server.baseURL}/api/zones`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ path: zonePath }) })
  expect(response.status).toBe(200)
  const zone = (await response.json()).zones[0].path
  const ids: Record<string, string> = {}
  for (const role of ['manager', 'node']) {
    response = await fetch(`${server.baseURL}/api/tasks`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ name: role, taskType: role, agent: 'omp', chatName: '{chat.id}', model: 'default', thinking: 'default', skill: null, prompt: 'FIXED ROLE', extensions: [] }) })
    expect(response.status).toBe(200)
    ids[role] = (await response.json()).tasks.find((task: { name: string }) => task.name === role).id
  }
  const maps = path.join(server.cwd, '.data/workflow-maps'), bots = path.join(server.cwd, '.data/bots')
  fs.mkdirSync(maps, { recursive: true }); fs.mkdirSync(bots, { recursive: true })
  fs.writeFileSync(path.join(maps, 'launch.json'), JSON.stringify({ title: '启动图', description: '', nodes: { A: { title: 'A', bot_id: 'worker', input: 'NODE ONLY', zone, blocked_by: [] } } }))
  fs.writeFileSync(path.join(bots, 'worker.json'), JSON.stringify({ id: 'worker', name: '', when: '', plan_guide: '', node_guide: '', task_id: ids.node }))
  return { server, zone, ids }
}

test('P12 invalid start stays on the launch form without external effects', async ({ page }, info) => {
  const { server } = await fixture()
  await page.goto(server.baseURL)
  await page.getByRole('tab', { name: 'scheduler', exact: true }).click()
  await page.getByRole('button', { name: 'launch.json', exact: true }).click()
  await page.getByRole('button', { name: '+ 发起 Run', exact: true }).click()
  await page.getByRole('button', { name: '启动 Run', exact: true }).click()
  await expect(page.getByRole('form', { name: '发起 Run' })).toContainText('启动预检未通过')
  expect(page.url()).not.toContain('#workflow/')
  expect(server.herdrCalls()).toEqual([])
  expect(fs.existsSync(path.join(server.cwd, '.data/workflow-runs'))).toBe(false)
  fs.writeFileSync(info.outputPath('rejected-start-external-calls.json'), JSON.stringify(server.herdrCalls()))
})

// P45/P54 violation: showing startup success, hiding the real Chat, or losing
// process-local error on page refresh. Controlled failure is not native-host proof.
test('P45 P54 startup failure and real partial effects survive browser refresh @time-contract', async ({ page }, info) => {
  const { server, zone, ids } = await fixture()
  server.setHerdrBehavior({ native_session: { source: 'herdr:omp', agent: 'omp', kind: 'id', value: 'failed-manager-04' }, fail_prompt: true })
  const response = await page.request.post(`${server.baseURL}/api/scheduler/maps/launch.json/runs`, { data: { managerTaskId: ids.manager, zone, userInput: '' } })
  expect(response.status()).toBe(500)
  const detail = await response.json()
  expect(detail.startupError.stage).toBe('submit')
  await page.goto(`${server.baseURL}/#workflow/${detail.id}`)
  await expect(page.getByRole('alert', { name: '管理者启动错误' })).toContainText('已发生的外部效果未回滚')
  await expect(page.getByText('failed-manager-04', { exact: true })).toHaveCount(2)
  const calls = server.herdrCalls()
  await page.reload()
  await expect(page.getByRole('heading', { name: '管理者启动失败 · submit' })).toBeVisible()
  await page.waitForTimeout(750)
  expect(server.herdrCalls()).toEqual(calls)
  expect(server.diagnostics()).toContain(`run=${detail.id}`)
  expect(server.diagnostics()).toContain('stage=submit')
  fs.writeFileSync(info.outputPath('failure-effects-and-calls.json'), JSON.stringify({ detail, calls, diagnostics: server.diagnostics() }, null, 2))
  await page.screenshot({ path: info.outputPath('startup-error-refresh.png') })
})

test('P9 P10 P12 P67 P68 manager Run opens refreshable read-only details @time-contract', async ({ page }, info) => {
  const { server, zone, ids } = await fixture()
  const runRoot = path.join(path.dirname(server.configPath), 'nondefault-run-records')
  fs.appendFileSync(server.configPath, `\n[scheduler]\nrun_directory = '${runRoot}'\n`)
  await server.restart()
  await page.goto(server.baseURL)
  await page.getByRole('tab', { name: 'scheduler', exact: true }).click()
  await page.getByRole('button', { name: 'launch.json', exact: true }).click()
  await page.getByRole('button', { name: '+ 发起 Run', exact: true }).click()
  await page.getByRole('combobox', { name: '管理者 Task' }).selectOption(ids.manager)
  await page.getByRole('combobox', { name: '管理者 zone' }).selectOption(zone)
  const requirements = '# 浏览器原文\n\n- 不可改写  空格\n末尾\n'
  await page.getByRole('textbox', { name: '本次要求' }).fill(requirements)
  await page.getByRole('button', { name: '启动 Run', exact: true }).click()
  await expect(page.getByRole('heading', { name: 'Run 详情' })).toBeVisible()
  const url = page.url()
  expect(url).toContain('#workflow/')
  const runId = url.split('#workflow/')[1]
  expect(fs.existsSync(path.join(runRoot, runId, 'run.json'))).toBe(true)
  expect(fs.existsSync(path.join(server.cwd, '.data/workflow-runs'))).toBe(false)
  expect(server.herdrCalls().find(call => call.kind === 'agent_prompt')?.text).toContain(path.join(runRoot, runId, 'run.json'))
  await expect(page.getByTestId('run-user-input')).toHaveText(requirements)
  await expect(page.getByRole('textbox', { name: '本次要求' })).toHaveCount(0)
  await expect(page.getByText('browser-manager-04', { exact: true })).toBeVisible()
  const count = server.herdrCalls().filter(call => call.kind === 'agent_start').length
  expect(count).toBe(1)
  await page.reload()
  await expect(page.getByRole('heading', { name: 'Run 详情' })).toBeVisible()
  await page.waitForTimeout(750)
  await page.getByRole('tab', { name: 'chat', exact: true }).click()
  await page.getByRole('tab', { name: 'workflow', exact: true }).click()
  await expect(page.getByRole('heading', { name: 'Run 详情' })).toBeVisible()
  await page.waitForTimeout(750)
  expect(server.herdrCalls().filter(call => call.kind === 'agent_start')).toHaveLength(count)
  fs.writeFileSync(info.outputPath('external-calls.json'), JSON.stringify(server.herdrCalls(), null, 2))
  fs.writeFileSync(info.outputPath('run-records.json'), JSON.stringify({
    root: runRoot, serviceCwd: server.cwd,
    run: JSON.parse(fs.readFileSync(path.join(runRoot, runId, 'run.json'), 'utf8')),
    history: fs.readFileSync(path.join(runRoot, runId, 'history.jsonl'), 'utf8'),
    messages: fs.readFileSync(path.join(runRoot, runId, 'messages.jsonl'), 'utf8'),
  }, null, 2))
  await page.screenshot({ path: info.outputPath('run-details.png') })
  await page.setViewportSize({ width: 390, height: 844 })
  await expectNoHorizontalOverflow(page)
  await page.screenshot({ path: info.outputPath('run-details-mobile.png') })
})
