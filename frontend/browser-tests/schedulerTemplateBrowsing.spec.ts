import fs from 'node:fs'
import path from 'node:path'
import { expect, test } from '@playwright/test'
import { startServer, type E2EServer } from './support'

let server: E2EServer | null = null
test.afterEach(async () => { await server?.cleanup(); server = null })

// Approved S1: an actual empty directory, not a stub response. Contract P1
// requires adjacent controls and same-Tab navigation; opening a popup or showing
// the Map listing instead cannot satisfy these independent observations.
test('P1 template management beside refresh opens an empty list in the same Tab', async ({ page, context }) => {
  server = await startServer({ separateCwd: true })
  await page.goto(server.baseURL)
  await page.getByTestId('tab-scheduler').click()
  const refresh = page.getByRole('button', { name: '刷新全部', exact: true })
  const manage = page.getByRole('button', { name: '模板管理', exact: true })
  await expect(refresh).toBeEnabled()
  await expect(manage).toBeVisible()
  const refreshBox = await refresh.boundingBox()
  const manageBox = await manage.boundingBox()
  expect(Math.abs(refreshBox!.y - manageBox!.y)).toBeLessThan(8)
  expect(Math.abs(refreshBox!.x - manageBox!.x)).toBeLessThan(250)
  const originalURL = page.url()
  const openedPages: unknown[] = []
  context.on('page', (opened) => openedPages.push(opened))
  await manage.click()
  const listing = page.getByRole('region', { name: '共享模板列表', exact: true })
  await expect(listing).toBeVisible()
  await expect(listing).toContainText('共享模板目录没有模板 JSON 文件。')
  await expect(page.getByRole('complementary', { name: 'Map 文件目录', exact: true })).toHaveCount(0)
  expect(context.pages()).toEqual([page])
  expect(openedPages).toEqual([])
  expect(page.url()).toBe(originalURL)
  expect(fs.readdirSync(path.join(server.cwd, '.data', 'node-templates'))).toEqual([])
  const response = await page.request.get(`${server.baseURL}/api/scheduler/templates`)
  expect(response.status()).toBe(200)
  expect(await response.json()).toEqual({ templates: [] })
  await page.getByRole('button', { name: '返回 Map', exact: true }).click()
  await expect(page.getByRole('complementary', { name: 'Map 文件目录', exact: true })).toBeVisible()
})

// P5/P35 S1+S2: retain the selected identity while external file maintenance
// moves, damages and restores it. A stale preview, automatic repair or sample
// installer disagrees with the literal observations and complete byte inventory.
test('P5 P35 selected preview diagnoses missing and damaged files then recovers without writing definitions', async ({ page }) => {
  server = await startServer({ herdr: {}, separateCwd: true })
  const maps = path.join(server.cwd, '.data', 'workflow-maps')
  const templates = path.join(server.cwd, '.data', 'node-templates')
  fs.mkdirSync(maps, { recursive: true })
  fs.mkdirSync(templates, { recursive: true })
  const capability = { schema_version: 1, id: 'catalog-life', title: '人工核对能力', use_when: '需要只读核对时',
    description: '保留原始文本，不执行工作', inputs: { source: '原始说明' }, outputs: { note: '核对说明' }, implementation: { type: 'chat' } }
  const healthy = { ...capability, id: 'healthy', title: '其他健康能力' }
  const mapBytes = '{"schema_version":1,"title":"不改写的计划","description":"","nodes":{"A":{"title":"核对工作","node_template_id":"catalog-life","parameters":{},"blocked_by":[]}}}\n'
  const templateBytes = `${JSON.stringify(capability, null, 2)}\n`
  const healthyBytes = `${JSON.stringify(healthy)}\n`
  const definition = path.join(templates, 'personal-name.json')
  const saved = path.join(server.cwd, 'saved-definition.json')
  fs.writeFileSync(path.join(maps, 'plan.json'), mapBytes)
  fs.writeFileSync(definition, templateBytes)
  fs.writeFileSync(path.join(templates, 'healthy.json'), healthyBytes)
  const unchanged = (current: string | null) => {
    expect(fs.readFileSync(path.join(maps, 'plan.json'), 'utf8')).toBe(mapBytes)
    expect(fs.readdirSync(maps)).toEqual(['plan.json'])
    expect(fs.readFileSync(path.join(templates, 'healthy.json'), 'utf8')).toBe(healthyBytes)
    expect(fs.readdirSync(templates)).toEqual(current === null ? ['healthy.json'] : ['healthy.json', 'personal-name.json'])
    if (current !== null) expect(fs.readFileSync(definition, 'utf8')).toBe(current)
    expect(server!.herdrCalls()).toEqual([])
  }
  const catalog = async () => {
    const response = await page.request.get(`${server!.baseURL}/api/scheduler/templates`)
    expect(response.status()).toBe(200)
    return response.json()
  }
  await page.goto(server.baseURL)
  await page.getByTestId('tab-scheduler').click()
  await page.getByRole('button', { name: '模板管理', exact: true }).click()
  await page.getByRole('button', { name: '人工核对能力 · catalog-life', exact: true }).click()
  const preview = page.getByRole('region', { name: '模板预览', exact: true })
  await expect(preview.getByText('保留原始文本，不执行工作', { exact: true })).toBeVisible()
  expect(await catalog()).toEqual({ templates: [healthy, capability] })
  unchanged(templateBytes)

  fs.renameSync(definition, saved)
  await page.getByRole('button', { name: '刷新全部', exact: true }).click()
  await expect(preview).toHaveText('缺失共享模板：catalog-life')
  expect(await catalog()).toEqual({ templates: [healthy] })
  unchanged(null)
  expect(fs.readFileSync(saved, 'utf8')).toBe(templateBytes)

  fs.renameSync(saved, definition)
  const brokenBytes = '{ damaged JSON; DO NOT REPAIR\n'
  fs.writeFileSync(definition, brokenBytes)
  await page.getByRole('button', { name: '刷新全部', exact: true }).click()
  await expect(preview).toHaveText('缺失共享模板：catalog-life')
  await expect(page.getByRole('region', { name: '模板问题', exact: true })).toContainText('personal-name.json')
  await expect(page.getByRole('region', { name: '模板问题', exact: true })).toContainText('结构无效')
  expect(await catalog()).toMatchObject({ templates: [healthy], problems: [{ code: 'template-invalid', files: ['personal-name.json'] }] })
  unchanged(brokenBytes)
  await page.getByRole('button', { name: '返回模板列表', exact: true }).click()
  await page.getByRole('button', { name: '其他健康能力 · healthy', exact: true }).click()
  await expect(preview.getByText('healthy', { exact: true })).toBeVisible()
  unchanged(brokenBytes)

  fs.writeFileSync(definition, templateBytes)
  await page.getByRole('button', { name: '刷新全部', exact: true }).click()
  await page.getByRole('button', { name: '返回模板列表', exact: true }).click()
  await page.getByRole('button', { name: '人工核对能力 · catalog-life', exact: true }).click()
  await expect(preview.getByText('保留原始文本，不执行工作', { exact: true })).toBeVisible()
  await expect(page.getByRole('region', { name: '模板问题', exact: true })).toHaveCount(0)
  expect(await catalog()).toEqual({ templates: [healthy, capability] })
  unchanged(templateBytes)
})
