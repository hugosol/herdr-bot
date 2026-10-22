import { expect, test } from '@playwright/test'
import { lanIPv4, startServer, type E2EServer } from './support'

let server: E2EServer | null = null
test.afterEach(async () => { await server?.cleanup(); server = null })

// Approved S1/S2. P14/P15: a durable metadata directory, not a second status list.
// A plausible violation is a cosmetic triangle that leaves both panels present,
// or current-session cards reused with status/actions. Real clicks and exact four
// columns distinguish those errors. Long unbroken strings exercise actual wrapping.
for (const viewport of [{ width: 1280, height: 900 }, { width: 390, height: 844 }]) {
  test(`chat current/history are exclusive and readable at ${viewport.width}px`, async ({ page }) => {
    await page.setViewportSize(viewport)
    server = await startServer()
    const longName = 'Long native conversation name ' + '名称'.repeat(45)
    const longPath = 'C:/project/' + 'long-directory-'.repeat(16) + '/source'
    for (const [sessionId, data] of [
      ['history-long-full-id', { cwd: longPath, sessionName: longName, agentType: 'omp', model: 'SECRET-MODEL', sessionFile: 'C:/private/native.jsonl' }],
      ['history-unknown-full-session-id-not-a-prefix', { cwd: '' }],
    ] as const) {
      const result = await fetch(`${server.baseURL}/api/sessions/events`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ sessionId, kind: 'register', data }) })
      expect(result.status).toBe(200)
    }
    await page.goto(server.baseURL)
    await expect(page.getByTestId('session-board')).toBeVisible()
    await page.getByRole('button', { name: '查看历史会话' }).click()
    const history = page.getByRole('region', { name: '历史会话' })
    await expect(history).toBeVisible()
    await expect(page.getByRole('heading', { name: '历史会话', exact: true })).toBeVisible()
    await expect(page.getByTestId('session-board')).toHaveCount(0)
    const headers = history.getByRole('columnheader')
    await expect(headers).toHaveText(['名称', 'agent 类型', 'cwd', '首次收录时间'])
    const longRow = history.getByRole('row').filter({ hasText: longName })
    await expect(longRow.getByRole('cell').nth(0)).toHaveText(longName)
    await expect(longRow.getByRole('cell').nth(1)).toHaveText('omp')
    await expect(longRow.getByRole('cell').nth(2)).toHaveText(longPath)
    await expect(longRow.locator('time')).toHaveAttribute('datetime', /\d{4}-\d{2}-\d{2}T/)
    const missing = history.getByRole('row').filter({ hasText: 'history-unknown-full-session-id-not-a-prefix' })
    await expect(missing.getByRole('cell').nth(0)).toHaveText('history-unknown-full-session-id-not-a-prefix')
    await expect(missing.getByRole('cell').nth(1)).toHaveText('—')
    await expect(missing.getByRole('cell').nth(2)).toHaveText('—')
    await expect(history).not.toContainText(/SECRET-MODEL|private\/native|keepAlive|在线|离线|关机|来源/)
    await expect(history.getByRole('button')).toHaveCount(0)
    await expect(history.getByRole('textbox')).toHaveCount(0)
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true)
    const rowGeometry = await longRow.evaluate((row) => {
      const cells = Array.from(row.querySelectorAll('td'))
      return cells.map((cell) => ({ x: cell.getBoundingClientRect().x, y: cell.getBoundingClientRect().y, scroll: cell.scrollWidth, width: cell.clientWidth, text: cell.textContent }))
    })
    expect(rowGeometry.every((cell) => cell.scroll <= cell.width + 1)).toBe(true)
    if (viewport.width < 600) expect(rowGeometry[1].y).toBeGreaterThan(rowGeometry[0].y)
    else expect(rowGeometry[1].x).toBeGreaterThan(rowGeometry[0].x)
    const historyColor = await page.getByTestId('chat-views').evaluate((element) => getComputedStyle(element).backgroundColor)
    await page.screenshot({ path: test.info().outputPath(`history-${viewport.width}.png`), fullPage: true })
    await page.getByRole('button', { name: '查看当前会话' }).click()
    await expect(page.getByRole('heading', { name: '当前会话', exact: true })).toBeVisible()
    await expect(page.getByTestId('session-board')).toBeVisible()
    await expect(history).toHaveCount(0)
    const currentColor = await page.getByTestId('chat-views').evaluate((element) => getComputedStyle(element).backgroundColor)
    expect(currentColor).not.toBe(historyColor)
  })
}

// P06: a real process restart, independent HTTP clients and two browser contexts
// distinguish server persistence from a browser cache or process-local list.
test('durable history survives refresh, browser closure and backend restart for independent device contexts', async ({ browser }) => {
  server = await startServer({ listenAddr: '0.0.0.0:0' })
  const startedAt = Date.now()
  for (const [sessionId, data] of [
    ['durable-native-one-full-id', { cwd: 'C:/durable/one', sessionName: 'Durable one', agentType: 'pi' }],
    ['durable-native-two-full-id', { cwd: 'C:/durable/two' }],
  ] as const) {
    const response = await fetch(`${server.baseURL}/api/sessions/events`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ sessionId, kind: 'register', data }) })
    expect(response.status).toBe(200)
  }
  const first = await fetch(`${server.baseURL}/api/sessions/history`).then((response) => response.json())
  const savedBy = Date.now()
  expect(first.sessions).toHaveLength(2)
  const byId = [...first.sessions].sort((a, b) => a.sessionId.localeCompare(b.sessionId))
  expect(byId[0]).toEqual({ sessionId: 'durable-native-one-full-id', sessionName: 'Durable one', agentType: 'pi', cwd: 'C:/durable/one', firstRecordedAt: expect.any(Number) })
  expect(byId[1]).toEqual({ sessionId: 'durable-native-two-full-id', sessionName: null, agentType: null, cwd: 'C:/durable/two', firstRecordedAt: expect.any(Number) })
  for (const record of byId) { expect(record.firstRecordedAt).toBeGreaterThanOrEqual(startedAt); expect(record.firstRecordedAt).toBeLessThanOrEqual(savedBy) }
  const desktop = await browser.newContext({ viewport: { width: 1280, height: 900 } })
  const page = await desktop.newPage()
  await page.goto(server.baseURL)
  await page.getByRole('button', { name: '查看历史会话' }).click()
  await expect(page.getByRole('row').filter({ hasText: 'Durable one' })).toBeVisible()
  await page.reload()
  await expect(page.getByRole('heading', { name: '当前会话', exact: true })).toBeVisible()
  await expect(page.getByRole('region', { name: '历史会话' })).toHaveCount(0)
  await desktop.close()
  expect(await fetch(`${server.baseURL}/api/sessions/history`).then((response) => response.json())).toEqual(first)
  await server.restart()
  expect(await fetch(`${server.baseURL}/api/sessions/history`).then((response) => response.json())).toEqual(first)
  const mobile = await browser.newContext({ viewport: { width: 390, height: 844 }, isMobile: true, hasTouch: true })
  try {
    const reopened = await mobile.newPage()
    const host = lanIPv4()
    const url = host === null ? server.baseURL : server.urlForHost(host)
    test.info().annotations.push({ type: 'independent browser read', description: url })
    await reopened.goto(url)
    await expect(reopened.getByRole('heading', { name: '当前会话', exact: true })).toBeVisible()
    await expect(reopened.getByTestId('session-card')).toHaveCount(0)
    await reopened.getByRole('button', { name: '查看历史会话' }).click()
    const history = reopened.getByRole('region', { name: '历史会话' })
    await expect(history.getByRole('row')).toHaveCount(3)
    await expect(history.getByRole('row').filter({ hasText: 'Durable one' })).toBeVisible()
    await expect(history.getByRole('row').filter({ hasText: 'durable-native-two-full-id' })).toBeVisible()
    const readback = await reopened.request.get(`${url}/api/sessions/history`)
    expect(readback.status()).toBe(200)
    expect(await readback.json()).toEqual(first)
    await reopened.reload()
    await expect(reopened.getByRole('heading', { name: '当前会话', exact: true })).toBeVisible()
    expect(await fetch(`${server.baseURL}/api/sessions/history`).then((response) => response.json())).toEqual(first)
  } finally { await mobile.close() }
})
