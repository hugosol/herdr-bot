import fs from 'node:fs'
import path from 'node:path'
import { test, expect, type Page, type TestInfo } from '@playwright/test'
import { startServer, type E2EServer } from './support'
import { nativeVisibilityBrowser } from './workflowVisibility'
let server: E2EServer | null = null
test.afterEach(async () => { await server?.cleanup(); server = null })

async function fixture(interval?: number) {
  server = await startServer({ separateCwd: true, herdr: {} })
  if (interval !== undefined) {
    await server.stop()
    fs.appendFileSync(server.configPath, `\n[scheduler]\nworkflow_poll_interval_seconds = ${interval}\n`)
    await server.restart()
  }
  const root = path.join(server.cwd, '.data/workflow-runs/observed-run')
  fs.mkdirSync(root, { recursive: true })
  const maps = path.join(server.cwd, '.data/workflow-maps')
  fs.mkdirSync(maps, { recursive: true })
  fs.writeFileSync(path.join(maps, 'observed-map.json'), JSON.stringify({ title: '轮询图', description: '', nodes: { A: { title: '保留节点', bot_id: 'saved-bot', zone: 'C:/saved-zone', input: '节点输入', blocked_by: [] } } }))
  const run = { id: 'observed-run', map: 'observed-map.json', createdAt: 1000, status: 'running', managerTaskId: 'manager', zone: 'C:/saved-zone', workspaceName: 'saved-workspace', userInput: '保留上次成功的现场', manager: null, finished: null, nodes: { A: { status: 'pending', execution: null, failure: null, completion: null, previousExecutions: [] } } }
  const runPath = path.join(root, 'run.json')
  fs.writeFileSync(runPath, JSON.stringify(run))
  fs.writeFileSync(path.join(root, 'history.jsonl'), '')
  fs.writeFileSync(path.join(root, 'messages.jsonl'), '')
  return { server, run, runPath }
}

async function retain(info: TestInfo, name: string, evidence: unknown) {
  const file = info.outputPath(name)
  fs.writeFileSync(file, JSON.stringify(evidence, null, 2))
  await info.attach(name, { path: file, contentType: 'application/json' })
}

function observe(page: Page) {
  const events: { at: number; method: string; url: string }[] = []
  page.on('request', request => {
    if (new URL(request.url()).pathname.startsWith('/api/workflow/')) events.push({ at: Date.now(), method: request.method(), url: new URL(request.url()).pathname })
  })
  return events
}

// P55.01: hold an actual service response across two configured intervals.
// Starting another cycle, clearing content on refresh, or leaving a live timer
// in an inactive/hidden page must be visible in the external request trace.
test('P55.01 slow and manual refresh never overlap; leave hide and return control polling @time-contract', async ({}, info) => {
  const native = await nativeVisibilityBrowser()
  const { page, context } = native
  try {
  const { server } = await fixture(1)
  const events = observe(page)
  await page.goto(`${server.baseURL}/#workflow/observed-run`)
  await expect(page.getByTestId('run-user-input')).toBeVisible()
  const captured = Promise.withResolvers<void>()
  const release = Promise.withResolvers<void>()
  const delivered = Promise.withResolvers<void>()
  await page.route('**/api/workflow/runs', async route => {
    try {
      const response = await route.fetch()
      captured.resolve()
      await release.promise
      await route.fulfill({ response })
      delivered.resolve()
    } catch (error) { captured.reject(error); delivered.reject(error) }
  }, { times: 1 })
  await page.getByRole('button', { name: '刷新 Run', exact: true }).click()
  await captured.promise
  const held = events.length
  try {
    await page.getByRole('button', { name: '刷新 Run', exact: true }).click({ clickCount: 3 })
    await page.waitForTimeout(2300)
    expect(events.length).toBe(held)
    await expect(page.getByTestId('run-user-input')).toHaveText('保留上次成功的现场')
    await page.getByRole('tab', { name: 'chat', exact: true }).click()
  } finally { release.resolve() }
  await delivered.promise
  await page.waitForTimeout(1500)
  const awayCount = events.filter(event => event.url === '/api/workflow/runs').length
  expect(awayCount).toBe(2)
  expect(events.length).toBe(held)
  const returned = Date.now()
  await page.getByRole('tab', { name: 'workflow', exact: true }).click()
  await expect.poll(() => events.filter(event => event.url === '/api/workflow/runs').length, { timeout: 750 }).toBe(3)
  expect(events.filter(event => event.url === '/api/workflow/runs')[2].at - returned).toBeLessThan(750)
  await expect(page.getByTestId('run-user-input')).toBeVisible()
  const cdp = await context.newCDPSession(page)
  const { windowId } = await cdp.send('Browser.getWindowForTarget')
  await cdp.send('Browser.setWindowBounds', { windowId, bounds: { windowState: 'minimized' } })
  await expect.poll(() => page.evaluate(() => document.visibilityState)).toBe('hidden')
  const hiddenCount = events.filter(event => event.url === '/api/workflow/runs').length
  await page.waitForTimeout(2300)
  expect(events.filter(event => event.url === '/api/workflow/runs').length).toBe(hiddenCount)
  const restored = Date.now()
  await cdp.send('Browser.setWindowBounds', { windowId, bounds: { windowState: 'normal' } })
  await page.bringToFront()
  await expect.poll(() => page.evaluate(() => document.visibilityState)).toBe('visible')
  await expect.poll(() => events.filter(event => event.url === '/api/workflow/runs').length, { timeout: 750 }).toBe(hiddenCount + 1)
  expect(events.filter(event => event.url === '/api/workflow/runs')[hiddenCount].at - restored).toBeLessThan(750)
  await retain(info, 'visibility-slow-timing.json', { heldWindowMs: 2300, hiddenWindowMs: 2300, awayWindowMs: 1500, events })
  } finally { await native.cleanup() }
})

// P56.01 source: the literal saved input/status survive real service read
// errors. A cleared list, partial list commit, or fabricated failed Run status
// is distinguishable from the last successful snapshot and its unchanged time.
test('P56.01 list and detail failures retain last success then recover without Run writes @time-contract', async ({ page }, info) => {
  const { server, run, runPath } = await fixture()
  const events = observe(page)
  const calls = server.herdrCalls()
  const original = fs.readFileSync(runPath, 'utf8')
  await page.goto(`${server.baseURL}/#workflow/observed-run`)
  await expect(page.getByTestId('run-user-input')).toHaveText('保留上次成功的现场')
  const stamp = page.getByRole('status', { name: 'workflow 更新' }).locator('time')
  await expect(stamp).toHaveAttribute('datetime', /T/)
  const lastSuccess = await stamp.getAttribute('datetime')
  const table = page.getByRole('table', { name: '全部 Run' })
  const graphNode = page.getByRole('group', { name: 'Map 依赖图', exact: true }).getByRole('button', { name: '保留节点 A', exact: true })
  await expect(graphNode).toBeVisible()
  const root = path.dirname(path.dirname(runPath))
  fs.renameSync(root, `${root}.saved`)
  fs.writeFileSync(root, 'not a directory')
  try {
    const failedList = page.waitForResponse(response => new URL(response.url()).pathname === '/api/workflow/runs' && response.status() === 500)
    await page.getByRole('button', { name: '刷新 Run', exact: true }).click()
    await failedList
    await expect(page.getByRole('alert').filter({ hasText: '更新失败' })).toBeVisible()
    await expect(table).toContainText('observed-run')
    await expect(table).toContainText('running')
    await expect(page.getByTestId('run-user-input')).toHaveText('保留上次成功的现场')
    await expect(stamp).toHaveAttribute('datetime', lastSuccess!)
    await expect(graphNode).toBeVisible()
  } finally {
    fs.unlinkSync(root)
    fs.renameSync(`${root}.saved`, root)
  }
  fs.writeFileSync(runPath, '{ broken Run')
  const failedDetail = page.waitForResponse(response => new URL(response.url()).pathname === '/api/workflow/runs/observed-run' && response.status() === 500)
  await page.getByRole('button', { name: '刷新 Run', exact: true }).click()
  await failedDetail
  await expect(page.getByRole('alert').filter({ hasText: '更新失败' })).toBeVisible()
  await expect(table).toContainText('running')
  await expect(table).not.toContainText('记录读取失败')
  await expect(page.getByTestId('run-user-input')).toHaveText('保留上次成功的现场')
  await expect(stamp).toHaveAttribute('datetime', lastSuccess!)
  await expect(graphNode).toBeVisible()
  expect(fs.readFileSync(runPath, 'utf8')).toBe('{ broken Run')
  fs.writeFileSync(runPath, JSON.stringify({ ...run, status: 'paused', userInput: '恢复后的现场' }))
  const restored = fs.readFileSync(runPath, 'utf8')
  // No manual refresh: the next configured poll must clear the stale error.
  await expect(page.getByTestId('run-user-input')).toHaveText('恢复后的现场', { timeout: 7500 })
  await expect(page.getByRole('alert').filter({ hasText: '更新失败' })).toHaveCount(0)
  await expect(table).toContainText('paused')
  expect(await stamp.getAttribute('datetime')).not.toBe(lastSuccess)
  expect(fs.readFileSync(runPath, 'utf8')).toBe(restored)
  expect(server.herdrCalls()).toEqual(calls)
  expect(events.every(event => event.method === 'GET')).toBe(true)
  await page.screenshot({ path: info.outputPath('recovered-workflow.png'), fullPage: true })
  await retain(info, 'failure-recovery.json', { original, restored, lastSuccess, recoveredAt: await stamp.getAttribute('datetime'), events, herdrCallsBefore: calls, herdrCallsAfter: server.herdrCalls() })
})

// P67.03 source: explicit non-default deployment configuration, not an UI
// setting. Ignoring the service value would leave a five-second gap instead.
test('P67.03 service configuration controls actual page cadence without UI settings @time-contract', async ({ page }, info) => {
  const { server } = await fixture(1)
  const events = observe(page)
  await page.goto(`${server.baseURL}/#workflow/observed-run`)
  await expect(page.getByTestId('run-user-input')).toBeVisible()
  await expect.poll(() => events.filter(event => event.url === '/api/workflow/runs').length, { timeout: 3500 }).toBeGreaterThanOrEqual(3)
  const lists = events.filter(event => event.url === '/api/workflow/runs')
  for (let index = 1; index < 3; index++) {
    expect(lists[index].at - lists[index - 1].at).toBeGreaterThanOrEqual(950)
    expect(lists[index].at - lists[index - 1].at).toBeLessThan(2000)
  }
  await expect(page.getByRole('spinbutton')).toHaveCount(0)
  await expect(page.getByRole('combobox')).toHaveCount(0)
  expect(events.every(event => event.method === 'GET')).toBe(true)
  await retain(info, 'custom-request-timing.json', { config: fs.readFileSync(server.configPath, 'utf8'), events })
})

// S1/S2: actual delivered page and HTTP requests. Source: P55's literal default
// of five seconds. A mount-only fetch, or a fast generic refresh timer, violates
// this contract; observing a second list request distinguishes both.
test('P55.01 visible workflow defaults to five-second polling @time-contract', async ({ page }, info) => {
  const { server } = await fixture()
  const events = observe(page)
  await page.goto(`${server.baseURL}/#workflow/observed-run`)
  await expect(page.getByTestId('run-user-input')).toHaveText('保留上次成功的现场')
  await expect.poll(() => events.filter(event => event.url === '/api/workflow/runs').length, { timeout: 7500 }).toBe(2)
  const lists = events.filter(event => event.url === '/api/workflow/runs')
  expect(lists[1].at - lists[0].at).toBeGreaterThanOrEqual(4800)
  expect(lists[1].at - lists[0].at).toBeLessThan(6500)
  await retain(info, 'request-timing.json', events)
})
