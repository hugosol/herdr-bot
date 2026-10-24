import { expect, test, type Page } from '@playwright/test'
import fs from 'node:fs'
import path from 'node:path'
import { startServer, type E2EServer } from './support'

// Approved seams: S1 real delivered browser and S2 independent HTTP client.
// Disk fixtures are time-boundary inputs, not an assertion side channel.
let server: E2EServer | null = null
test.afterEach(async () => { await server?.cleanup(); server = null })

function seed(count: number) {
  const file = path.join(path.dirname(server!.configPath), 'session-history.json')
  fs.writeFileSync(file, JSON.stringify(Array.from({ length: count }, (_, i) => ({
    sessionId: `native-${String(i + 1).padStart(3, '0')}`,
    sessionName: `Conversation ${i + 1}`,
    agentType: 'omp', cwd: 'C:/project/' + 'long-directory-'.repeat(18),
    firstRecordedAt: 946684800000 + (i + 1) * 1000, sessionFile: null,
  }))))
  return file
}

async function register(sessionId: string, sessionName: string) {
  const response = await fetch(`${server!.baseURL}/api/sessions/events`, {
    method: 'POST', headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ sessionId, kind: 'register', data: { cwd: 'C:/updated', sessionName } }),
  })
  expect(response.status).toBe(200)
}

async function names(page: Page, expected: string[]) {
  await expect(page.locator('.history-table tbody tr td:first-child')).toHaveText(expected)
}
const range = (high: number, low: number) => Array.from({ length: high - low + 1 }, (_, i) => `Conversation ${high - i}`)

// P12/P13: 123 known distinct first times require exact 50/50/23 pages and
// functional first/last/previous/next controls. A client-side truncated list or
// decorative navigation fails exact row contents on a real delivered page.
for (const viewport of [{ width: 1280, height: 900 }, { width: 390, height: 844 }]) {
  test(`history navigates exact pages with long content at ${viewport.width}px`, async ({ browser }) => {
    server = await startServer()
    seed(123)
    const context = await browser.newContext({ viewport, isMobile: viewport.width < 600, hasTouch: viewport.width < 600 })
    try {
      const page = await context.newPage()
      await page.goto(server.baseURL)
      await expect(page.getByRole('heading', { name: '当前会话', exact: true })).toBeVisible()
      await page.getByRole('button', { name: '查看历史会话' }).click()
      await names(page, range(123, 74))
      await expect(page.getByRole('button', { name: '首页', exact: true })).toBeDisabled()
      await expect(page.getByRole('button', { name: '上一页', exact: true })).toBeDisabled()
      await page.getByRole('button', { name: '下一页', exact: true }).click()
      await names(page, range(73, 24))
      await expect(page.getByText('第 2 / 3 页 · 共 123 条', { exact: true })).toBeVisible()
      await page.getByRole('button', { name: '上一页', exact: true }).click()
      await names(page, range(123, 74))
      await page.getByRole('button', { name: '末页', exact: true }).click()
      await names(page, range(23, 1))
      await expect(page.getByRole('button', { name: '下一页', exact: true })).toBeDisabled()
      await expect(page.getByRole('button', { name: '末页', exact: true })).toBeDisabled()
      await page.getByRole('button', { name: '首页', exact: true }).click()
      await names(page, range(123, 74))
      expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true)
      const geometry = await page.locator('.history-table tbody tr').first().evaluate((row) => Array.from(row.querySelectorAll('td')).map((cell) => ({ x: cell.getBoundingClientRect().x, y: cell.getBoundingClientRect().y, fits: cell.scrollWidth <= cell.clientWidth + 1 })))
      expect(geometry.every((cell) => cell.fits)).toBe(true)
      if (viewport.width < 600) expect(geometry[2].y).toBeGreaterThan(geometry[0].y)
      else expect(geometry[2].x).toBeGreaterThan(geometry[0].x)
      await page.getByRole('button', { name: '末页', exact: true }).scrollIntoViewIfNeeded()
      await page.screenshot({ path: test.info().outputPath(`pagination-${viewport.width}.png`) })
    } finally { await context.close() }
  })
}

// P16: the independent HTTP client changes durable data while the browser stays
// open. Exact old values + no read during a bounded observation window catch SSE
// coupling/polling; navigation then requires fresh data, not a frozen snapshot.
test('history stays still until navigation and always reopens with fresh durable results', { tag: '@time-contract' }, async ({ page }) => {
  server = await startServer()
  seed(123)
  const reads: string[] = []
  page.on('request', (request) => { if (new URL(request.url()).pathname === '/api/sessions/history') reads.push(request.url()) })
  await page.goto(server.baseURL)
  await expect(page.getByRole('heading', { name: '当前会话', exact: true })).toBeVisible()
  expect(reads).toHaveLength(0)
  await page.getByRole('button', { name: '查看历史会话' }).click()
  await names(page, range(123, 74))
  expect(reads).toHaveLength(1)
  await register('native-123', 'Changed original newest')
  await register('native-001', 'Changed oldest')
  await register('new-independent', 'New independent arrival')
  // Deliberate bounded no-refresh observation, longer than the 5s maintenance
  // cadence; not a scheduling sleep used to make an async operation succeed.
  await page.waitForTimeout(6100)
  await names(page, range(123, 74))
  expect(reads).toHaveLength(1)
  await page.getByRole('button', { name: '下一页', exact: true }).click()
  await names(page, range(74, 25))
  expect(reads).toHaveLength(2)
  await page.getByRole('button', { name: '上一页', exact: true }).click()
  await names(page, ['New independent arrival', 'Changed original newest', ...range(122, 75)])
  await page.getByRole('button', { name: '末页', exact: true }).click()
  await names(page, [...range(24, 2), 'Changed oldest'])
  await page.getByRole('button', { name: '查看当前会话' }).click()
  await register('new-after-exit', 'Added after exit')
  await page.getByRole('button', { name: '查看历史会话' }).click()
  await names(page, ['Added after exit', 'New independent arrival', 'Changed original newest', ...range(122, 76)])
  await page.reload()
  await expect(page.getByRole('heading', { name: '当前会话', exact: true })).toBeVisible()
  await expect(page.getByRole('region', { name: '历史会话' })).toHaveCount(0)
})

// P13 boundary UI: exact content, no phantom next page for empty/full data,
// and precisely one row on the 51st-record page.
for (const count of [0, 50, 51]) {
  test(`history boundary ${count} has truthful navigation`, async ({ page }) => {
    server = await startServer()
    seed(count)
    await page.goto(server.baseURL)
    await page.getByRole('button', { name: '查看历史会话' }).click()
    if (count === 0) {
      await expect(page.getByText('尚无历史会话。成功接入的对话会收录在这里。', { exact: true })).toBeVisible()
      await expect(page.locator('.history-table')).toHaveCount(0)
    } else await names(page, range(count, count === 51 ? 2 : 1))
    await expect(page.getByRole('button', { name: '首页', exact: true })).toBeDisabled()
    await expect(page.getByRole('button', { name: '上一页', exact: true })).toBeDisabled()
    const next = page.getByRole('button', { name: '下一页', exact: true })
    if (count < 51) {
      await expect(next).toBeDisabled()
      await expect(page.getByRole('button', { name: '末页', exact: true })).toBeDisabled()
    } else {
      await next.click()
      await names(page, ['Conversation 1'])
      await expect(next).toBeDisabled()
      await page.getByRole('button', { name: '上一页', exact: true }).click()
      await names(page, range(51, 2))
    }
  })
}

// A real unreadable document encountered on next page must not look empty or
// leave stale rows labelled as the new page. Recovery uses the existing toggle.
test('page read failure is explicit and never a fabricated empty page', async ({ page }) => {
  server = await startServer()
  const file = seed(51)
  const saved = fs.readFileSync(file)
  await page.goto(server.baseURL)
  await page.getByRole('button', { name: '查看历史会话' }).click()
  await names(page, range(51, 2))
  fs.writeFileSync(file, 'not valid JSON')
  await page.getByRole('button', { name: '下一页', exact: true }).click()
  await expect(page.getByRole('alert')).toContainText('读取历史会话失败')
  await expect(page.getByText('尚无历史会话。成功接入的对话会收录在这里。', { exact: true })).toHaveCount(0)
  await expect(page.locator('.history-table')).toHaveCount(0)
  await expect(page.getByRole('navigation', { name: '历史分页' })).toHaveCount(0)
  fs.writeFileSync(file, saved)
  await page.getByRole('button', { name: '查看当前会话' }).click()
  await page.getByRole('button', { name: '查看历史会话' }).click()
  await names(page, range(51, 2))
  await page.getByRole('button', { name: '下一页', exact: true }).click()
  await names(page, ['Conversation 1'])
})
