import fs from 'node:fs'
import path from 'node:path'
import { expect, test } from '@playwright/test'
import { startServer, type E2EServer } from './support'

let server: E2EServer | null = null
test.afterEach(async () => { await server?.cleanup(); server = null })

// Approved seams S1 delivered scheduler, S2 HTTP, S4 definition files.
// External-call absence is observed only within each bounded test (90 s budget),
// from isolated server start through the last completed definition request.
// The Herdr recorder is real HTTP/process boundary evidence, not native/model proof.
// P1.01/P2.01/P3.01: literal contract fields, not derived from the application.
// Violation: treating one document as a prerequisite, merging their contents,
// or requiring Task binding before saving. Each tab is opened in reverse order.
test('P1.01 P2.01 P3.01 Bot documents round-trip independently without a Task binding', async ({ page }, testInfo) => {
  server = await startServer({ separateCwd: true, herdr: {} })
  await page.goto(`${server.baseURL}?layout=expanded`)
  await page.getByTestId('tab-scheduler').click()
  await page.getByRole('button', { name: 'Bot 管理', exact: true }).click()
  await page.getByRole('button', { name: '新增 Bot', exact: true }).click()
  const content = { name: '合同能力', when: 'WHEN_ONLY **何时选用**', plan_guide: 'PLAN_ONLY\n- 规划说明', node_guide: 'NODE_ONLY\n# 非校验规则：必须有业务输入' }
  await page.getByLabel('Bot ID', { exact: true }).fill('shared-contract')
  await page.getByLabel('Bot 内容 JSON', { exact: true }).fill(JSON.stringify(content))
  await page.getByRole('button', { name: '保存 Bot', exact: true }).click()
  await expect(page.getByRole('form', { name: 'Bot 编辑', exact: true })).toHaveCount(0)
  const preview = page.getByRole('region', { name: 'Bot 预览', exact: true })
  for (const [tab, text] of [['节点指引', content.node_guide], ['规划指引', content.plan_guide], ['选用说明', content.when]]) {
    await preview.getByRole('button', { name: tab, exact: true }).click()
    await expect(preview.getByRole('region', { name: tab, exact: true })).toHaveText(text)
  }
  await page.screenshot({ path: testInfo.outputPath('unbound-bot-documents.png'), fullPage: true })
  const catalog = await (await page.request.get(`${server.baseURL}/api/scheduler/bots`)).json()
  const evidence = testInfo.outputPath('saved-unbound-bot.json')
  fs.writeFileSync(evidence, JSON.stringify(catalog, null, 2))
  await testInfo.attach('saved-unbound-bot.json', { path: evidence, contentType: 'application/json' })
  expect(catalog).toEqual({ bots: [{ id: 'shared-contract', ...content }] })
  const root = path.join(server.cwd, '.data/bots')
  expect(fs.readdirSync(root)).toHaveLength(1)
  expect(JSON.parse(fs.readFileSync(path.join(root, fs.readdirSync(root)[0]), 'utf8'))).toEqual({ id: 'shared-contract', ...content })
  const edit = await (await page.request.post(`${server.baseURL}/api/scheduler/bots/edit`, { data: { id: 'shared-contract' } })).json()
  const empty = { name: '', when: '', plan_guide: '', node_guide: '' }
  const saved = await page.request.post(`${server.baseURL}/api/scheduler/bots/save`, { data: { id: edit.id, content: JSON.stringify(empty), revision: edit.revision } })
  expect(saved.ok()).toBe(true)
  expect(await saved.json()).toEqual({ id: 'shared-contract', ...empty })
  expect(await (await page.request.get(`${server.baseURL}/api/scheduler/bots`)).json()).toEqual({ bots: [{ id: 'shared-contract', ...empty }] })
  expect(server.herdrCalls()).toEqual([])
})

// P4.01/P3.01: old fields are unknown, not migration hints. All malformed
// saves are submitted against a real valid revision and must preserve its bytes.
test('P1.01 P2.01 P3.01 P4.01 Bot rejects malformed or legacy definitions without replacing the saved reference', async ({ request }) => {
  server = await startServer({ separateCwd: true, herdr: {} })
  const api = `${server.baseURL}/api/scheduler/bots`
  const content = { name: '引用而非副本', when: 'WHEN', plan_guide: 'PLAN', node_guide: 'NODE', task_id: 'task-does-not-exist' }
  const saved = await request.post(`${api}/save`, { data: { id: 'reference-only', content: JSON.stringify(content), revision: null } })
  expect(saved.ok()).toBe(true)
  expect(await saved.json()).toEqual({ id: 'reference-only', ...content })
  let edit = await (await request.post(`${api}/edit`, { data: { id: 'reference-only' } })).json()
  // Clearing any single document does not require clearing or reading another.
  for (const key of ['when', 'plan_guide', 'node_guide']) {
    const value = { ...content, [key]: '' }
    const result = await request.post(`${api}/save`, { data: { id: edit.id, content: JSON.stringify(value), revision: edit.revision } })
    expect(result.ok()).toBe(true)
    expect(await result.json()).toEqual({ id: 'reference-only', ...value })
    edit = await (await request.post(`${api}/edit`, { data: { id: edit.id } })).json()
    expect(edit.content).toEqual(value)
  }
  const root = path.join(server.cwd, '.data/bots')
  const file = path.join(root, fs.readdirSync(root)[0])
  const bytes = fs.readFileSync(file, 'utf8')
  const invalid = [
    ...['schema_version', 'use_when', 'description', 'inputs', 'outputs', 'implementation', 'chat_id', 'state', 'task'].map(key => ({ ...content, [key]: key === 'schema_version' ? 1 : {} })),
    ...['name', 'when', 'plan_guide', 'node_guide'].map(key => Object.fromEntries(Object.entries(content).filter(([field]) => field !== key))),
    ...['name', 'when', 'plan_guide', 'node_guide', 'task_id'].map(key => ({ ...content, [key]: 7 })),
    { ...content, typo: true }, [],
  ]
  for (const value of invalid) {
    const response = await request.post(`${api}/save`, { data: { id: edit.id, content: JSON.stringify(value), revision: edit.revision } })
    expect(response.ok(), JSON.stringify(value)).toBe(false)
    expect((await response.json()).message).toBeTruthy()
    expect(fs.readFileSync(file, 'utf8')).toBe(bytes)
    expect((await (await request.post(`${api}/edit`, { data: { id: edit.id } })).json()).content).toEqual(edit.content)
  }
  // A positional Bot array must not become a valid catalog entry either.
  fs.writeFileSync(path.join(root, 'array.json'), '["array-id","name","when","plan","node",null]')
  const catalog = await (await request.get(api)).json()
  expect(catalog.bots).toEqual([{ id: edit.id, ...edit.content }])
  expect(catalog.problems).toEqual([expect.objectContaining({ code: 'bot-invalid', files: ['array.json'] })])
  expect(server.herdrCalls()).toEqual([])
})

// P1.01/P3.01/P4.01: a literal A -> B map is retained while editing B.
// Violation: retaining parameters, auto-binding absent external resources, or
// committing a malformed node before validating the entire dependency graph.
test('P1.01 P3.01 P4.01 node input and zone save without external binding and invalid edits preserve the Map', async ({ page }, testInfo) => {
  server = await startServer({ separateCwd: true, herdr: {} })
  const root = path.join(server.cwd, '.data/workflow-maps')
  fs.mkdirSync(root, { recursive: true })
  const original = { title: '独立合同图', description: '定义不复制 Task 或 Chat', nodes: {
    A: { title: '准备', bot_id: 'not-created', input: '原文\n**并非 JSON 参数**', zone: 'Z:/not-registered', blocked_by: [] },
    B: { title: '核对', bot_id: 'not-created', input: '', zone: '', blocked_by: ['A'] },
  } }
  const file = path.join(root, 'contract.json')
  fs.writeFileSync(file, JSON.stringify(original))
  await page.goto(`${server.baseURL}?layout=expanded`)
  await page.getByTestId('tab-scheduler').click()
  await page.getByRole('button', { name: 'contract.json', exact: true }).click()
  await page.getByRole('region', { name: '节点列表', exact: true }).getByRole('button', { name: '核对 B', exact: true }).click()
  await page.getByRole('button', { name: '修改 node', exact: true }).click()
  const changed = { title: '核对新定义', bot_id: 'not-created', input: '# 独立工作输入\n不注入 Bot 文档', zone: 'C:/does-not-exist', blocked_by: ['A'] }
  await page.getByLabel('node 内容 JSON', { exact: true }).fill(JSON.stringify(changed))
  await page.getByRole('button', { name: '保存 node', exact: true }).click()
  await expect(page.getByRole('form', { name: 'node 编辑', exact: true })).toHaveCount(0)
  const expected = { ...original, nodes: { ...original.nodes, B: changed } }
  const api = `${server.baseURL}/api/scheduler/maps/contract.json`
  expect(await (await page.request.get(api)).json()).toEqual(expected)
  expect(JSON.parse(fs.readFileSync(file, 'utf8'))).toEqual(expected)
  const details = page.getByRole('region', { name: 'node 详情', exact: true })
  await expect(details.getByRole('region', { name: '本次输入', exact: true })).toHaveText('本次输入' + changed.input)
  await expect(details).toContainText('C:/does-not-exist')
  await expect(details.getByRole('alert')).toHaveText('缺失共享 Bot：not-created')
  const bytes = fs.readFileSync(file, 'utf8')
  const edit = await (await page.request.post(`${api}/nodes/edit`, { data: { id: 'B' } })).json()
  const invalid = [
    { ...changed, parameters: {} }, { ...changed, schema_version: 1 }, { ...changed, state: 'running' },
    { ...changed, typo: true }, { ...changed, input: {} }, { ...changed, zone: null },
    ...['title', 'bot_id', 'input', 'zone', 'blocked_by'].map(key => Object.fromEntries(Object.entries(changed).filter(([field]) => field !== key))),
    { ...changed, blocked_by: ['ghost'] }, { ...changed, blocked_by: ['B'] },
  ]
  for (const value of invalid) {
    const response = await page.request.post(`${api}/nodes/save`, { data: { id: 'B', content: JSON.stringify(value), revision: edit.revision, create: false } })
    expect(response.ok(), JSON.stringify(value)).toBe(false)
    expect((await response.json()).message).toBeTruthy()
    expect(fs.readFileSync(file, 'utf8')).toBe(bytes)
    expect(await (await page.request.get(api)).json()).toEqual(expected)
  }
  // Adding B as A's prerequisite closes A -> B -> A, not merely a self-edge.
  const cycle = await page.request.post(`${api}/nodes/save`, { data: { id: 'A', content: JSON.stringify({ ...original.nodes.A, blocked_by: ['B'] }), revision: edit.revision, create: false } })
  expect(cycle.ok()).toBe(false)
  expect((await cycle.json()).message).toContain('依赖环')
  expect(fs.readFileSync(file, 'utf8')).toBe(bytes)
  await page.getByRole('button', { name: '修改 node', exact: true }).click()
  const badDraft = JSON.stringify({ ...changed, blocked_by: ['ghost'] })
  await page.getByLabel('node 内容 JSON', { exact: true }).fill(badDraft)
  await page.getByRole('button', { name: '保存 node', exact: true }).click()
  await expect(page.getByRole('alert').first()).toContainText('ghost')
  await expect(page.getByLabel('node 内容 JSON', { exact: true })).toHaveValue(badDraft)
  expect(fs.readFileSync(file, 'utf8')).toBe(bytes)
  await page.screenshot({ path: testInfo.outputPath('invalid-dependency-preserves-map.png'), fullPage: true })
  const mapEvidence = testInfo.outputPath('preserved-map.json')
  const callEvidence = testInfo.outputPath('external-launch-calls.json')
  fs.writeFileSync(mapEvidence, fs.readFileSync(file))
  fs.writeFileSync(callEvidence, JSON.stringify(server.herdrCalls()))
  await testInfo.attach('preserved-map.json', { path: mapEvidence, contentType: 'application/json' })
  await testInfo.attach('external-launch-calls.json', { path: callEvidence, contentType: 'application/json' })
  expect(server.herdrCalls()).toEqual([])
})
