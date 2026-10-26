import fs from 'node:fs'
import path from 'node:path'
import { expect, test } from '@playwright/test'
import { startServer, type E2EServer } from './support'

let server: E2EServer | null = null
test.afterEach(async () => { await server?.cleanup(); server = null })

// Approved S1 + production S2. P22 requires identifying the bad node/field, not
// silently defaulting blocked_by. Independent counterexample: the only node is
// work and deliberately omits blocked_by. A generic error without node location
// is insufficient when the user has many nodes to repair.
test('P22 missing blocked_by identifies the Map file, node and required field', async ({ page }) => {
  server = await startServer()
  const maps = path.join(server.cwd, '.data', 'workflow-maps')
  fs.mkdirSync(maps, { recursive: true })
  const bytes = '{"schema_version":1,"title":"Missing dependency field","description":"","nodes":{"work":{"title":"Work","node_template_id":"shared","parameters":{}}}}'
  fs.writeFileSync(path.join(maps, 'missing-blocked-by.json'), bytes)
  const response = await page.request.get(`${server.baseURL}/api/scheduler/maps/missing-blocked-by.json`)
  expect(response.status()).toBe(500)
  const problem = await response.json()
  expect(problem.code).toBe('map-read-failed')
  expect(problem.message).toContain('missing-blocked-by.json')
  expect(problem.message).toContain('nodes.work')
  expect(problem.message).toContain('missing field `blocked_by`')
  await page.goto(server.baseURL)
  await page.getByTestId('tab-scheduler').click()
  await page.getByRole('button', { name: 'missing-blocked-by.json', exact: true }).click()
  await expect(page.getByRole('alert')).toContainText('nodes.work')
  await expect(page.getByRole('group', { name: 'Map 依赖图', exact: true })).toHaveCount(0)
  expect(fs.readFileSync(path.join(maps, 'missing-blocked-by.json'), 'utf8')).toBe(bytes)
})

// S2/P22: node is an object with required named fields, never a positional tuple.
// Serde's positional struct representation would accidentally accept this array.
test('P22 node arrays are not accepted as named node definitions', async ({ request }) => {
  server = await startServer()
  const maps = path.join(server.cwd, '.data', 'workflow-maps')
  fs.mkdirSync(maps, { recursive: true })
  fs.writeFileSync(path.join(maps, 'node-array.json'), '{"schema_version":1,"title":"Bad node","description":"","nodes":{"work":["Work","shared",{},[]]}}')
  const response = await request.get(`${server.baseURL}/api/scheduler/maps/node-array.json`)
  expect(response.status()).toBe(500)
  expect((await response.json()).message).toContain('nodes.work')
})

// S1/S2/P22: each row is a separate real file and independently written expected
// diagnostic. Missing, null, wrong type, misspelling and version are not aliases.
// This extends existing strict behavior rather than manufacturing a red state.
test('P22 Map and node required fields, types, unknown keys and versions are strict', async ({ page }) => {
  server = await startServer()
  const maps = path.join(server.cwd, '.data', 'workflow-maps')
  fs.mkdirSync(maps, { recursive: true })
  const valid = { schema_version: 1, title: '', description: '', nodes: {
    work: { title: '', node_template_id: '', parameters: {}, blocked_by: [] },
  } }
  const cases: { name: string; value: unknown; expected: string[] }[] = []
  for (const field of ['schema_version', 'title', 'description', 'nodes'] as const) {
    const value: Record<string, unknown> = { ...valid }
    delete value[field]
    cases.push({ name: `missing-map-${field}`, value, expected: [`missing field \`${field}\``] })
  }
  for (const field of ['title', 'node_template_id', 'parameters', 'blocked_by'] as const) {
    const work: Record<string, unknown> = { ...valid.nodes.work }
    delete work[field]
    cases.push({ name: `missing-node-${field}`, value: { ...valid, nodes: { work } }, expected: ['nodes.work', `missing field \`${field}\``] })
  }
  for (const [field, wrong, diagnostic] of [
    ['schema_version', '1', 'invalid type'], ['title', [], 'string'], ['description', false, 'string'], ['nodes', [], 'map'],
  ] as const) cases.push({ name: `type-map-${field}`, value: { ...valid, [field]: wrong }, expected: [field, diagnostic] })
  for (const [field, wrong, diagnostic] of [
    ['title', 7, 'string'], ['node_template_id', {}, 'string'], ['parameters', [], 'map'], ['blocked_by', 'work', 'sequence'],
  ] as const) cases.push({ name: `type-node-${field}`, value: { ...valid, nodes: { work: { ...valid.nodes.work, [field]: wrong } } }, expected: [`nodes.work.${field}`, diagnostic] })
  cases.push(
    { name: 'misspelled-blockedBy', value: { ...valid, nodes: { work: { title: '', node_template_id: '', parameters: {}, blockedBy: [] } } }, expected: ['nodes.work', 'unknown field `blockedBy`'] },
    { name: 'unknown-map-field', value: { ...valid, node_templates: {} }, expected: ['unknown field `node_templates`'] },
    { name: 'unknown-node-field', value: { ...valid, nodes: { work: { ...valid.nodes.work, typo: true } } }, expected: ['nodes.work', 'unknown field `typo`'] },
    { name: 'unknown-version', value: { ...valid, schema_version: 9 }, expected: ['schema_version', '1'] },
    { name: 'fractional-version', value: { ...valid, schema_version: 1.5 }, expected: ['schema_version', 'invalid type'] },
    { name: 'null-nodes', value: { ...valid, nodes: null }, expected: ['nodes', 'null'] },
    { name: 'null-parameters', value: { ...valid, nodes: { work: { ...valid.nodes.work, parameters: null } } }, expected: ['nodes.work.parameters', 'null'] },
    { name: 'null-blocked-by', value: { ...valid, nodes: { work: { ...valid.nodes.work, blocked_by: null } } }, expected: ['nodes.work.blocked_by', 'null'] },
    { name: 'null-title', value: { ...valid, title: null }, expected: ['title', 'null'] },
    { name: 'non-string-prerequisite', value: { ...valid, nodes: { work: { ...valid.nodes.work, blocked_by: [7] } } }, expected: ['nodes.work.blocked_by[0]', 'string'] },
    { name: 'root-array', value: [1, '', '', {}], expected: ['object'] },
    { name: 'empty-node-object', value: { ...valid, nodes: { work: {} } }, expected: ['nodes.work', 'missing field'] },
  )
  for (const item of cases) fs.writeFileSync(path.join(maps, `${item.name}.json`), JSON.stringify(item.value))
  await page.goto(server.baseURL)
  await page.getByTestId('tab-scheduler').click()
  for (const item of cases) {
    const response = await page.request.get(`${server.baseURL}/api/scheduler/maps/${item.name}.json`)
    expect(response.status(), item.name).toBe(500)
    const problem = await response.json()
    expect(problem.code).toBe('map-read-failed')
    expect(problem.message).toContain(`${item.name}.json`)
    for (const text of item.expected) expect(problem.message, item.name).toContain(text)
    await page.getByRole('button', { name: `${item.name}.json`, exact: true }).click()
    for (const text of item.expected) await expect(page.getByRole('alert')).toContainText(text)
    await expect(page.getByRole('group', { name: 'Map 依赖图', exact: true })).toHaveCount(0)
  }
})

// S1/S2/P23: literal counterexamples, not implementation-generated graphs. The
// unknown id, self edge and two-node cycle each have a distinct repair location.
// A reader dropping edges, or accepting the acyclic remainder, must fail.
test('P23 unknown prerequisite, self-dependency and cycle reject the entire graph with locations', async ({ page }) => {
  server = await startServer()
  const maps = path.join(server.cwd, '.data', 'workflow-maps')
  fs.mkdirSync(maps, { recursive: true })
  const cases = [
    { name: 'unknown.json', nodes: { A: { title: 'Unknown', node_template_id: 'shared', parameters: {}, blocked_by: ['ghost'] } }, expected: ['node A', 'blocked_by', '未知', 'ghost'], absent: '自身' },
    { name: 'self.json', nodes: { A: { title: 'Self', node_template_id: 'shared', parameters: {}, blocked_by: ['A'] } }, expected: ['node A', 'blocked_by', '自身'], absent: '未知' },
    { name: 'cycle.json', nodes: {
      A: { title: 'Cycle A', node_template_id: 'shared', parameters: {}, blocked_by: ['B'] },
      B: { title: 'Cycle B', node_template_id: 'shared', parameters: {}, blocked_by: ['A'] },
      independent: { title: 'Must not salvage this node', node_template_id: 'shared', parameters: {}, blocked_by: [] },
    }, expected: ['依赖环', 'A', 'B'], absent: '未知' },
  ]
  for (const item of cases) fs.writeFileSync(path.join(maps, item.name), JSON.stringify({ schema_version: 1, title: 'Invalid relationships', description: '', nodes: item.nodes }))
  await page.goto(server.baseURL)
  await page.getByTestId('tab-scheduler').click()
  for (const item of cases) {
    const response = await page.request.get(`${server.baseURL}/api/scheduler/maps/${item.name}`)
    expect(response.status()).toBe(500)
    const problem = await response.json()
    expect(problem.code).toBe('map-read-failed')
    for (const text of item.expected) expect(problem.message).toContain(text)
    expect(problem.message).not.toContain(item.absent)
    await page.getByRole('button', { name: item.name, exact: true }).click()
    for (const text of item.expected) await expect(page.getByRole('alert')).toContainText(text)
    await expect(page.getByRole('region', { name: 'Map 预览', exact: true })).toHaveCount(0)
    await expect(page.getByRole('group', { name: 'Map 依赖图', exact: true })).toHaveCount(0)
  }
})

// S1/S2/P22: explicit empty values have the specified type and are not omission.
// Free parameter keys (even structural-looking keys) and every JSON value kind
// must survive without template-input or real-resource validation.
test('P22 explicit empty objects arrays and text are legal and parameters stay open', async ({ page }) => {
  server = await startServer({ herdr: {} })
  const maps = path.join(server.cwd, '.data', 'workflow-maps')
  fs.mkdirSync(maps, { recursive: true })
  const originals = new Map([
    ['empty-map.json', '{"schema_version":1,"title":"","description":"","nodes":{}}'],
    ['empty-node.json', '{"schema_version":1,"title":"","description":"","nodes":{"E":{"title":"","node_template_id":"","parameters":{},"blocked_by":[]}}}'],
    ['open-parameters.json', '{"schema_version":1,"title":"Open parameters","description":"","nodes":{"P":{"title":"Parameters","node_template_id":"not-a-real-template","parameters":{"blockedBy":[],"schema_version":99,"emptyObject":{},"text":"","nothing":null,"flag":false,"count":7,"nested":{"arbitrary-key":["raw",true,null]},"zone":"not-a-real-zone","Bot":"not-a-real-Bot"},"blocked_by":[]}}}'],
  ])
  for (const [name, bytes] of originals) fs.writeFileSync(path.join(maps, name), bytes)
  const callsBefore = server.herdrCalls(), tabsBefore = server.herdrTabs()
  await page.goto(server.baseURL)
  await page.getByTestId('tab-scheduler').click()
  for (const name of ['empty-map.json', 'empty-node.json', 'open-parameters.json']) {
    const response = await page.request.get(`${server.baseURL}/api/scheduler/maps/${name}`)
    expect(response.status(), name).toBe(200)
    await page.getByRole('button', { name, exact: true }).click()
    await expect(page.getByRole('region', { name: 'Map 预览', exact: true })).toBeVisible()
    const graph = page.getByRole('group', { name: 'Map 依赖图', exact: true })
    await expect(graph.getByRole('button')).toHaveCount(name === 'empty-map.json' ? 0 : 1)
    if (name === 'empty-node.json') {
      await graph.getByRole('button').click()
      const details = page.getByRole('region', { name: 'node 详情', exact: true })
      await expect(details).toContainText('没有参数。')
      await expect(details).toContainText('无前置依赖。')
    }
    if (name === 'open-parameters.json') {
      await graph.getByRole('button', { name: 'Parameters P', exact: true }).click()
      const parameters = page.getByRole('region', { name: '本次参数', exact: true })
      await expect(parameters.locator('dt')).toHaveText(['Bot', 'blockedBy', 'count', 'emptyObject', 'flag', 'nested', 'nothing', 'schema_version', 'text', 'zone'])
      await expect(parameters.locator('dd')).toHaveText(['not-a-real-Bot', '[]', '7', '{}', 'false', '{\n  "arbitrary-key": [\n    "raw",\n    true,\n    null\n  ]\n}', 'null', '99', '', 'not-a-real-zone'])
    }
  }
  for (const [name, bytes] of originals) expect(fs.readFileSync(path.join(maps, name), 'utf8')).toBe(bytes)
  expect(server.herdrCalls()).toEqual(callsBefore)
  expect(server.herdrTabs()).toEqual(tabsBefore)
})

// S1/S2/P24/P29: A -> bad B -> C with real disk failures, not route mocks or a
// broken whole directory. Ticket04 is not in this baseline: the template region
// absence below is a guard, NOT evidence of clearing an opened template detail.
// Integrated acceptance must open ticket04's 模板明细 before selecting bad B.
// Bad JSON, trailing JSON, bad structure, bad dependency
// and a file removed after listing are separate scenarios. Each retains a valid
// neighbor and independently named node/parameter text to expose stale results.
for (const failure of ['json', 'trailing-json', 'structure', 'dependency', 'file-load'] as const) {
  test(`P24 P29 ${failure}: clears the previous graph and details and opens a healthy neighbor`, async ({ page }, testInfo) => {
    server = await startServer({ herdr: {} })
    const maps = path.join(server.cwd, '.data', 'workflow-maps')
    const templates = path.join(server.cwd, '.data', 'node-templates')
    fs.mkdirSync(maps, { recursive: true })
    fs.mkdirSync(templates, { recursive: true })
    const templateBytes = '{"schema_version":1,"id":"shared","title":"Shared recovery capability","use_when":"Inspect plans","description":"Read-only recovery evidence","inputs":{},"outputs":{},"implementation":{"type":"chat"}}'
    fs.writeFileSync(path.join(templates, 'capability.json'), templateBytes)
    const operationScript = path.join(server.cwd, 'record-operation.bat')
    const operationMarker = path.join(server.cwd, 'operation-executed.txt')
    fs.writeFileSync(operationScript, '@echo off\r\necho unexpected execution>>"%~dp0operation-executed.txt"\r\nexit /b 0\r\n')
    const config = fs.readFileSync(server.configPath, 'utf8')
    fs.writeFileSync(server.configPath, config.replace(/(\[operations\.(?:start|stop)\]\r?\nscript = )'[^']+'/g, `$1'${operationScript}'`))
    await server.restart()
    const a = '{"schema_version":1,"title":"Healthy A","description":"First result","nodes":{"A":{"title":"Only A","node_template_id":"shared","parameters":{"target":"alpha-only"},"blocked_by":[]}}}'
    const c = '{"schema_version":1,"title":"Healthy C","description":"Recovery result","nodes":{"C":{"title":"Only C","node_template_id":"shared","parameters":{"target":"charlie-only"},"blocked_by":[]}}}'
    const badInputs = {
      json: '{"schema_version":1,"title":',
      'trailing-json': `${a} {"extra":"not allowed"}`,
      structure: '{"schema_version":1,"title":"Invalid B","description":"","nodes":{"B":{"title":"Bad B","node_template_id":"shared","parameters":{},"blockedBy":[]}}}',
      dependency: '{"schema_version":1,"title":"Invalid B","description":"","nodes":{"B":{"title":"Bad B","node_template_id":"shared","parameters":{},"blocked_by":["ghost"]}}}',
      'file-load': a,
    }
    const expected = { json: 'EOF', 'trailing-json': 'trailing', structure: 'blockedBy', dependency: 'ghost', 'file-load': 'Map 文件读取失败' }
    const originals = { 'a.json': a, 'b.json': badInputs[failure], 'c.json': c }
    for (const [name, bytes] of Object.entries(originals)) fs.writeFileSync(path.join(maps, name), bytes)
    const callsBefore = server.herdrCalls(), tabsBefore = server.herdrTabs()
    await page.goto(server.baseURL)
    await page.getByTestId('tab-scheduler').click()
    const list = page.getByTestId('scheduler-map-list')
    await expect(list.getByRole('listitem')).toHaveText(['a.json', 'b.json', 'c.json'])
    await list.getByRole('button', { name: 'a.json', exact: true }).click()
    const graph = page.getByRole('group', { name: 'Map 依赖图', exact: true })
    const details = page.getByRole('region', { name: 'node 详情', exact: true })
    await graph.getByRole('button', { name: 'Only A A', exact: true }).click()
    await expect(details).toContainText('alpha-only')
    if (failure === 'file-load') fs.unlinkSync(path.join(maps, 'b.json'))
    const response = await page.request.get(`${server.baseURL}/api/scheduler/maps/b.json`)
    expect(response.status()).toBe(500)
    expect((await response.json()).message).toContain(expected[failure])
    await list.getByRole('button', { name: 'b.json', exact: true }).click()
    await expect(page.getByRole('alert')).toContainText(expected[failure])
    await expect(page.getByRole('alert')).toContainText('b.json')
    await expect(graph).toHaveCount(0)
    await expect(details).toHaveCount(0)
    await expect(page.getByRole('region', { name: '模板明细', exact: true })).toHaveCount(0)
    await expect(page.getByRole('region', { name: 'Map 预览', exact: true })).toHaveCount(0)
    await expect(page.getByTestId('panel-scheduler')).not.toContainText('alpha-only')
    await expect(list.getByRole('button', { name: 'c.json', exact: true })).toBeEnabled()
    await page.screenshot({ path: testInfo.outputPath(`${failure}-cleared.png`), fullPage: true })
    await list.getByRole('button', { name: 'c.json', exact: true }).click()
    await graph.getByRole('button', { name: 'Only C C', exact: true }).click()
    await expect(graph.getByRole('button')).toHaveCount(1)
    await expect(details).toContainText('charlie-only')
    await expect(details).not.toContainText('alpha-only')
    await expect(page.getByRole('region', { name: 'Map 预览', exact: true })).toContainText('Healthy C')
    await expect(page.getByRole('alert')).toHaveCount(0)
    await page.screenshot({ path: testInfo.outputPath(`${failure}-recovered.png`), fullPage: true })
    for (const [name, bytes] of Object.entries(originals)) {
      if (failure === 'file-load' && name === 'b.json') expect(fs.existsSync(path.join(maps, name))).toBe(false)
      else expect(fs.readFileSync(path.join(maps, name), 'utf8')).toBe(bytes)
    }
    expect(fs.readFileSync(path.join(templates, 'capability.json'), 'utf8')).toBe(templateBytes)
    expect(server.herdrCalls()).toEqual(callsBefore)
    expect(server.herdrTabs()).toEqual(tabsBefore)
    expect(fs.existsSync(operationMarker)).toBe(false)
    expect(fs.existsSync(path.join(server.cwd, 'extension-executed'))).toBe(false)
  })
}
