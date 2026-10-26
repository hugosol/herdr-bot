import fs from 'node:fs'
import path from 'node:path'
import { execFileSync } from 'node:child_process'
import { expect, test } from '@playwright/test'
import { startServer, type E2EServer } from './support'

let server: E2EServer | null = null
test.afterEach(async () => { await server?.cleanup(); server = null })

const weather = {
  schema_version: 1, id: 'weather', title: '目的地天气摘要',
  use_when: '出行前需要了解目的地天气时。', description: '整理天气及穿衣建议；不预订行程。',
  inputs: { city: '需要查询的城市名称。', date: '计划出行日期。' },
  outputs: { forecast: '温度、降水和穿衣建议的文本摘要。' }, implementation: { type: 'chat' },
}

// S1 P14/P15: these independently written expected texts distinguish a complete
// capability description from merely echoing the node parameters or stale data.
// Opening uses real keyboard input. Selecting a different template must collapse
// and replace the previous description; no HTML is interpreted or work launched.
test('P14 P15 details are opt-in and show every named capability field for the current node', async ({ page }) => {
  server = await startServer({ herdr: {} })
  const maps = path.join(server.cwd, '.data', 'workflow-maps')
  const templates = path.join(server.cwd, '.data', 'node-templates')
  fs.mkdirSync(maps, { recursive: true })
  fs.mkdirSync(templates, { recursive: true })
  const weatherBytes = `${JSON.stringify(weather)}\n`
  fs.writeFileSync(path.join(templates, 'different-filename.json'), weatherBytes)
  const quotaBytes = JSON.stringify({ schema_version: 1, id: 'quota', title: '额度恢复时间',
    use_when: '<img src=x onerror="window.templateExecuted=true">',
    description: '<script>window.templateExecuted=true</script>',
    inputs: { account: '仅作说明的账户名称。' }, outputs: { reset: '预计恢复时间；不是实际结果。' }, implementation: { type: 'chat' } })
  fs.writeFileSync(path.join(templates, 'quota-description.json'), quotaBytes)
  const mapBytes = JSON.stringify({ schema_version: 1, title: '业务核对', description: '只读定义', nodes: {
    A: { title: '查询上海', node_template_id: 'weather', parameters: { city: '上海' }, blocked_by: [] },
    B: { title: '核对额度', node_template_id: 'quota', parameters: { account: 'not-a-real-account' }, blocked_by: ['A'] },
  } })
  fs.writeFileSync(path.join(maps, 'work.json'), mapBytes)
  await page.goto(server.baseURL)
  await page.getByTestId('tab-scheduler').click()
  await page.getByRole('button', { name: 'work.json', exact: true }).click()
  await page.getByRole('button', { name: '查询上海 A', exact: true }).click()
  const details = page.getByRole('region', { name: 'node 详情', exact: true })
  const toggle = details.getByRole('button', { name: '模板明细', exact: true })
  await expect(toggle).toHaveAttribute('aria-expanded', 'false')
  await expect(page.getByRole('region', { name: '模板明细', exact: true })).toHaveCount(0)
  await expect(page.getByText('目的地天气摘要', { exact: true })).toHaveCount(0)
  await toggle.focus()
  await page.keyboard.press('Enter')
  await expect(toggle).toHaveAttribute('aria-expanded', 'true')
  const capability = page.getByRole('region', { name: '模板明细', exact: true })
  for (const text of ['目的地天气摘要', '出行前需要了解目的地天气时。', '整理天气及穿衣建议；不预订行程。',
    'city', '需要查询的城市名称。', 'date', '计划出行日期。', 'forecast', '温度、降水和穿衣建议的文本摘要。', 'chat']) {
    await expect(capability.getByText(text, { exact: true })).toBeVisible()
  }
  await page.getByRole('button', { name: '核对额度 B', exact: true }).click()
  await expect(details.getByRole('button', { name: '模板明细', exact: true })).toHaveAttribute('aria-expanded', 'false')
  await expect(capability).toHaveCount(0)
  await details.getByRole('button', { name: '模板明细', exact: true }).click()
  for (const text of ['额度恢复时间', '<img src=x onerror="window.templateExecuted=true">',
    '<script>window.templateExecuted=true</script>', 'account', '仅作说明的账户名称。', 'reset', '预计恢复时间；不是实际结果。', 'chat']) {
    await expect(capability.getByText(text, { exact: true })).toBeVisible()
  }
  await expect(capability.getByText('目的地天气摘要', { exact: true })).toHaveCount(0)
  expect(await page.evaluate(() => (window as unknown as Record<string, unknown>).templateExecuted)).toBeUndefined()
  await expect(capability.locator('img, script')).toHaveCount(0)
  expect(server.herdrCalls()).toEqual([])
  expect(fs.readFileSync(path.join(templates, 'different-filename.json'), 'utf8')).toBe(weatherBytes)
  expect(fs.readFileSync(path.join(templates, 'quota-description.json'), 'utf8')).toBe(quotaBytes)
  expect(fs.readFileSync(path.join(maps, 'work.json'), 'utf8')).toBe(mapBytes)
})

// P6/P16 and direct-only scanning: existing production behavior is exercised,
// not replaced. Config-relative lookup, filename identity, recursion, or looking
// in the Map directory would each disagree with these fixed expected texts/IDs.
test('P6 P16 configured cwd templates are independent and reusable across nodes Maps and filename changes', async ({ page }) => {
  server = await startServer({ separateCwd: true })
  const maps = path.join(server.cwd, 'plans')
  const templates = path.join(server.cwd, 'capabilities')
  fs.mkdirSync(maps, { recursive: true })
  fs.mkdirSync(path.join(templates, 'nested'), { recursive: true })
  const bytes = `${JSON.stringify(weather)}\n`
  fs.writeFileSync(path.join(templates, 'not-weather.JSON'), bytes)
  fs.writeFileSync(path.join(templates, 'notes.txt'), 'not JSON')
  fs.writeFileSync(path.join(templates, 'nested', 'excluded.json'), JSON.stringify({ ...weather, id: 'nested', title: '嵌套错误能力' }))
  fs.writeFileSync(path.join(maps, 'template-decoy.json'), JSON.stringify({ ...weather, id: 'map-decoy', title: 'Map 目录错误能力' }))
  const wrongCwd = path.join(path.dirname(server.configPath), 'capabilities')
  fs.mkdirSync(wrongCwd)
  fs.writeFileSync(path.join(wrongCwd, 'not-weather.JSON'), JSON.stringify({ ...weather, title: '配置目录错误能力' }))
  fs.writeFileSync(path.join(templates, 'plan-decoy.txt'), 'not a Map')
  fs.writeFileSync(path.join(maps, 'first.json'), JSON.stringify({ schema_version: 1, title: '第一张计划', description: '', nodes: {
    A: { title: '上海天气', node_template_id: 'weather', parameters: { city: '上海' }, blocked_by: [] },
    B: { title: '北京天气', node_template_id: 'weather', parameters: { city: '北京' }, blocked_by: ['A'] },
  } }))
  fs.writeFileSync(path.join(maps, 'second.json'), JSON.stringify({ schema_version: 1, title: '第二张计划', description: '', nodes: {
    C: { title: '广州天气', node_template_id: 'weather', parameters: { city: '广州' }, blocked_by: [] },
  } }))
  fs.appendFileSync(server.configPath, "\n[scheduler]\nmap_directory = 'plans'\nnode_template_directory = 'capabilities'\n")
  await server.restart()
  await page.goto(server.baseURL)
  await page.getByTestId('tab-scheduler').click()
  const catalog = await page.request.get(`${server.baseURL}/api/scheduler/templates`)
  expect(catalog.status()).toBe(200)
  expect(await catalog.json()).toEqual({ templates: [weather] })
  const listing = await page.request.get(`${server.baseURL}/api/scheduler/maps`)
  expect(await listing.json()).toEqual({ files: ['first.json', 'second.json', 'template-decoy.json'] })
  for (const [filename, node, city] of [['first.json', '上海天气 A', '上海'], ['first.json', '北京天气 B', '北京'], ['second.json', '广州天气 C', '广州']]) {
    await page.getByRole('button', { name: filename, exact: true }).click()
    await page.getByRole('button', { name: node, exact: true }).click()
    const details = page.getByRole('region', { name: 'node 详情', exact: true })
    await expect(details.getByTestId('parameter-city')).toHaveText(city)
    await expect(details.getByRole('button', { name: '模板明细', exact: true })).toHaveAttribute('aria-expanded', 'false')
    await details.getByRole('button', { name: '模板明细', exact: true }).click()
    await expect(page.getByRole('region', { name: '模板明细', exact: true }).getByRole('heading', { name: '目的地天气摘要', exact: true })).toBeVisible()
  }
  fs.renameSync(path.join(templates, 'not-weather.JSON'), path.join(templates, 'renamed-again.json'))
  // Re-enter the page to read current files; ticket 07 owns full explicit reload coverage.
  await page.reload()
  await page.getByTestId('tab-scheduler').click()
  await page.getByRole('button', { name: 'second.json', exact: true }).click()
  await page.getByRole('button', { name: '广州天气 C', exact: true }).click()
  await page.getByRole('button', { name: '模板明细', exact: true }).click()
  await expect(page.getByRole('region', { name: '模板明细', exact: true })).toContainText('目的地天气摘要')
  expect(await (await page.request.get(`${server.baseURL}/api/scheduler/templates`)).json()).toEqual({ templates: [weather] })
  expect(fs.readFileSync(path.join(templates, 'renamed-again.json'), 'utf8')).toBe(bytes)
})

// P7/P8 S1+S2: a real file obstructs a missing configured multi-level directory.
// Swallowing the filesystem error would incorrectly render the normal empty state.
test('P7 P8 template parents are created while obstructing files report a distinct creation failure', async ({ page }) => {
  server = await startServer({ separateCwd: true })
  const parent = path.join(server.cwd, 'missing', 'parents')
  const templates = path.join(parent, 'templates')
  fs.appendFileSync(server.configPath, "\n[scheduler]\nnode_template_directory = 'missing/parents/templates'\n")
  await server.restart()
  expect(fs.existsSync(parent)).toBe(false)
  await page.goto(server.baseURL)
  await page.getByTestId('tab-scheduler').click()
  await expect(page.getByTestId('scheduler-template-empty')).toBeVisible()
  expect(fs.readdirSync(templates)).toEqual([])
  fs.rmdirSync(templates)
  fs.writeFileSync(templates, 'DO NOT REPLACE THIS USER FILE')
  await page.getByRole('button', { name: '刷新 Map 列表' }).click()
  await expect(page.getByTestId('scheduler-template-error')).toContainText('共享模板目录创建失败')
  await expect(page.getByTestId('scheduler-template-empty')).toHaveCount(0)
  const failure = await page.request.get(`${server.baseURL}/api/scheduler/templates`)
  expect(failure.status()).toBe(500)
  expect(await failure.json()).toMatchObject({ code: 'template-directory-create-failed' })
  expect(fs.readFileSync(templates, 'utf8')).toBe('DO NOT REPLACE THIS USER FILE')
  fs.unlinkSync(templates)
  fs.mkdirSync(templates)
  await page.getByRole('button', { name: '刷新 Map 列表' }).click()
  await expect(page.getByTestId('scheduler-template-error')).toHaveCount(0)
  await expect(page.getByTestId('scheduler-template-empty')).toBeVisible()
  const empty = await page.request.get(`${server.baseURL}/api/scheduler/templates`)
  expect(empty.status()).toBe(200)
  expect(await empty.json()).toEqual({ templates: [] })
})

// P8 S1+S2: deny the real Windows directory-list permission, keeping fs::read_dir
// in the exercised production path. Both empty-success and stale-data fallbacks
// would fail the explicit HTTP error and visible-error/absent-empty observations.
test('P8 actual unreadable shared template directory reports read failure and recovers', async ({ page }) => {
  server = await startServer()
  const templates = path.join(server.cwd, '.data', 'node-templates')
  fs.mkdirSync(templates, { recursive: true })
  fs.writeFileSync(path.join(templates, 'weather.json'), JSON.stringify(weather))
  await page.goto(server.baseURL)
  await page.getByTestId('tab-scheduler').click()
  const initial = await page.request.get(`${server.baseURL}/api/scheduler/templates`)
  expect(initial.status()).toBe(200)
  expect(await initial.json()).toEqual({ templates: [weather] })
  const user = execFileSync('whoami.exe', [], { encoding: 'utf8' }).trim()
  try {
    execFileSync('icacls.exe', [templates, '/deny', `${user}:(RD)`])
    expect(() => fs.readdirSync(templates)).toThrow()
    await page.getByRole('button', { name: '刷新 Map 列表' }).click()
    await expect(page.getByTestId('scheduler-template-error')).toContainText('共享模板目录读取失败')
    await expect(page.getByTestId('scheduler-template-empty')).toHaveCount(0)
    const failure = await page.request.get(`${server.baseURL}/api/scheduler/templates`)
    expect(failure.status()).toBe(500)
    expect(await failure.json()).toMatchObject({ code: 'template-directory-read-failed' })
  } finally {
    execFileSync('icacls.exe', [templates, '/remove:d', user])
  }
  fs.unlinkSync(path.join(templates, 'weather.json'))
  await page.getByRole('button', { name: '刷新 Map 列表' }).click()
  await expect(page.getByTestId('scheduler-template-error')).toHaveCount(0)
  await expect(page.getByTestId('scheduler-template-empty')).toBeVisible()
  const empty = await page.request.get(`${server.baseURL}/api/scheduler/templates`)
  expect(empty.status()).toBe(200)
  expect(await empty.json()).toEqual({ templates: [] })
})

// Approved S1 + S2: actual page and the exact HTTP catalog it calls, real files.
// P7 independent source: .data/node-templates is created without samples. A
// hard-coded empty result or a destructive default installer fails after seeding.
test('P7 shared template default creates empty parents and preserves existing definitions', async ({ page }) => {
  server = await startServer({ separateCwd: true })
  const directory = path.join(server.cwd, '.data', 'node-templates')
  expect(fs.existsSync(directory)).toBe(false)
  await page.goto(server.baseURL)
  await page.getByTestId('tab-scheduler').click()
  await expect(page.getByTestId('scheduler-template-empty')).toHaveText('共享模板目录没有模板 JSON 文件。')
  expect(fs.readdirSync(directory)).toEqual([])
  const first = await page.request.get(`${server.baseURL}/api/scheduler/templates`)
  expect(first.status()).toBe(200)
  expect(await first.json()).toEqual({ templates: [] })
  const bytes = `${JSON.stringify(weather, null, 2)}\n`
  fs.writeFileSync(path.join(directory, 'personal-not-the-id.json'), bytes)
  await page.getByRole('button', { name: '刷新 Map 列表' }).click()
  await expect(page.getByTestId('scheduler-template-empty')).toHaveCount(0)
  const loaded = await page.request.get(`${server.baseURL}/api/scheduler/templates`)
  expect(loaded.status()).toBe(200)
  expect(await loaded.json()).toEqual({ templates: [weather] })
  expect(fs.readFileSync(path.join(directory, 'personal-not-the-id.json'), 'utf8')).toBe(bytes)
  expect(fs.readdirSync(directory)).toEqual(['personal-not-the-id.json'])
})
