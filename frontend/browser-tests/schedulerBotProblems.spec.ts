import fs from 'node:fs'
import path from 'node:path'
import { execFileSync } from 'node:child_process'
import { expect, test, type Page } from '@playwright/test'
import { startServer, type E2EServer } from './support'

let server: E2EServer | null = null
test.afterEach(async () => { await server?.cleanup(); server = null })

const normal = { id: 'normal', name: '正常能力说明', when: '需要核对材料时', plan_guide: '仅整理差异，不执行工作', node_guide: JSON.stringify({ inputs: { source: '待核对的文本' }, outputs: { report: '差异报告说明' } }) }
const plan = { title: 'Bot 问题隔离计划', description: '', nodes: {
  A: { title: '问题工作', bot_id: 'problem', input: JSON.stringify({ target: '原始参数' }), zone: 'unregistered-zone', blocked_by: [] },
  B: { title: '正常工作', bot_id: 'normal', input: JSON.stringify({}), zone: 'unregistered-zone', blocked_by: ['A'] },
  C: { title: '独立工作', bot_id: 'normal', input: JSON.stringify({}), zone: 'unregistered-zone', blocked_by: [] },
} }

async function prepare() {
  server = await startServer({ herdr: {} })
  const maps = path.join(server.cwd, '.data', 'workflow-maps')
  const bots = path.join(server.cwd, '.data', 'bots')
  fs.mkdirSync(maps, { recursive: true })
  fs.mkdirSync(bots, { recursive: true })
  fs.writeFileSync(path.join(maps, 'plan.json'), JSON.stringify(plan))
  fs.writeFileSync(path.join(bots, 'normal.json'), JSON.stringify(normal))
  return { bots, maps }
}

async function openPlan(page: Page) {
  await page.goto(server!.baseURL)
  await page.getByTestId('tab-scheduler').click()
  await page.getByRole('button', { name: 'plan.json', exact: true }).click()
}

async function completeGraph(page: Page) {
  await expect(page.getByRole('region', { name: '节点列表', exact: true }).getByRole('button')).toHaveCount(3)
  await page.getByText('依赖图', { exact: true }).click()
  const graph = page.getByRole('group', { name: 'Map 依赖图', exact: true })
  await expect(graph.getByRole('button')).toHaveCount(3)
  for (const name of ['问题工作 A', '正常工作 B', '独立工作 C']) await expect(graph.getByRole('button', { name, exact: true })).toBeVisible()
  await expect(graph.getByRole('img')).toHaveCount(1)
  await expect(graph.getByRole('img', { name: 'A → B', exact: true })).toBeAttached()
  return graph
}

async function normalDetails(page: Page) {
  if (await page.getByRole('button', { name: '返回 Map', exact: true }).isVisible()) {
    await page.getByRole('button', { name: '返回 Map', exact: true }).click()
  }
  await page.getByRole('region', { name: '节点列表', exact: true }).getByRole('button', { name: '正常工作 B', exact: true }).click()
  await page.getByRole('region', { name: 'node 详情', exact: true }).getByRole('button', { name: 'Bot 明细', exact: true }).click()
  const details = page.getByRole('region', { name: 'Bot 预览', exact: true })
  await expect(details.getByText('正常能力说明', { exact: true })).toBeVisible()
  await expect(details.getByRole('region', { name: '选用说明', exact: true })).toHaveText('需要核对材料时')
  await details.getByRole('button', { name: '规划指引', exact: true }).click()
  await expect(details.getByRole('region', { name: '规划指引', exact: true })).toHaveText('仅整理差异，不执行工作')
  await details.getByRole('button', { name: '节点指引', exact: true }).click()
  for (const text of ['待核对的文本', '差异报告说明']) await expect(details.getByRole('region', { name: '节点指引', exact: true })).toContainText(text)
}

// S1 + production S2, P20/P29: a real bad file coexists with a normal
// capability. Aborting the whole catalog or returning empty success fails both
// the explicit file diagnostic and the normal node's independently named text.
test('P20 P29 malformed JSON is a local file problem and normal bot remains available', async ({ page }) => {
  const { bots } = await prepare()
  const bad = path.join(bots, 'broken.json')
  fs.writeFileSync(bad, '{ broken JSON')
  await openPlan(page)
  await completeGraph(page)
  const response = await page.request.get(`${server!.baseURL}/api/scheduler/bots`)
  expect(response.status()).toBe(200)
  expect(await response.json()).toMatchObject({ bots: [normal], problems: [{ code: 'bot-invalid', files: ['broken.json'], message: expect.stringContaining('结构无效') }] })
  await expect(page.getByRole('region', { name: 'Bot 问题', exact: true })).toContainText('broken.json')
  await normalDetails(page)
  expect(fs.readFileSync(bad, 'utf8')).toBe('{ broken JSON')
  expect(server!.herdrCalls()).toEqual([])
})

// P20/P29 S1/S2: deny ReadData on one real file, not directory listing.
// A mocked 500 or directory-wide failure cannot establish local isolation.
test('P20 P29 an actual unreadable bot file leaves the valid graph and normal capability usable', async ({ page }) => {
  const { bots } = await prepare()
  const unreadable = path.join(bots, 'unreadable.json')
  const bytes = JSON.stringify({ ...normal, id: 'problem', name: '不可读取的业务说明' })
  fs.writeFileSync(unreadable, bytes)
  const user = execFileSync('whoami.exe', [], { encoding: 'utf8' }).trim()
  try {
    execFileSync('icacls.exe', [unreadable, '/deny', `${user}:(RD)`])
    expect(() => fs.readFileSync(unreadable)).toThrow()
    expect(fs.readdirSync(bots)).toEqual(['normal.json', 'unreadable.json'])
    await openPlan(page)
    await completeGraph(page)
    const response = await page.request.get(`${server!.baseURL}/api/scheduler/bots`)
    expect(response.status()).toBe(200)
    expect(await response.json()).toMatchObject({ bots: [normal], problems: [{ code: 'bot-read-failed', files: ['unreadable.json'], message: expect.stringContaining('读取失败') }] })
    await expect(page.getByRole('region', { name: 'Bot 问题', exact: true })).toContainText('unreadable.json')
    await page.getByRole('region', { name: '节点列表', exact: true }).getByRole('button', { name: '问题工作 A', exact: true }).click()
    const node = page.getByRole('region', { name: 'node 详情', exact: true })
    await node.getByRole('button', { name: 'Bot 明细', exact: true }).click()
    const detail = page.getByRole('region', { name: 'Bot 预览', exact: true })
    await expect(detail).toContainText('problem')
    await expect(detail).not.toContainText('chat')
    await expect(detail).not.toContainText('不可读取的业务说明')
    await normalDetails(page)
    expect(server!.herdrCalls()).toEqual([])
  } finally {
    execFileSync('icacls.exe', [unreadable, '/remove:d', user])
  }
  expect(fs.readFileSync(unreadable, 'utf8')).toBe(bytes)
})

// P21 S1/S2: two distinct business descriptions claim one id. First/last
// winner indexing fails the zero-description check and both filename checks.
test('P5 P35 duplicate identity diagnoses every conflicting file in list and node preview without choosing a winner', async ({ page }) => {
  const { bots } = await prepare()
  const first = JSON.stringify({ ...normal, id: 'problem', name: '禁止任选第一说明', plan_guide: 'FIRST_BUSINESS_TEXT' })
  const second = JSON.stringify({ ...normal, id: 'problem', name: '禁止任选第二说明', plan_guide: 'SECOND_BUSINESS_TEXT' })
  fs.writeFileSync(path.join(bots, 'first.json'), first)
  fs.writeFileSync(path.join(bots, 'second.json'), second)
  await openPlan(page)
  const graph = await completeGraph(page)
  await expect(page.getByRole('region', { name: '节点列表', exact: true }).getByRole('button', { name: '问题工作 A', exact: true })).toContainText('冲突')
  const response = await page.request.get(`${server!.baseURL}/api/scheduler/bots`)
  expect(response.status()).toBe(200)
  expect(await response.json()).toMatchObject({ bots: [normal], problems: [{ code: 'bot-id-conflict', bot_id: 'problem', files: ['first.json', 'second.json'] }] })
  const issues = page.getByRole('region', { name: 'Bot 问题', exact: true })
  for (const text of ['冲突', 'problem', 'first.json', 'second.json']) await expect(issues).toContainText(text)
  await graph.getByRole('button', { name: '问题工作 A', exact: true }).click()
  const node = page.getByRole('region', { name: 'node 详情', exact: true })
  await expect(node.getByRole('alert')).toContainText('冲突')
  await node.getByRole('button', { name: 'Bot 明细', exact: true }).click()
  const detail = page.getByRole('region', { name: 'Bot 预览', exact: true })
  for (const text of ['first.json', 'second.json', 'problem']) await expect(detail).toContainText(text)
  for (const text of ['FIRST_BUSINESS_TEXT', 'SECOND_BUSINESS_TEXT', '禁止任选第一说明', '禁止任选第二说明', 'chat']) await expect(detail).not.toContainText(text)
  const listing = page.getByRole('region', { name: '共享 Bot 列表', exact: true })
  await expect(listing.getByRole('button')).toHaveCount(1)
  for (const text of ['冲突', 'problem', 'first.json', 'second.json']) await expect(issues).toContainText(text)
  await listing.getByRole('button', { name: '正常能力说明 · normal', exact: true }).click()
  await expect(detail.getByText('正常能力说明', { exact: true })).toBeVisible()
  await expect(detail).not.toContainText('FIRST_BUSINESS_TEXT')
  await expect(detail).not.toContainText('SECOND_BUSINESS_TEXT')
  await normalDetails(page)
  expect(fs.readFileSync(path.join(bots, 'first.json'), 'utf8')).toBe(first)
  expect(fs.readFileSync(path.join(bots, 'second.json'), 'utf8')).toBe(second)
  expect(server!.herdrCalls()).toEqual([])
})

// An invalid file still explicitly declares the shared identity. Treating only
// valid candidates as declarations would wrongly select the other business text.
test('P21 invalid duplicate declaration cannot make another file an unambiguous winner', async ({ page }) => {
  const { bots } = await prepare()
  fs.writeFileSync(path.join(bots, 'first.json'), JSON.stringify({ ...normal, id: 'problem', name: '不能选中的合法说明' }))
  fs.writeFileSync(path.join(bots, 'invalid.json'), JSON.stringify({ ...normal, id: 'problem', implementation: {} }))
  await openPlan(page)
  await completeGraph(page)
  const response = await page.request.get(`${server!.baseURL}/api/scheduler/bots`)
  expect(response.status()).toBe(200)
  expect(await response.json()).toMatchObject({ bots: [normal], problems: [
    { code: 'bot-id-conflict', bot_id: 'problem', files: ['first.json', 'invalid.json'] },
    { code: 'bot-invalid', bot_id: 'problem', files: ['invalid.json'] },
  ] })
  await page.getByRole('region', { name: '节点列表', exact: true }).getByRole('button', { name: '问题工作 A', exact: true }).click()
  const node = page.getByRole('region', { name: 'node 详情', exact: true })
  await expect(node.getByRole('alert')).toContainText('冲突')
  await node.getByRole('button', { name: 'Bot 明细', exact: true }).click()
  await expect(page.getByRole('region', { name: 'Bot 预览', exact: true })).not.toContainText('不能选中的合法说明')
  await normalDetails(page)
})

// P22 S1/S2: each independently specified malformed field is a real file.
// Existing strict serde behavior is retained, not replaced with an internal
// validation seam. Missing/defaulting, null coercion, permissive structures or
// fallback-to-chat would expose a bad capability instead of a local diagnostic.
const requiredFields = ['id', 'name', 'when', 'plan_guide', 'node_guide'] as const
const invalidDefinitions: { name: string; value: Record<string, unknown>; diagnostic: string }[] = []
for (const field of requiredFields) {
  const base: Record<string, unknown> = { ...normal, id: 'problem' }
  const omitted = { ...base }
  delete omitted[field]
  invalidDefinitions.push({ name: `missing ${field}`, value: omitted, diagnostic: `missing field \`${field}\`` })
  invalidDefinitions.push({ name: `null ${field}`, value: { ...base, [field]: null }, diagnostic: 'invalid type' })
  invalidDefinitions.push({ name: `wrong type ${field}`, value: { ...base, [field]: 7 }, diagnostic: 'invalid type' })
}
invalidDefinitions.push(
  { name: 'unknown root field', value: { ...normal, id: 'problem', useWhen: 'typo' }, diagnostic: 'unknown field `useWhen`' },
  { name: 'legacy version one', value: { ...normal, id: 'problem', schema_version: 1 }, diagnostic: 'unknown field `schema_version`' },
  { name: 'unsupported version', value: { ...normal, id: 'problem', schema_version: 2 }, diagnostic: 'unknown field `schema_version`' },
  { name: 'fractional version', value: { ...normal, id: 'problem', schema_version: 1.5 }, diagnostic: 'unknown field `schema_version`' },
  { name: 'legacy implementation', value: { ...normal, id: 'problem', implementation: { type: 'chat' } }, diagnostic: 'unknown field `implementation`' },
  { name: 'legacy inputs', value: { ...normal, id: 'problem', inputs: {} }, diagnostic: 'unknown field `inputs`' },
  { name: 'legacy outputs', value: { ...normal, id: 'problem', outputs: {} }, diagnostic: 'unknown field `outputs`' },
  { name: 'legacy description', value: { ...normal, id: 'problem', description: '' }, diagnostic: 'unknown field `description`' },
  { name: 'legacy use_when', value: { ...normal, id: 'problem', use_when: '' }, diagnostic: 'unknown field `use_when`' },
  { name: 'nontext Task reference', value: { ...normal, id: 'problem', task_id: {} }, diagnostic: 'expected a string' },
)
for (const example of invalidDefinitions) {
  test(`P20 P22 P29 bot structure rejects ${example.name} without hiding normal bots`, async ({ page }) => {
    const { bots } = await prepare()
    const filename = path.join(bots, 'invalid.json')
    const bytes = JSON.stringify(example.value)
    fs.writeFileSync(filename, bytes)
    await openPlan(page)
    await completeGraph(page)
    const response = await page.request.get(`${server!.baseURL}/api/scheduler/bots`)
    expect(response.status()).toBe(200)
    const catalog = await response.json()
    expect(catalog.bots).toEqual([normal])
    expect(catalog.problems).toEqual([expect.objectContaining({ code: 'bot-invalid', files: ['invalid.json'], message: expect.stringContaining(example.diagnostic) })])
    const issues = page.getByRole('region', { name: 'Bot 问题', exact: true })
    await expect(issues).toContainText('invalid.json')
    await expect(issues).toContainText(example.diagnostic)
    await page.getByRole('region', { name: '节点列表', exact: true }).getByRole('button', { name: '问题工作 A', exact: true }).click()
    const node = page.getByRole('region', { name: 'node 详情', exact: true })
    await expect(node.getByRole('alert')).toContainText('problem')
    await node.getByRole('button', { name: 'Bot 明细', exact: true }).click()
    const details = page.getByRole('region', { name: 'Bot 预览', exact: true })
    await expect(details.locator('h4, dl')).toHaveCount(0)
    await expect(details.getByText('chat', { exact: true })).toHaveCount(0)
    await normalDetails(page)
    expect(fs.readFileSync(filename, 'utf8')).toBe(bytes)
    expect(server!.herdrCalls()).toEqual([])
  })
}

// Explicit empty text/maps are permitted by the approved format, unlike omitted
// keys or null. No unapproved business-text length or name quota is added.
test('P22 explicit empty descriptions named objects and long text are valid rather than omitted', async ({ page }) => {
  const { bots } = await prepare()
  const empty = { id: 'problem', name: '', when: '', plan_guide: '', node_guide: '' }
  const longText = '完整能力文本'.repeat(2500)
  const named = { ...empty, id: 'named', plan_guide: longText, node_guide: '# 业务指引\n不是校验规则' }
  fs.writeFileSync(path.join(bots, 'empty.json'), JSON.stringify(empty))
  fs.writeFileSync(path.join(bots, 'named.json'), JSON.stringify(named))
  await openPlan(page)
  const graph = await completeGraph(page)
  const response = await page.request.get(`${server!.baseURL}/api/scheduler/bots`)
  expect(response.status()).toBe(200)
  expect(await response.json()).toEqual({ bots: [empty, named, normal] })
  await expect(page.getByRole('region', { name: 'Bot 问题', exact: true })).toHaveCount(0)
  await expect(graph.getByRole('button', { name: '问题工作 A', exact: true })).not.toContainText('缺失')
  await graph.getByRole('button', { name: '问题工作 A', exact: true }).click()
  const node = page.getByRole('region', { name: 'node 详情', exact: true })
  await expect(node.getByRole('alert')).toHaveCount(0)
  await node.getByRole('button', { name: 'Bot 明细', exact: true }).click()
  const detail = page.getByRole('region', { name: 'Bot 预览', exact: true })
  for (const title of ['节点指引', '规划指引', '选用说明']) {
    await detail.getByRole('button', { name: title, exact: true }).click()
    await expect(detail.getByRole('region', { name: title, exact: true })).toBeEmpty()
  }
})

// Approved S1, P19: fixed A -> B plus isolated C must survive a missing id.
// A renderer that silently drops A or only reports a problem after expansion
// violates the independently specified graph or the three visible locations.
test('P5 P35 missing bot ID stays distinct from an empty directory and preserves all definitions', async ({ page }) => {
  const { bots, maps } = await prepare()
  const mapBefore = fs.readFileSync(path.join(maps, 'plan.json'), 'utf8')
  const botBefore = fs.readFileSync(path.join(bots, 'normal.json'), 'utf8')
  await openPlan(page)
  await expect(page.getByRole('group', { name: 'Map 依赖图', exact: true })).toHaveCount(0)
  await expect(page.getByRole('region', { name: '节点列表', exact: true }).getByRole('button', { name: '问题工作 A', exact: true })).toContainText('problem')
  await expect(page.getByRole('region', { name: 'Bot 问题', exact: true })).toContainText('problem')
  const graph = await completeGraph(page)
  await expect(page.getByRole('region', { name: 'Bot 问题', exact: true })).toContainText('缺失共享 Bot：problem')
  await graph.getByRole('button', { name: '问题工作 A', exact: true }).click()
  const node = page.getByRole('region', { name: 'node 详情', exact: true })
  await expect(node.getByRole('alert')).toHaveText('缺失共享 Bot：problem')
  await expect(node.getByTestId('node-input')).toHaveText('{"target":"原始参数"}')
  await node.getByRole('button', { name: 'Bot 明细', exact: true }).click()
  await expect(page.getByRole('region', { name: 'Bot 预览', exact: true })).toHaveText('缺失共享 Bot：problem')
  const catalog = await page.request.get(`${server!.baseURL}/api/scheduler/bots`)
  expect(catalog.status()).toBe(200)
  expect(await catalog.json()).toEqual({ bots: [normal] })
  const mapResponse = await page.request.get(`${server!.baseURL}/api/scheduler/maps/plan.json`)
  expect(mapResponse.status()).toBe(200)
  expect(await mapResponse.json()).toEqual(plan)
  await expect(page.getByTestId('scheduler-bot-empty')).toHaveCount(0)
  await expect(page.getByRole('region', { name: '共享 Bot 列表', exact: true }).getByRole('button', { name: '正常能力说明 · normal', exact: true })).toBeVisible()
  await normalDetails(page)
  expect(server!.herdrCalls()).toEqual([])
  expect(fs.readFileSync(path.join(maps, 'plan.json'), 'utf8')).toBe(mapBefore)
  expect(fs.readFileSync(path.join(bots, 'normal.json'), 'utf8')).toBe(botBefore)
})
