import fs from 'node:fs'
import path from 'node:path'
import { execFileSync } from 'node:child_process'
import { expect, test, type Page } from '@playwright/test'
import { startServer, type E2EServer } from './support'

let server: E2EServer | null = null
test.afterEach(async () => { await server?.cleanup(); server = null })

const normal = { schema_version: 1, id: 'normal', title: '正常能力说明', use_when: '需要核对材料时',
  description: '仅整理差异，不执行工作', inputs: { source: '待核对的文本' }, outputs: { report: '差异报告说明' }, implementation: { type: 'chat' } }
const plan = { schema_version: 1, title: '模板问题隔离计划', description: '', nodes: {
  A: { title: '问题工作', node_template_id: 'problem', parameters: { target: '原始参数' }, blocked_by: [] },
  B: { title: '正常工作', node_template_id: 'normal', parameters: {}, blocked_by: ['A'] },
  C: { title: '独立工作', node_template_id: 'normal', parameters: {}, blocked_by: [] },
} }

async function prepare() {
  server = await startServer({ herdr: {} })
  const maps = path.join(server.cwd, '.data', 'workflow-maps')
  const templates = path.join(server.cwd, '.data', 'node-templates')
  fs.mkdirSync(maps, { recursive: true })
  fs.mkdirSync(templates, { recursive: true })
  fs.writeFileSync(path.join(maps, 'plan.json'), JSON.stringify(plan))
  fs.writeFileSync(path.join(templates, 'normal.json'), JSON.stringify(normal))
  return { templates, maps }
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
  await page.getByRole('region', { name: '节点列表', exact: true }).getByRole('button', { name: '正常工作 B', exact: true }).click()
  await page.getByRole('region', { name: 'node 详情', exact: true }).getByRole('button', { name: '模板明细', exact: true }).click()
  const details = page.getByRole('region', { name: '模板明细', exact: true })
  for (const text of ['正常能力说明', '需要核对材料时', '仅整理差异，不执行工作', '待核对的文本', '差异报告说明', 'chat']) await expect(details.getByText(text, { exact: true })).toBeVisible()
}

// S1 + production S2, P20/P29: a real bad file coexists with a normal
// capability. Aborting the whole catalog or returning empty success fails both
// the explicit file diagnostic and the normal node's independently named text.
test('P20 P29 malformed JSON is a local file problem and normal template remains available', async ({ page }) => {
  const { templates } = await prepare()
  const bad = path.join(templates, 'broken.json')
  fs.writeFileSync(bad, '{ broken JSON')
  await openPlan(page)
  await completeGraph(page)
  const response = await page.request.get(`${server!.baseURL}/api/scheduler/templates`)
  expect(response.status()).toBe(200)
  expect(await response.json()).toMatchObject({ templates: [normal], problems: [{ code: 'template-invalid', files: ['broken.json'], message: expect.stringContaining('结构无效') }] })
  await expect(page.getByRole('region', { name: '模板问题', exact: true })).toContainText('broken.json')
  await normalDetails(page)
  expect(fs.readFileSync(bad, 'utf8')).toBe('{ broken JSON')
  expect(server!.herdrCalls()).toEqual([])
})

// P20/P29 S1/S2: deny ReadData on one real file, not directory listing.
// A mocked 500 or directory-wide failure cannot establish local isolation.
test('P20 P29 an actual unreadable template file leaves the valid graph and normal capability usable', async ({ page }) => {
  const { templates } = await prepare()
  const unreadable = path.join(templates, 'unreadable.json')
  const bytes = JSON.stringify({ ...normal, id: 'problem', title: '不可读取的业务说明' })
  fs.writeFileSync(unreadable, bytes)
  const user = execFileSync('whoami.exe', [], { encoding: 'utf8' }).trim()
  try {
    execFileSync('icacls.exe', [unreadable, '/deny', `${user}:(RD)`])
    expect(() => fs.readFileSync(unreadable)).toThrow()
    expect(fs.readdirSync(templates)).toEqual(['normal.json', 'unreadable.json'])
    await openPlan(page)
    await completeGraph(page)
    const response = await page.request.get(`${server!.baseURL}/api/scheduler/templates`)
    expect(response.status()).toBe(200)
    expect(await response.json()).toMatchObject({ templates: [normal], problems: [{ code: 'template-read-failed', files: ['unreadable.json'], message: expect.stringContaining('读取失败') }] })
    await expect(page.getByRole('region', { name: '模板问题', exact: true })).toContainText('unreadable.json')
    await page.getByRole('region', { name: '节点列表', exact: true }).getByRole('button', { name: '问题工作 A', exact: true }).click()
    const node = page.getByRole('region', { name: 'node 详情', exact: true })
    await node.getByRole('button', { name: '模板明细', exact: true }).click()
    const detail = page.getByRole('region', { name: '模板明细', exact: true })
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
test('P21 duplicate identity lists every conflicting file and never selects either capability', async ({ page }) => {
  const { templates } = await prepare()
  const first = JSON.stringify({ ...normal, id: 'problem', title: '禁止任选第一说明', description: 'FIRST_BUSINESS_TEXT' })
  const second = JSON.stringify({ ...normal, id: 'problem', title: '禁止任选第二说明', description: 'SECOND_BUSINESS_TEXT' })
  fs.writeFileSync(path.join(templates, 'first.json'), first)
  fs.writeFileSync(path.join(templates, 'second.json'), second)
  await openPlan(page)
  const graph = await completeGraph(page)
  await expect(page.getByRole('region', { name: '节点列表', exact: true }).getByRole('button', { name: '问题工作 A', exact: true })).toContainText('冲突')
  const response = await page.request.get(`${server!.baseURL}/api/scheduler/templates`)
  expect(response.status()).toBe(200)
  expect(await response.json()).toMatchObject({ templates: [normal], problems: [{ code: 'template-id-conflict', template_id: 'problem', files: ['first.json', 'second.json'] }] })
  const issues = page.getByRole('region', { name: '模板问题', exact: true })
  for (const text of ['冲突', 'problem', 'first.json', 'second.json']) await expect(issues).toContainText(text)
  await graph.getByRole('button', { name: '问题工作 A', exact: true }).click()
  const node = page.getByRole('region', { name: 'node 详情', exact: true })
  await expect(node.getByRole('alert')).toContainText('冲突')
  await node.getByRole('button', { name: '模板明细', exact: true }).click()
  const detail = page.getByRole('region', { name: '模板明细', exact: true })
  for (const text of ['first.json', 'second.json', 'problem']) await expect(detail).toContainText(text)
  for (const text of ['FIRST_BUSINESS_TEXT', 'SECOND_BUSINESS_TEXT', '禁止任选第一说明', '禁止任选第二说明', 'chat']) await expect(detail).not.toContainText(text)
  await normalDetails(page)
  expect(fs.readFileSync(path.join(templates, 'first.json'), 'utf8')).toBe(first)
  expect(fs.readFileSync(path.join(templates, 'second.json'), 'utf8')).toBe(second)
  expect(server!.herdrCalls()).toEqual([])
})

// An invalid file still explicitly declares the shared identity. Treating only
// valid candidates as declarations would wrongly select the other business text.
test('P21 invalid duplicate declaration cannot make another file an unambiguous winner', async ({ page }) => {
  const { templates } = await prepare()
  fs.writeFileSync(path.join(templates, 'first.json'), JSON.stringify({ ...normal, id: 'problem', title: '不能选中的合法说明' }))
  fs.writeFileSync(path.join(templates, 'invalid.json'), JSON.stringify({ ...normal, id: 'problem', implementation: {} }))
  await openPlan(page)
  await completeGraph(page)
  const response = await page.request.get(`${server!.baseURL}/api/scheduler/templates`)
  expect(response.status()).toBe(200)
  expect(await response.json()).toMatchObject({ templates: [normal], problems: [
    { code: 'template-id-conflict', template_id: 'problem', files: ['first.json', 'invalid.json'] },
    { code: 'template-invalid', template_id: 'problem', files: ['invalid.json'] },
  ] })
  await page.getByRole('region', { name: '节点列表', exact: true }).getByRole('button', { name: '问题工作 A', exact: true }).click()
  const node = page.getByRole('region', { name: 'node 详情', exact: true })
  await expect(node.getByRole('alert')).toContainText('冲突')
  await node.getByRole('button', { name: '模板明细', exact: true }).click()
  await expect(page.getByRole('region', { name: '模板明细', exact: true })).not.toContainText('不能选中的合法说明')
  await normalDetails(page)
})

// P22 S1/S2: each independently specified malformed field is a real file.
// Existing strict serde behavior is retained, not replaced with an internal
// validation seam. Missing/defaulting, null coercion, permissive structures or
// fallback-to-chat would expose a bad capability instead of a local diagnostic.
const requiredFields = ['schema_version', 'id', 'title', 'use_when', 'description', 'inputs', 'outputs', 'implementation'] as const
const invalidDefinitions: { name: string; value: Record<string, unknown>; diagnostic: string }[] = []
for (const field of requiredFields) {
  const base: Record<string, unknown> = { ...normal, id: 'problem' }
  const omitted = { ...base }
  delete omitted[field]
  invalidDefinitions.push({ name: `missing ${field}`, value: omitted, diagnostic: `missing field \`${field}\`` })
  invalidDefinitions.push({ name: `null ${field}`, value: { ...base, [field]: null }, diagnostic: 'invalid type' })
  invalidDefinitions.push({ name: `wrong type ${field}`, value: { ...base, [field]: field === 'schema_version' ? '1' : 7 }, diagnostic: 'invalid type' })
}
invalidDefinitions.push(
  { name: 'unknown root field', value: { ...normal, id: 'problem', useWhen: 'typo' }, diagnostic: 'unknown field `useWhen`' },
  { name: 'unsupported version', value: { ...normal, id: 'problem', schema_version: 2 }, diagnostic: 'schema_version 必须为 1' },
  { name: 'fractional version', value: { ...normal, id: 'problem', schema_version: 1.5 }, diagnostic: 'invalid type' },
  { name: 'empty implementation object', value: { ...normal, id: 'problem', implementation: {} }, diagnostic: 'missing field `type`' },
  { name: 'unknown implementation field', value: { ...normal, id: 'problem', implementation: { type: 'chat', command: 'never-run' } }, diagnostic: 'unknown field `command`' },
  // The contract requires a structure diagnostic, not a specific enum-parser
  // phrase. Serde reports "expected value" for these two shapes; acceptance
  // remains rejection, zero fabricated capability, and normal-file isolation.
  { name: 'null implementation type', value: { ...normal, id: 'problem', implementation: { type: null } }, diagnostic: '结构无效' },
  { name: 'wrong implementation type shape', value: { ...normal, id: 'problem', implementation: { type: [] } }, diagnostic: '结构无效' },
  { name: 'unsupported implementation type', value: { ...normal, id: 'problem', implementation: { type: 'shell' } }, diagnostic: 'unknown variant `shell`' },
  { name: 'empty implementation type', value: { ...normal, id: 'problem', implementation: { type: '' } }, diagnostic: 'unknown variant' },
  { name: 'null named input description', value: { ...normal, id: 'problem', inputs: { source: null } }, diagnostic: 'expected a string' },
  { name: 'nontext named output description', value: { ...normal, id: 'problem', outputs: { report: {} } }, diagnostic: 'expected a string' },
  { name: 'array inputs', value: { ...normal, id: 'problem', inputs: [] }, diagnostic: 'expected a map' },
)
for (const example of invalidDefinitions) {
  test(`P20 P22 P29 template structure rejects ${example.name} without hiding normal templates`, async ({ page }) => {
    const { templates } = await prepare()
    const filename = path.join(templates, 'invalid.json')
    const bytes = JSON.stringify(example.value)
    fs.writeFileSync(filename, bytes)
    await openPlan(page)
    await completeGraph(page)
    const response = await page.request.get(`${server!.baseURL}/api/scheduler/templates`)
    expect(response.status()).toBe(200)
    const catalog = await response.json()
    expect(catalog.templates).toEqual([normal])
    expect(catalog.problems).toEqual([expect.objectContaining({ code: 'template-invalid', files: ['invalid.json'], message: expect.stringContaining(example.diagnostic) })])
    const issues = page.getByRole('region', { name: '模板问题', exact: true })
    await expect(issues).toContainText('invalid.json')
    await expect(issues).toContainText(example.diagnostic)
    await page.getByRole('region', { name: '节点列表', exact: true }).getByRole('button', { name: '问题工作 A', exact: true }).click()
    const node = page.getByRole('region', { name: 'node 详情', exact: true })
    await expect(node.getByRole('alert')).toContainText('problem')
    await node.getByRole('button', { name: '模板明细', exact: true }).click()
    const details = page.getByRole('region', { name: '模板明细', exact: true })
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
  const { templates } = await prepare()
  const empty = { schema_version: 1, id: 'problem', title: '', use_when: '', description: '', inputs: {}, outputs: {}, implementation: { type: 'chat' } }
  const longText = '完整能力文本'.repeat(2500)
  const named = { ...empty, id: 'named', description: longText, inputs: { '': '', undeclared_business_name: '' }, outputs: { result: '' } }
  fs.writeFileSync(path.join(templates, 'empty.json'), JSON.stringify(empty))
  fs.writeFileSync(path.join(templates, 'named.json'), JSON.stringify(named))
  await openPlan(page)
  const graph = await completeGraph(page)
  const response = await page.request.get(`${server!.baseURL}/api/scheduler/templates`)
  expect(response.status()).toBe(200)
  expect(await response.json()).toEqual({ templates: [empty, named, normal] })
  await expect(page.getByRole('region', { name: '模板问题', exact: true })).toHaveCount(0)
  await expect(graph.getByRole('button', { name: '问题工作 A', exact: true })).not.toContainText('缺失')
  await graph.getByRole('button', { name: '问题工作 A', exact: true }).click()
  const node = page.getByRole('region', { name: 'node 详情', exact: true })
  await expect(node.getByRole('alert')).toHaveCount(0)
  await node.getByRole('button', { name: '模板明细', exact: true }).click()
  const detail = page.getByRole('region', { name: '模板明细', exact: true })
  await expect(detail.getByText('chat', { exact: true })).toBeVisible()
  await expect(detail.getByRole('region', { name: '输入说明（inputs）', exact: true })).toContainText('无。')
  await expect(detail.getByRole('region', { name: '输出说明（outputs）', exact: true })).toContainText('无。')
})

// Approved S1, P19: fixed A -> B plus isolated C must survive a missing id.
// A renderer that silently drops A or only reports a problem after expansion
// violates the independently specified graph or the three visible locations.
test('P19 missing template ID is visible on node details and page without losing any graph structure', async ({ page }) => {
  const { templates, maps } = await prepare()
  const mapBefore = fs.readFileSync(path.join(maps, 'plan.json'), 'utf8')
  const templateBefore = fs.readFileSync(path.join(templates, 'normal.json'), 'utf8')
  await openPlan(page)
  await expect(page.getByRole('group', { name: 'Map 依赖图', exact: true })).toHaveCount(0)
  await expect(page.getByRole('region', { name: '节点列表', exact: true }).getByRole('button', { name: '问题工作 A', exact: true })).toContainText('problem')
  await expect(page.getByRole('region', { name: '模板问题', exact: true })).toContainText('problem')
  const graph = await completeGraph(page)
  await expect(page.getByRole('region', { name: '模板问题', exact: true })).toContainText('缺失共享模板：problem')
  await graph.getByRole('button', { name: '问题工作 A', exact: true }).click()
  const node = page.getByRole('region', { name: 'node 详情', exact: true })
  await expect(node.getByRole('alert')).toHaveText('缺失共享模板：problem')
  await expect(node.getByTestId('parameter-target')).toHaveText('原始参数')
  await node.getByRole('button', { name: '模板明细', exact: true }).click()
  await expect(page.getByRole('region', { name: '模板明细', exact: true })).toHaveText('缺失共享模板：problem')
  await normalDetails(page)
  expect(server!.herdrCalls()).toEqual([])
  expect(fs.readFileSync(path.join(maps, 'plan.json'), 'utf8')).toBe(mapBefore)
  expect(fs.readFileSync(path.join(templates, 'normal.json'), 'utf8')).toBe(templateBefore)
})
