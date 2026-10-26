import fs from 'node:fs'
import path from 'node:path'
import { expect, test, type Page } from '@playwright/test'
import { startServer, type E2EServer } from './support'

let server: E2EServer | null = null
test.afterEach(async () => { await server?.cleanup(); server = null })

const originalTemplate = {
  schema_version: 1, id: 'shared', title: '原能力标题', use_when: '原适用说明',
  description: '原能力说明', inputs: { target: '原输入说明' }, outputs: { report: '原输出说明' },
  implementation: { type: 'chat' },
}
const updatedTemplate = {
  schema_version: 1, id: 'shared', title: '新能力标题', use_when: '新适用说明',
  description: '新能力说明', inputs: { target: '新输入说明' }, outputs: { report: '新输出说明' },
  implementation: { type: 'chat' },
}
const originalMap = {
  schema_version: 1, title: '原计划', description: '原计划说明', nodes: {
    A: { title: '原准备', node_template_id: 'shared', parameters: { target: 'old-alpha' }, blocked_by: [] },
    B: { title: '原核对', node_template_id: 'shared', parameters: { target: 'old-beta' }, blocked_by: ['A'] },
  },
}
const updatedMap = {
  schema_version: 1, title: '新计划', description: '新计划说明', nodes: {
    A: { title: '新准备', node_template_id: 'shared', parameters: { target: 'new-alpha' }, blocked_by: ['B'] },
    B: { title: '新核对', node_template_id: 'shared', parameters: { target: 'new-beta', count: 17 }, blocked_by: [] },
    C: { title: '新独立项', node_template_id: 'shared', parameters: {}, blocked_by: [] },
  },
}

// S1 P25: fixed before/after filenames distinguish a fresh directory read
// from a startup-only list. Removed and newly created files share one process.
test('P25 refreshing the list sees added and removed Map files in the running service', async ({ page }) => {
  server = await startServer()
  const fixture = files()
  fs.writeFileSync(fixture.map, JSON.stringify(originalMap))
  fs.writeFileSync(fixture.template, JSON.stringify(originalTemplate))
  await page.goto(server.baseURL)
  await page.getByTestId('tab-scheduler').click()
  const list = page.getByTestId('scheduler-map-list')
  await expect(list.getByRole('listitem')).toHaveText(['current.json'])
  await page.getByRole('button', { name: 'current.json', exact: true }).click()
  await expect(page.getByRole('heading', { name: '原计划', exact: true })).toBeVisible()
  fs.unlinkSync(fixture.map)
  const replacement = path.join(fixture.maps, 'replacement.json')
  const bytes = JSON.stringify(updatedMap)
  fs.writeFileSync(replacement, bytes)
  await page.getByRole('button', { name: '刷新全部', exact: true }).click()
  await expect(list.getByRole('listitem')).toHaveText(['replacement.json'])
  await page.getByRole('button', { name: 'replacement.json', exact: true }).click()
  await expect(page.getByRole('heading', { name: '新计划', exact: true })).toBeVisible()
  expect(fs.existsSync(fixture.map)).toBe(false)
  expect(fs.readFileSync(replacement, 'utf8')).toBe(bytes)
})

// Only transport timing is controlled: route.fetch obtains the actual running
// service's bytes before the gate; fulfil forwards that same response unchanged.
async function holdResponse(page: Page, url: string) {
  const captured = Promise.withResolvers<void>()
  const release = Promise.withResolvers<void>()
  const delivered = Promise.withResolvers<void>()
  await page.route(url, async (route) => {
    try {
      const response = await route.fetch()
      captured.resolve()
      await release.promise
      await route.fulfill({ response })
      delivered.resolve()
    } catch (error) {
      captured.reject(error)
      delivered.reject(error)
    }
  }, { times: 1 })
  return { captured: captured.promise, release: release.resolve, delivered: delivered.promise }
}

// S1 P26. Old A responses contain the original plan/capability, while B has
// independently specified new values. Holding both real responses catches
// selection-scoped Map guards that forget to guard the template request.
// @time-contract: late-result attribution, not an elapsed-time budget.
for (const scenario of ['selection', 'reload'] as const) {
test(`P26 @time-contract late ${scenario} responses cannot replace current graph node or expanded template`, async ({ page }) => {
  server = await startServer()
  const fixture = files()
  fs.writeFileSync(fixture.map, JSON.stringify(originalMap))
  fs.writeFileSync(fixture.template, JSON.stringify(originalTemplate))
  fs.writeFileSync(path.join(fixture.maps, 'next.json'), JSON.stringify(updatedMap))
  await page.goto(server.baseURL)
  await page.getByTestId('tab-scheduler').click()
  await page.getByRole('button', { name: 'current.json', exact: true }).click()
  await page.getByRole('region', { name: '节点列表', exact: true }).getByRole('button', { name: '原核对 B', exact: true }).click()
  await page.getByRole('button', { name: '模板明细', exact: true }).click()
  await expect(page.getByText('原能力说明', { exact: true })).toBeVisible()
  await page.getByRole('button', { name: '返回 Map', exact: true }).click()
  if (scenario === 'selection') {
    await page.getByRole('button', { name: 'next.json', exact: true }).click()
    await page.getByRole('region', { name: '节点列表', exact: true }).getByRole('button', { name: '新核对 B', exact: true }).click()
    await page.getByRole('button', { name: '模板明细', exact: true }).click()
    await expect(page.getByText('原能力说明', { exact: true })).toBeVisible()
    await page.getByRole('button', { name: '返回 Map', exact: true }).click()
  }
  const map = await holdResponse(page, '**/api/scheduler/maps/current.json')
  const template = await holdResponse(page, '**/api/scheduler/templates')
  try {
    await page.getByRole('button', { name: scenario === 'selection' ? 'current.json' : '刷新全部', exact: true }).click()
    await Promise.all([map.captured, template.captured])
    fs.writeFileSync(fixture.template, JSON.stringify(updatedTemplate))
    await expect(page.getByRole('button', { name: '刷新全部', exact: true })).toBeDisabled()
    await expect(page.getByRole('region', { name: 'node 详情', exact: true })).toHaveCount(0)
    await expect(page.getByRole('region', { name: '模板预览', exact: true })).toHaveCount(0)
    await page.getByRole('button', { name: 'next.json', exact: true }).click()
    await page.getByRole('region', { name: '节点列表', exact: true }).getByRole('button', { name: '新核对 B', exact: true }).click()
    await page.getByRole('button', { name: '模板明细', exact: true }).click()
    await expect(page.getByRole('region', { name: '模板预览', exact: true }).getByText('新能力说明', { exact: true })).toBeVisible()
    await page.getByRole('button', { name: '返回 Map', exact: true }).click()
    map.release()
    template.release()
    await Promise.all([map.delivered, template.delivered])
    // A rendering opportunity after transport release, rather than a sleep.
    await page.evaluate(() => {
      const frame = Promise.withResolvers<void>()
      requestAnimationFrame(() => frame.resolve())
      return frame.promise
    })
    await expect(page.getByRole('button', { name: 'next.json', exact: true })).toHaveAttribute('aria-pressed', 'true')
    await expect(page.getByRole('button', { name: 'current.json', exact: true })).toHaveAttribute('aria-pressed', 'false')
    await expect(page.getByRole('button', { name: '刷新全部', exact: true })).toBeEnabled()
    const preview = page.getByRole('region', { name: 'Map 预览', exact: true })
    await expect(preview.getByRole('heading', { name: '新计划', exact: true })).toBeVisible()
    await page.getByText('依赖图', { exact: true }).click()
    const graph = preview.getByRole('group', { name: 'Map 依赖图', exact: true })
    await expect(graph.getByRole('img')).toHaveCount(1)
    await expect(preview.getByRole('img', { name: 'B → A', exact: true })).toBeAttached()
    const details = page.getByRole('region', { name: 'node 详情', exact: true })
    await expect(details.getByRole('heading', { name: '新核对 · B', exact: true })).toBeVisible()
    await expect(details.getByTestId('parameter-target')).toHaveText('new-beta')
    await expect(details.getByTestId('parameter-count')).toHaveText('17')
    const capability = page.getByRole('region', { name: '模板预览', exact: true })
    await details.getByRole('button', { name: '模板明细', exact: true }).click()
    await expect(capability.getByText('新能力说明', { exact: true })).toBeVisible()
    await page.getByRole('button', { name: '返回 Map', exact: true }).click()
    await expect(preview).not.toContainText('原')
  } finally {
    map.release()
    template.release()
    await Promise.all([map.delivered, template.delivered])
  }
})
}

function files() {
  const maps = path.join(server!.cwd, '.data', 'workflow-maps')
  const templates = path.join(server!.cwd, '.data', 'node-templates')
  fs.mkdirSync(maps, { recursive: true })
  fs.mkdirSync(templates, { recursive: true })
  return { map: path.join(maps, 'current.json'), template: path.join(templates, 'capability.json'), maps }
}

// Approved seam S1, P25. The literal new plan/capability above is independent
// of the response. A cached Map or template would leave A -> B, old-beta or an
// old capability field. One real process reads both generations; no restart.
test('P25 explicit reload replaces dependencies parameters and every template field without restarting', async ({ page }) => {
  server = await startServer({ herdr: {} })
  const fixture = files()
  fs.writeFileSync(fixture.map, JSON.stringify(originalMap))
  fs.writeFileSync(fixture.template, JSON.stringify(originalTemplate))
  await page.goto(server.baseURL)
  await page.getByTestId('tab-scheduler').click()
  await page.getByRole('button', { name: 'current.json', exact: true }).click()
  await page.getByRole('region', { name: '节点列表', exact: true }).getByRole('button', { name: '原核对 B', exact: true }).click()
  const details = page.getByRole('region', { name: 'node 详情', exact: true })
  const capability = page.getByRole('region', { name: '模板预览', exact: true })
  await expect(details.getByTestId('parameter-target')).toHaveText('old-beta')
  await details.getByRole('button', { name: '模板明细', exact: true }).click()
  await expect(capability.getByText('原能力说明', { exact: true })).toBeVisible()
  await page.getByRole('button', { name: '返回 Map', exact: true }).click()

  const newMapBytes = `${JSON.stringify(updatedMap)}\n`
  const newTemplateBytes = `${JSON.stringify(updatedTemplate)}\n`
  fs.writeFileSync(fixture.map, newMapBytes)
  fs.writeFileSync(fixture.template, newTemplateBytes)
  await page.getByRole('button', { name: '刷新全部', exact: true }).click()
  const preview = page.getByRole('region', { name: 'Map 预览', exact: true })
  await expect(preview.getByRole('heading', { name: '新计划', exact: true })).toBeVisible()
  await expect(preview.locator('details')).not.toHaveAttribute('open', '')
  await page.getByText('依赖图', { exact: true }).click()
  const graph = preview.getByRole('group', { name: 'Map 依赖图', exact: true })
  await expect(graph.getByRole('button')).toHaveCount(3)
  for (const name of ['新准备 A', '新核对 B', '新独立项 C']) await expect(graph.getByRole('button', { name, exact: true })).toBeVisible()
  await expect(graph.getByRole('img')).toHaveCount(1)
  await expect(graph.getByRole('img', { name: 'B → A', exact: true })).toBeAttached()
  await expect(graph.getByRole('img', { name: 'A → B', exact: true })).toHaveCount(0)
  await expect(details).toHaveCount(0)
  await expect(capability).toHaveCount(0)
  await graph.getByRole('button', { name: '新核对 B', exact: true }).click()
  await expect(details.getByTestId('parameter-target')).toHaveText('new-beta')
  await expect(details.getByTestId('parameter-count')).toHaveText('17')
  await expect(details.getByRole('region', { name: '前置依赖' }).getByRole('listitem')).toHaveCount(0)
  await details.getByRole('button', { name: '模板明细', exact: true }).click()
  for (const text of ['新能力标题', '新适用说明', '新能力说明', 'chat']) {
    await expect(capability.getByText(text, { exact: true })).toBeVisible()
  }
  await capability.getByRole('button', { name: '输入与输出', exact: true }).click()
  for (const text of ['target', '新输入说明', 'report', '新输出说明']) {
    await expect(capability.getByText(text, { exact: true })).toBeVisible()
  }
  await page.getByRole('button', { name: '返回 Map', exact: true }).click()
  await graph.getByRole('button', { name: '新准备 A', exact: true }).click()
  await expect(details.getByTestId('parameter-target')).toHaveText('new-alpha')
  await expect(details.getByRole('region', { name: '前置依赖' }).getByRole('listitem')).toHaveText(['新核对 · B'])
  await expect(preview).not.toContainText('原')
  expect(fs.readFileSync(fixture.map, 'utf8')).toBe(newMapBytes)
  expect(fs.readFileSync(fixture.template, 'utf8')).toBe(newTemplateBytes)
  expect(server.herdrCalls()).toEqual([])
})
