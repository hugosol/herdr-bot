import fs from 'node:fs'
import path from 'node:path'
import { expect, test, type Locator } from '@playwright/test'
import { startServer, type E2EServer } from './support'

let server: E2EServer | null = null
test.afterEach(async () => { await server?.cleanup(); server = null })

async function prepare() {
  server = await startServer({ herdr: {} })
  const maps = path.join(server.cwd, '.data', 'workflow-maps')
  const templates = path.join(server.cwd, '.data', 'node-templates')
  fs.mkdirSync(maps, { recursive: true })
  fs.mkdirSync(templates, { recursive: true })
  fs.writeFileSync(path.join(maps, 'parallel.json'), JSON.stringify({
    schema_version: 1, title: '并行、汇合与独立工作', description: 'A、B 汇合到 C；D 独立。', nodes: {
      A: { title: '准备甲', node_template_id: 'shared', parameters: { city: '上海' }, blocked_by: [] },
      B: { title: '准备乙', node_template_id: 'shared', parameters: { city: '北京' }, blocked_by: [] },
      C: { title: '汇合', node_template_id: 'shared', parameters: { result: '核对两项准备' }, blocked_by: ['A', 'B'] },
      D: { title: '独立工作', node_template_id: 'shared', parameters: {}, blocked_by: [] },
    },
  }))
  fs.writeFileSync(path.join(templates, 'shared.json'), JSON.stringify({
    schema_version: 1, id: 'shared', title: '共享核对能力', use_when: '需要核对城市时。',
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

// Approved seam S1: real delivered page, HTTP and isolated real files. P12's
// source is A/B -> C plus isolated D above. A no-op zoom/pan/fit, missing node,
// or overlapping layout must fail via rendered rectangles, not internal calls.
test('P12 automatic graph supports real mouse pan zoom and fit on desktop', async ({ page }, testInfo) => {
  const running = await prepare()
  await page.goto(running.baseURL)
  await page.getByTestId('tab-scheduler').click()
  await page.getByRole('button', { name: 'parallel.json', exact: true }).click()
  const graph = page.getByRole('group', { name: 'Map 依赖图', exact: true })
  await expect(graph).toBeVisible()
  await expectFitted(graph)
  const node = graph.getByRole('button', { name: '准备甲 A', exact: true })
  const initial = await rectangle(node)
  await page.getByRole('button', { name: '放大依赖图', exact: true }).click()
  await expect.poll(async () => (await rectangle(node)).width).toBeGreaterThan(initial.width * 1.1)
  const enlarged = await rectangle(node)
  await page.getByRole('button', { name: '缩小依赖图', exact: true }).click()
  await expect.poll(async () => (await rectangle(node)).width).toBeLessThan(enlarged.width)
  await graph.scrollIntoViewIfNeeded()
  const beforePan = await rectangle(node)
  const canvas = await rectangle(graph)
  await page.mouse.move(canvas.x + canvas.width - 12, canvas.y + canvas.height - 12)
  await page.mouse.down()
  await page.mouse.move(canvas.x + canvas.width - 92, canvas.y + canvas.height - 52, { steps: 8 })
  await page.mouse.up()
  await expect.poll(async () => (await rectangle(node)).x).toBeLessThan(beforePan.x - 40)
  await page.getByRole('button', { name: '适应画布', exact: true }).click()
  await expectFitted(graph)
  await expect.poll(async () => (await rectangle(node)).x).toBeCloseTo(initial.x, 0)
  await page.screenshot({ path: testInfo.outputPath('desktop-parallel-join-independent.png'), fullPage: true })
})

// P4 node/detail keyboard ownership and P30: real 390px viewport, native Tab /
// Enter / Space only for focus and activation. A mouse-only node, unreachable
// template entry, stale node details, or page-wide horizontal crop must fail.
test('P4 P30 narrow screen keyboard reaches Map graph node and template details and returns', async ({ page }, testInfo) => {
  await page.setViewportSize({ width: 390, height: 844 })
  const running = await prepare()
  await page.goto(running.baseURL)
  await page.getByTestId('tab-scheduler').click()
  // Start from the existing scheduler tab; do not focus nodes with a script.
  const file = page.getByRole('button', { name: 'parallel.json', exact: true })
  for (let count = 0; count < 15 && !(await file.evaluate((el) => el === document.activeElement)); count++) await page.keyboard.press('Tab')
  await expect(file).toBeFocused()
  await page.keyboard.press('Enter')
  const graph = page.getByRole('group', { name: 'Map 依赖图', exact: true })
  await expect(graph).toBeVisible()
  await expectFitted(graph)
  const zoom = page.getByRole('button', { name: '放大依赖图', exact: true })
  for (let count = 0; count < 10 && !(await zoom.evaluate((el) => el === document.activeElement)); count++) await page.keyboard.press('Tab')
  await expect(zoom).toBeFocused()
  const first = graph.getByRole('button', { name: '准备甲 A', exact: true })
  const fittedWidth = (await rectangle(first)).width
  await page.keyboard.press('Enter')
  await page.keyboard.press('Enter')
  await expect.poll(async () => (await rectangle(first)).width).toBeGreaterThan(fittedWidth)
  for (let count = 0; count < 10 && !(await first.evaluate((el) => el === document.activeElement)); count++) await page.keyboard.press('Tab')
  await expect(first).toBeFocused()
  await expectFitted(graph)
  await page.keyboard.press('Enter')
  const details = page.getByRole('region', { name: 'node 详情', exact: true })
  await expect(details.getByTestId('parameter-city')).toHaveText('上海')
  await page.keyboard.press('Tab')
  const second = graph.getByRole('button', { name: '准备乙 B', exact: true })
  await expect(second).toBeFocused()
  await page.keyboard.press('Space')
  await expect(second).toHaveAttribute('aria-pressed', 'true')
  await expect(details.getByTestId('parameter-city')).toHaveText('北京')
  const toggle = details.getByRole('button', { name: '模板明细', exact: true })
  for (let count = 0; count < 10 && !(await toggle.evaluate((el) => el === document.activeElement)); count++) await page.keyboard.press('Tab')
  await expect(toggle).toBeFocused()
  await page.keyboard.press('Enter')
  const template = page.getByRole('region', { name: '模板明细', exact: true })
  await expect(template.getByText('共享核对能力', { exact: true })).toBeVisible()
  await template.getByText('核对摘要。', { exact: true }).scrollIntoViewIfNeeded()
  await expect(template.getByText('核对摘要。', { exact: true })).toBeInViewport()
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true)
  await page.screenshot({ path: testInfo.outputPath('narrow-template-details.png'), fullPage: true })
  await page.keyboard.press('Space')
  await expect(template).toHaveCount(0)
  // Return using the native reverse tab order, then select another file/list.
  for (let count = 0; count < 15 && !(await file.evaluate((el) => el === document.activeElement)); count++) await page.keyboard.press('Shift+Tab')
  await expect(file).toBeFocused()
  await expect(file).toBeInViewport()
  await expectFitted(graph)
  await page.screenshot({ path: testInfo.outputPath('narrow-parallel-join-independent.png'), fullPage: true })
  expect(running.herdrCalls()).toEqual([])
})

test.describe('touch graph controls', () => {
  test.use({ hasTouch: true, viewport: { width: 390, height: 844 } })

  // Existing pointer behavior is exercised through Chromium touch input, not a
  // DOM-dispatched event or direct handler call. A mouse-only pan cannot pass.
  test('P12 P30 single-finger pan and tap zoom fit preserve narrow node access', async ({ page }, testInfo) => {
    const running = await prepare()
    await page.goto(running.baseURL)
    await page.getByTestId('tab-scheduler').tap()
    await page.getByRole('button', { name: 'parallel.json', exact: true }).tap()
    const graph = page.getByRole('group', { name: 'Map 依赖图', exact: true })
    await expect(graph).toBeVisible()
    const node = graph.getByRole('button', { name: '准备甲 A', exact: true })
    const initial = await rectangle(node)
    await page.getByRole('button', { name: '放大依赖图', exact: true }).tap()
    await expect.poll(async () => (await rectangle(node)).width).toBeGreaterThan(initial.width)
    await graph.scrollIntoViewIfNeeded()
    const beforePan = await rectangle(node)
    const canvas = await rectangle(graph)
    const input = await page.context().newCDPSession(page)
    try {
      await input.send('Input.synthesizeScrollGesture', {
        x: canvas.x + canvas.width - 12, y: canvas.y + canvas.height - 12,
        xDistance: -80, yDistance: -40, gestureSourceType: 'touch', preventFling: true,
      })
    } finally { await input.detach() }
    await expect.poll(async () => (await rectangle(node)).x).toBeLessThan(beforePan.x - 40)
    await page.getByRole('button', { name: '适应画布', exact: true }).tap()
    await expect.poll(async () => (await rectangle(node)).width).toBeCloseTo(initial.width, 0)
    await expectFitted(graph)
    await node.tap()
    const details = page.getByRole('region', { name: 'node 详情', exact: true })
    await expect(details.getByTestId('parameter-city')).toHaveText('上海')
    await details.getByRole('button', { name: '模板明细', exact: true }).tap()
    await expect(page.getByRole('region', { name: '模板明细', exact: true }).getByText('共享核对能力', { exact: true })).toBeVisible()
    await page.screenshot({ path: testInfo.outputPath('narrow-touch-details.png'), fullPage: true })
  })
})
