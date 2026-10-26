import fs from 'node:fs'
import path from 'node:path'
import { expect, test, type Locator } from '@playwright/test'
import { startServer, type E2EServer } from './support'

let server: E2EServer | null = null
test.afterEach(async () => { await server?.cleanup(); server = null })

async function prepare() {
  server = await startServer({ herdr: {} })
  const maps = path.join(server.cwd, '.data', 'workflow-maps')
  const bots = path.join(server.cwd, '.data', 'bots')
  fs.mkdirSync(maps, { recursive: true })
  fs.mkdirSync(bots, { recursive: true })
  fs.writeFileSync(path.join(maps, 'parallel.json'), JSON.stringify({
    schema_version: 1, title: '并行、汇合与独立工作', description: 'A、B 汇合到 C；D 独立。', nodes: {
      A: { title: '准备甲', bot_id: 'shared', parameters: { city: '上海' }, blocked_by: [] },
      B: { title: '准备乙', bot_id: 'shared', parameters: { city: '北京' }, blocked_by: [] },
      C: { title: '汇合', bot_id: 'shared', parameters: { result: '核对两项准备' }, blocked_by: ['A', 'B'] },
      D: { title: '独立工作', bot_id: 'shared', parameters: {}, blocked_by: [] },
    },
  }))
  fs.writeFileSync(path.join(bots, 'shared.json'), JSON.stringify({
    schema_version: 1, id: 'shared', name: '共享核对能力', use_when: '需要核对城市时。',
    description: '只读能力说明。', inputs: { city: '目标城市。' }, outputs: { summary: '核对摘要。' }, implementation: { type: 'chat' },
  }))
  return server
}

async function rectangle(locator: Locator) {
  const box = await locator.boundingBox()
  expect(box).not.toBeNull()
  return box!
}

async function expectFitted(graph: Locator) {
  const canvas = await rectangle(graph)
  const visibleWidth = await graph.evaluate(() => window.innerWidth)
  expect(canvas.x).toBeGreaterThanOrEqual(0)
  expect(canvas.x + canvas.width).toBeLessThanOrEqual(visibleWidth)
  const nodes = graph.getByRole('button')
  await expect(nodes).toHaveCount(4)
  const boxes = []
  for (const node of await nodes.all()) {
    const box = await rectangle(node)
    expect(box.x).toBeGreaterThanOrEqual(canvas.x)
    expect(box.y).toBeGreaterThanOrEqual(canvas.y)
    expect(box.x + box.width).toBeLessThanOrEqual(canvas.x + canvas.width)
    expect(box.y + box.height).toBeLessThanOrEqual(canvas.y + canvas.height)
    boxes.push(box)
  }
  // No algorithm/coordinate prescription: every independent/parallel/join node
  // must have its own unobscured selectable area after automatic layout.
  for (let a = 0; a < boxes.length; a++) for (let b = a + 1; b < boxes.length; b++) {
    const left = boxes[a], right = boxes[b]
    expect(left.x + left.width <= right.x || right.x + right.width <= left.x ||
      left.y + left.height <= right.y || right.y + right.height <= left.y).toBe(true)
  }
}

// Real delivered page, HTTP and isolated files: parallel, joined and independent
// nodes must remain visible and non-overlapping after automatic layout.
test('automatic graph displays parallel joined and independent nodes on desktop', async ({ page }, testInfo) => {
  const running = await prepare()
  await page.goto(running.baseURL)
  await page.getByTestId('tab-scheduler').click()
  await page.getByRole('button', { name: 'parallel.json', exact: true }).click()
  await page.getByText('依赖图', { exact: true }).click()
  const graph = page.getByRole('group', { name: 'Map 依赖图', exact: true })
  await expect(graph).toBeVisible()
  await expectFitted(graph)
  await page.screenshot({ path: testInfo.outputPath('desktop-parallel-join-independent.png'), fullPage: true })
})

// P4 node/detail keyboard ownership and P30: real 390px viewport, native Tab /
// Enter / Space only for focus and activation. A mouse-only node, unreachable
// bot entry, stale node details, or page-wide horizontal crop must fail.
test('P4 P30 narrow screen keyboard reaches node list and bot details and returns', async ({ page }, testInfo) => {
  await page.setViewportSize({ width: 390, height: 844 })
  const running = await prepare()
  await page.goto(running.baseURL)
  await page.getByTestId('tab-scheduler').click()
  // Bound traversal by the actual controls, not an incidental fixed Tab count.
  const tabTo = async (target: Locator, reverse = false) => {
    const controls = await page.locator('button, input, summary, [tabindex="0"]').count()
    for (let count = 0; count <= controls && !(await target.evaluate(el => el === document.activeElement)); count++) {
      await page.keyboard.press(reverse ? 'Shift+Tab' : 'Tab')
    }
    await expect(target).toBeFocused()
  }
  const file = page.getByRole('button', { name: 'parallel.json', exact: true })
  await tabTo(file)
  await expect(file).toBeFocused()
  await page.keyboard.press('Enter')
  const nodes = page.getByRole('region', { name: '节点列表', exact: true })
  await expect(nodes).toBeVisible()
  const first = nodes.getByRole('button', { name: '准备甲 A', exact: true })
  await tabTo(first)
  await expect(first).toBeFocused()
  await page.keyboard.press('Enter')
  const details = page.getByRole('region', { name: 'node 详情', exact: true })
  await expect(details.getByTestId('parameter-city')).toHaveText('上海')
  const second = nodes.getByRole('button', { name: '准备乙 B', exact: true })
  await tabTo(second)
  await page.keyboard.press('Space')
  await expect(second).toHaveAttribute('aria-pressed', 'true')
  await expect(details.getByTestId('parameter-city')).toHaveText('北京')
  const toggle = details.getByRole('button', { name: 'Bot 明细', exact: true })
  await tabTo(toggle)
  await expect(toggle).toBeFocused()
  await page.keyboard.press('Enter')
  const bot = page.getByRole('region', { name: 'Bot 预览', exact: true })
  await expect(bot.getByText('共享核对能力', { exact: true })).toBeVisible()
  await tabTo(bot.getByRole('button', { name: '输入与输出', exact: true }))
  await page.keyboard.press('Enter')
  await bot.getByText('核对摘要。', { exact: true }).scrollIntoViewIfNeeded()
  await expect(bot.getByText('核对摘要。', { exact: true })).toBeInViewport()
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true)
  await page.screenshot({ path: testInfo.outputPath('narrow-bot-details.png'), fullPage: true })
  await tabTo(page.getByRole('button', { name: '返回 Map', exact: true }))
  await page.keyboard.press('Space')
  await expect(bot).toHaveCount(0)
  // Returning reloads the Map. Wait for its controls before bounding Tab traversal;
  // the transient loading view has fewer controls than the restored node view.
  await expect(nodes).toBeVisible()
  await expect(second).toHaveAttribute('aria-pressed', 'true')
  // Return using the native reverse tab order, then select another file/list.
  await tabTo(file, true)
  await expect(file).toBeInViewport()
  await page.screenshot({ path: testInfo.outputPath('narrow-parallel-join-independent.png'), fullPage: true })
  expect(running.herdrCalls()).toEqual([])
})

test('filtering to no matches preserves the current Map and selected node without reading again', async ({ page }) => {
  const running = await prepare()
  let reads = 0
  page.on('request', request => { if (new URL(request.url()).pathname.startsWith('/api/scheduler/')) reads++ })
  await page.goto(running.baseURL)
  await page.getByTestId('tab-scheduler').click()
  await page.getByRole('button', { name: 'parallel.json', exact: true }).click()
  const nodes = page.getByRole('region', { name: '节点列表', exact: true })
  const selected = nodes.getByRole('button', { name: '准备甲 A', exact: true })
  await selected.click()
  const details = page.getByRole('region', { name: 'node 详情', exact: true })
  await expect(details.getByTestId('parameter-city')).toHaveText('上海')
  await expect(page.getByRole('button', { name: '刷新全部', exact: true })).toBeEnabled()
  const beforeFilter = reads
  const filter = page.getByLabel('筛选 Map 文件', { exact: true })
  await filter.fill('no-matching-map')
  await expect(page.getByTestId('scheduler-map-list').getByRole('button')).toHaveCount(0)
  await expect(page.getByTestId('scheduler-empty')).toHaveCount(0)
  await expect(page.getByRole('heading', { name: '并行、汇合与独立工作', exact: true })).toBeVisible()
  await expect(selected).toHaveAttribute('aria-pressed', 'true')
  await expect(details.getByTestId('parameter-city')).toHaveText('上海')
  await filter.fill('PARALLEL')
  await expect(page.getByTestId('scheduler-map-list').getByRole('button')).toHaveCount(1)
  await filter.clear()
  await expect(selected).toHaveAttribute('aria-pressed', 'true')
  expect(reads).toBe(beforeFilter)
})

test('list graph and prerequisite navigation share selection and Map changes reset reading state', async ({ page }) => {
  const running = await prepare()
  fs.writeFileSync(path.join(running.cwd, '.data', 'workflow-maps', 'other.json'), JSON.stringify({
    schema_version: 1, title: '另一计划', description: '', nodes: {
      X: { title: '其他工作', bot_id: 'shared', parameters: { city: '广州' }, blocked_by: [] },
    },
  }))
  await page.goto(running.baseURL)
  await page.getByTestId('tab-scheduler').click()
  await page.getByRole('button', { name: 'parallel.json', exact: true }).click()
  const graph = page.getByRole('group', { name: 'Map 依赖图', exact: true })
  await expect(graph).toHaveCount(0)
  const nodes = page.getByRole('region', { name: '节点列表', exact: true })
  const details = page.getByRole('region', { name: 'node 详情', exact: true })
  await nodes.getByRole('button', { name: '准备甲 A', exact: true }).click()
  await expect(details.getByTestId('parameter-city')).toHaveText('上海')
  await page.getByText('依赖图', { exact: true }).click()
  await expect(graph.getByRole('button', { name: '准备甲 A', exact: true })).toHaveAttribute('aria-pressed', 'true')
  await graph.getByRole('button', { name: '汇合 C', exact: true }).click()
  await expect(nodes.getByRole('button', { name: '汇合 C', exact: true })).toHaveAttribute('aria-pressed', 'true')
  await expect(details.getByTestId('parameter-result')).toHaveText('核对两项准备')
  await details.getByRole('button', { name: '查看前置节点 准备乙 B', exact: true }).click()
  await expect(details.getByTestId('parameter-city')).toHaveText('北京')
  await expect(nodes.getByRole('button', { name: '准备乙 B', exact: true })).toHaveAttribute('aria-pressed', 'true')
  await expect(graph.getByRole('button', { name: '准备乙 B', exact: true })).toHaveAttribute('aria-pressed', 'true')
  await details.getByRole('button', { name: '查看后继节点 汇合 C', exact: true }).click()
  await expect(nodes.getByRole('button', { name: '汇合 C', exact: true })).toHaveAttribute('aria-pressed', 'true')
  await expect(graph.getByRole('button', { name: '汇合 C', exact: true })).toHaveAttribute('aria-pressed', 'true')
  await details.getByRole('button', { name: 'Bot 明细', exact: true }).click()
  await expect(page.getByRole('region', { name: 'Bot 预览', exact: true })).toBeVisible()
  await page.getByRole('button', { name: '返回 Map', exact: true }).click()
  await page.getByRole('button', { name: '关闭节点详情', exact: true }).click()
  await expect(details).toHaveCount(0)
  await expect(nodes.getByRole('button', { name: '汇合 C', exact: true })).toBeFocused()
  await nodes.getByRole('button', { name: '准备甲 A', exact: true }).click()
  await details.getByRole('button', { name: 'Bot 明细', exact: true }).click()
  await page.getByRole('button', { name: '返回 Map', exact: true }).click()
  await page.getByRole('button', { name: 'other.json', exact: true }).click()
  await expect(nodes.getByRole('button', { name: '其他工作 X', exact: true })).toBeVisible()
  await expect(details).toHaveCount(0)
  await expect(page.getByRole('region', { name: 'Bot 预览', exact: true })).toHaveCount(0)
  await expect(graph).toHaveCount(0)
  await nodes.getByRole('button', { name: '其他工作 X', exact: true }).click()
  await details.getByRole('button', { name: 'Bot 明细', exact: true }).click()
  await page.getByRole('button', { name: '返回 Map', exact: true }).click()
  await page.getByText('依赖图', { exact: true }).click()
  await page.getByRole('button', { name: '刷新全部', exact: true }).click()
  await expect(page.getByRole('button', { name: '刷新全部', exact: true })).toBeEnabled()
  await expect(nodes.getByRole('button', { name: '其他工作 X', exact: true })).toHaveAttribute('aria-pressed', 'false')
  await expect(details).toHaveCount(0)
  await expect(page.getByRole('region', { name: 'Bot 预览', exact: true })).toHaveCount(0)
  await expect(graph).toHaveCount(0)
})

test.describe('touch node access', () => {
  test.use({ hasTouch: true, viewport: { width: 390, height: 844 } })

  test('narrow screen touch opens node and bot details', async ({ page }, testInfo) => {
    const running = await prepare()
    await page.goto(running.baseURL)
    await page.getByTestId('tab-scheduler').tap()
    await page.getByRole('button', { name: 'parallel.json', exact: true }).tap()
    const nodes = page.getByRole('region', { name: '节点列表', exact: true })
    await expect(nodes).toBeVisible()
    const node = nodes.getByRole('button', { name: '准备甲 A', exact: true })
    await node.tap()
    const details = page.getByRole('region', { name: 'node 详情', exact: true })
    await expect(details.getByTestId('parameter-city')).toHaveText('上海')
    await details.getByRole('button', { name: 'Bot 明细', exact: true }).tap()
    await expect(page.getByRole('region', { name: 'Bot 预览', exact: true }).getByText('共享核对能力', { exact: true })).toBeVisible()
    await page.screenshot({ path: testInfo.outputPath('narrow-touch-details.png'), fullPage: true })
  })
})
