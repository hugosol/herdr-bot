import fs from 'node:fs'
import path from 'node:path'
import { expect, test } from '@playwright/test'
import { startServer, type E2EServer } from './support'

let server: E2EServer | null = null
test.afterEach(async () => { await server?.cleanup(); server = null })

// Approved seam: S1, the delivered page reading real isolated files. Independent
// source: P10/P11's fixed A/B -> C, isolated D; never derive expected edges from
// the returned graph. A reversed arrow or edge-only node list must fail here.
test('P10 P11 unordered Map keeps stable dependency layers in list and graph with only prerequisite-to-dependent arrows', async ({ page }, testInfo) => {
  server = await startServer()
  const maps = path.join(server.cwd, '.data', 'workflow-maps')
  fs.mkdirSync(maps, { recursive: true })
  fs.writeFileSync(path.join(maps, 'parallel.json'), JSON.stringify({ title: '并行与汇合', description: '独立预期：A → C，B → C；D 无连接。', nodes: {
    C: { title: '汇合', bot_id: 'shared', input: JSON.stringify({}), zone: 'unregistered-zone', blocked_by: ['A', 'B'] },
    B: { title: '准备乙', bot_id: 'shared', input: JSON.stringify({}), zone: 'unregistered-zone', blocked_by: [] },
    A: { title: '准备甲', bot_id: 'shared', input: JSON.stringify({}), zone: 'unregistered-zone', blocked_by: [] },
    D: { title: '独立工作', bot_id: 'shared', input: JSON.stringify({}), zone: 'unregistered-zone', blocked_by: [] },
  } }))
  await page.goto(server.baseURL)
  await page.getByTestId('tab-scheduler').click()
  await page.getByRole('button', { name: 'parallel.json', exact: true }).click()
  await page.getByText('依赖图', { exact: true }).click()
  const graph = page.getByRole('group', { name: 'Map 依赖图', exact: true })
  await expect(graph).toBeVisible()
  // The persisted JSON is C/B/A/D, but the service serializes its parsed Map
  // in canonical key order. Preserve parallel order from the actual definition
  // the page receives, not the original file's insignificant JSON key order.
  const current = await (await page.request.get(`${server.baseURL}/api/scheduler/maps/parallel.json`)).json()
  const peers = Object.keys(current.nodes).filter(id => id !== 'C')
  const expectedOrder = [...peers, 'C'].map(id => `${current.nodes[id].title} ${id}`)
  const outline = page.getByRole('region', { name: '节点列表', exact: true })
  expect(await outline.getByRole('button').evaluateAll(buttons => buttons.map(button => button.getAttribute('aria-label')))).toEqual(expectedOrder)
  expect(await graph.getByRole('button').evaluateAll(buttons => buttons.map(button => button.getAttribute('aria-label')))).toEqual(expectedOrder)
  for (const [id, title] of [['A', '准备甲'], ['B', '准备乙'], ['C', '汇合'], ['D', '独立工作']]) {
    const node = graph.getByRole('button', { name: `${title} ${id}`, exact: true })
    await expect(node).toBeVisible()
    await expect(node).toContainText(title)
    await expect(node).toContainText(id)
  }
  await expect(graph.getByRole('img')).toHaveCount(2)
  // Hit testing uses viewport coordinates; visibility alone permits the graph
  // to remain below the fold after the viewport controls and help text.
  await graph.scrollIntoViewIfNeeded()
  // Observe actual rendered path endpoints against real node rectangles and
  // marker geometry, not merely a computed adjacency list or node count.
  for (const [source, title] of [['A', '准备甲'], ['B', '准备乙']]) {
    const edge = graph.getByRole('img', { name: `${source} → C`, exact: true })
    // Horizontal SVG paths have zero-height bounding boxes, so Playwright's
    // box-based visibility check cannot observe their painted stroke. Hit-test
    // the rendered midpoint instead (also catches invisible/off-canvas edges).
    await expect(edge).toBeAttached()
    const geometry = await edge.evaluate((element) => {
      const line = element as SVGPathElement
      const length = line.getTotalLength()
      const matrix = line.getScreenCTM()!
      const first = line.getPointAtLength(0).matrixTransform(matrix)
      const last = line.getPointAtLength(length).matrixTransform(matrix)
      const markerUrl = getComputedStyle(line).markerEnd
      const markerId = /#([^)"]+)/.exec(markerUrl)?.[1]
      const marker = markerId ? document.getElementById(markerId) : null
      const midpoint = line.getPointAtLength(length / 2).matrixTransform(matrix)
      return { first: { x: first.x, y: first.y }, last: { x: last.x, y: last.y },
        painted: document.elementFromPoint(midpoint.x, midpoint.y) === line && getComputedStyle(line).stroke !== 'none',
        hasArrow: !!marker && marker.childElementCount > 0 }
    })
    const from = (await graph.getByRole('button', { name: `${title} ${source}`, exact: true }).boundingBox())!
    const to = (await graph.getByRole('button', { name: '汇合 C', exact: true }).boundingBox())!
    expect(geometry.painted).toBe(true)
    const distance = (point: { x: number; y: number }, box: { x: number; y: number; width: number; height: number }) =>
      Math.hypot(point.x - Math.max(box.x, Math.min(point.x, box.x + box.width)),
        point.y - Math.max(box.y, Math.min(point.y, box.y + box.height)))
    // The start belongs to the prerequisite and the arrow end to its dependent,
    // regardless of the chosen orientation, spacing or SVG path spelling.
    expect(distance(geometry.first, from)).toBeLessThan(distance(geometry.first, to))
    expect(distance(geometry.last, to)).toBeLessThan(distance(geometry.last, from))
    expect(geometry.hasArrow).toBe(true)
  }
  await page.screenshot({ path: testInfo.outputPath('parallel-join-independent.png'), fullPage: true })
})

// S1; P13/P17 source is the literal JSON below, not bot inputs. A plausible
// violation is merging bot inputs/defaults, validating zone/Bot resources,
// or retaining A's parameters/dependencies after selecting B, then C and A again.
// Real files and HTTP stay in path; only the external Herdr is record-only.
test('P13 P17 same-bot nodes show only current direct JSON parameters and prerequisites safely', async ({ page }, testInfo) => {
  server = await startServer({ herdr: {} })
  const maps = path.join(server.cwd, '.data', 'workflow-maps')
  const bots = path.join(server.cwd, '.data', 'bots')
  fs.mkdirSync(maps, { recursive: true })
  fs.mkdirSync(bots, { recursive: true })
  const outsidePath = path.join(server.cwd, 'must-not-read.txt')
  fs.writeFileSync(outsidePath, 'OUTSIDE_SECRET_MUST_NOT_APPEAR')
  const botBytes = JSON.stringify({ id: 'shared', name: '共享业务能力', when: '测试复用', plan_guide: '不是 node 参数', node_guide: JSON.stringify({ inputs: { target: 'TEMPLATE_DEFAULT_MUST_NOT_APPEAR', template_only: '不可补入参数' }, outputs: {} }) })
  const mapBytes = JSON.stringify({ title: '<script>window.mapExecuted = true</script>', description: '<img src=x onerror="window.mapExecuted = true">', nodes: {
    A: { title: '第一项', bot_id: 'shared', input: ['alpha-only', 'zone-does-not-exist', 'bot-does-not-exist', 'extra_undeclared', '<svg onload="window.mapExecuted = true">', outsidePath, 'false 7 null'].join('\n'), zone: 'unregistered-zone', blocked_by: [] },
    B: { title: '第二项', bot_id: 'shared', input: JSON.stringify({ target: 'beta-only', list: ['two', 2] }), zone: 'unregistered-zone', blocked_by: ['A'] },
    C: { title: '第三项', bot_id: 'shared', input: JSON.stringify({}), zone: 'unregistered-zone', blocked_by: ['B'] },
  } })
  fs.writeFileSync(path.join(bots, 'not-the-id.json'), botBytes)
  fs.writeFileSync(path.join(maps, 'parameters.json'), mapBytes)
  const callsBefore = server.herdrCalls()
  const tabsBefore = server.herdrTabs()
  await page.goto(server.baseURL)
  await page.getByTestId('tab-scheduler').click()
  await page.getByRole('button', { name: 'parameters.json', exact: true }).click()
  const graph = page.getByRole('region', { name: '节点列表', exact: true })
  const details = page.getByRole('region', { name: 'node 详情', exact: true })
  await graph.getByRole('button', { name: '第一项 A', exact: true }).click()
  await expect(details).toBeVisible()
  await expect(details.getByRole('heading', { name: '第一项 · A', exact: true })).toBeVisible()
  const parameters = details.getByRole('region', { name: '本次输入', exact: true })
  const prerequisites = details.getByRole('region', { name: '前置依赖', exact: true })
  for (const text of ['alpha-only', 'zone-does-not-exist', 'bot-does-not-exist', 'extra_undeclared', outsidePath, 'false', '7', 'null']) {
    await expect(parameters).toContainText(text)
  }
  await expect(parameters.getByTestId('node-input')).toContainText('extra_undeclared')
  await expect(prerequisites).toContainText('无前置依赖')
  await expect(details).not.toContainText('template_only')
  await expect(details).not.toContainText('TEMPLATE_DEFAULT_MUST_NOT_APPEAR')
  await graph.getByRole('button', { name: '第二项 B', exact: true }).click()
  await expect(details.getByRole('heading', { name: '第二项 · B', exact: true })).toBeVisible()
  await expect(parameters).toContainText('beta-only')
  await expect(parameters.getByTestId('node-input')).toHaveText('{"target":"beta-only","list":["two",2]}')
  await expect(parameters).not.toContainText('alpha-only')
  await expect(parameters).not.toContainText('extra_undeclared')
  await expect(prerequisites.getByRole('listitem')).toHaveText(['第一项 · A'])
  await graph.getByRole('button', { name: '第三项 C', exact: true }).click()
  await expect(parameters.getByTestId('node-input')).toHaveText('{}')
  await expect(prerequisites.getByRole('listitem')).toHaveText(['第二项 · B'])
  await graph.getByRole('button', { name: '第一项 A', exact: true }).click()
  await expect(parameters).toContainText('alpha-only')
  await expect(parameters).not.toContainText('beta-only')
  await expect(prerequisites).toContainText('无前置依赖')
  await expect(page.getByRole('heading', { name: '<script>window.mapExecuted = true</script>', exact: true })).toBeVisible()
  await expect(page.getByRole('region', { name: 'Map 预览', exact: true })).toContainText('<img src=x onerror="window.mapExecuted = true">')
  expect(await page.evaluate(() => 'mapExecuted' in window)).toBe(false)
  await expect(page.locator('.scheduler img, .scheduler script, .scheduler svg[onload]')).toHaveCount(0)
  await expect(page.locator('body')).not.toContainText('OUTSIDE_SECRET_MUST_NOT_APPEAR')
  expect(server.herdrCalls()).toEqual(callsBefore)
  expect(server.herdrTabs()).toEqual(tabsBefore)
  expect(fs.readFileSync(path.join(maps, 'parameters.json'), 'utf8')).toBe(mapBytes)
  expect(fs.readFileSync(path.join(bots, 'not-the-id.json'), 'utf8')).toBe(botBytes)
  await page.screenshot({ path: testInfo.outputPath('current-node-parameters.png'), fullPage: true })
})
