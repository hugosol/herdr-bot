import fs from 'node:fs'
import path from 'node:path'
import { execFileSync } from 'node:child_process'
import { expect, test, type Page } from '@playwright/test'
import { startServer, type E2EServer } from './support'

let server: E2EServer | null = null
test.afterEach(async () => { await server?.cleanup(); server = null })
const audit = { title: '独立审计', bot_id: 'missing-audit-template', input: JSON.stringify({ id: 'business-id', extra: { list: [1, null, { approved: false }] }, zone: 'unregistered-zone', bot: 'unregistered-bot' }), zone: 'unregistered-zone', blocked_by: [] as string[] }
const original = { title: '保留 Map 标题', description: '保留 Map 说明', nodes: {
  B: { title: '保留的工作', bot_id: 'other', input: JSON.stringify({ nested: { order: ['keep', null, 7], enabled: true } }), zone: 'unregistered-zone', blocked_by: [] as string[] },
  C: { title: '保留的后继', bot_id: 'summary', input: JSON.stringify({ untouched: [{ count: 9 }, null] }), zone: 'unregistered-zone', blocked_by: ['B'] },
  D: { title: '保留的独立工作', bot_id: 'independent', input: JSON.stringify({}), zone: 'unregistered-zone', blocked_by: [] as string[] },
} }
const inventory = (root: string) => Object.fromEntries(fs.readdirSync(root).sort().map(name => [name, fs.readFileSync(path.join(root, name), 'utf8')]))
async function setup(page: Page, source: unknown = original) {
  server = await startServer({ herdr: {}, separateCwd: true })
  const root = path.join(server.cwd, '.data/workflow-maps')
  fs.mkdirSync(root, { recursive: true })
  fs.writeFileSync(path.join(root, 'work.JSON'), JSON.stringify(source))
  fs.writeFileSync(path.join(root, 'untouched.json'), '  ' + JSON.stringify(original) + '\n')
  await page.goto(`${server.baseURL}?layout=expanded`)
  await page.getByTestId('tab-scheduler').click()
  await page.getByRole('button', { name: 'work.JSON', exact: true }).click()
  await expect(page.getByRole('region', { name: 'Map 预览', exact: true })).toBeVisible()
  return root
}
async function readMap(page: Page, name = 'work.JSON') {
  const response = await page.request.get(`${server!.baseURL}/api/scheduler/maps/${name}`)
  expect(response.ok()).toBe(true)
  return response.json()
}
async function draft(page: Page, id: string | null = null, name = 'work.JSON') {
  const response = id === null
    ? await page.request.get(`${server!.baseURL}/api/scheduler/maps/${name}/nodes/new`)
    : await page.request.post(`${server!.baseURL}/api/scheduler/maps/${name}/nodes/edit`, { data: { id } })
  expect(response.ok()).toBe(true)
  return response.json()
}
async function save(page: Page, id: string, content: unknown, revision: unknown, create = true, name = 'work.JSON') {
  return page.request.post(`${server!.baseURL}/api/scheduler/maps/${name}/nodes/save`, { data: { id, content: typeof content === 'string' ? content : JSON.stringify(content), revision, create } })
}

// Approved seams: S1 delivered browser, S2 production HTTP. Independent example
// values are from the approved Shanghai A scenario. Plausible violations: eager
// installation on opening, ignored edits, or updating only details not the graph.
test('node creation starts with a disposable Shanghai draft and saves changed content into the current Map immediately', async ({ page }) => {
  const root = await setup(page), before = inventory(root)
  const bots = path.join(server!.cwd, '.data/bots'), botsBefore = inventory(bots)
  await page.getByRole('button', { name: '新增 node', exact: true }).click()
  const id = page.getByLabel('node ID', { exact: true }), json = page.getByLabel('node 内容 JSON', { exact: true })
  await expect(id).toHaveValue('')
  expect(JSON.parse(await json.inputValue())).toEqual({ title: '上海出行天气', bot_id: 'destination-weather', input: '城市：上海\n日期：2026-11-01\n偏好：步行；带伞。', zone: 'C:/example/travel-not-registered', blocked_by: [] })
  expect(inventory(root)).toEqual(before)
  expect(inventory(bots)).toEqual(botsBefore)
  await page.getByRole('button', { name: '放弃编辑', exact: true }).click()
  expect(inventory(root)).toEqual(before)
  expect(inventory(bots)).toEqual(botsBefore)
  await page.getByRole('button', { name: '新增 node', exact: true }).click()
  await id.fill('A/independent:id')
  await json.fill(JSON.stringify({ ...audit, blocked_by: ['B'] }))
  await page.getByRole('button', { name: '保存 node', exact: true }).click()
  await expect(page.getByRole('form', { name: 'node 编辑', exact: true })).toHaveCount(0)
  await expect(page.getByRole('button', { name: 'work.JSON', exact: true })).toHaveAttribute('aria-pressed', 'true')
  await expect(page.getByRole('region', { name: '节点列表', exact: true }).getByRole('button', { name: '独立审计 A/independent:id', exact: true })).toHaveAttribute('aria-pressed', 'true')
  await expect(page.getByRole('complementary', { name: '节点检查器', exact: true })).toContainText('business-id')
  await expect(page.getByRole('region', { name: 'node 详情', exact: true }).getByRole('alert')).toHaveText('缺失共享 Bot：missing-audit-template')
  await expect(page.getByRole('group', { name: 'Map 依赖图', exact: true })).toContainText('独立审计')
  await expect(page.getByRole('img', { name: 'B → A/independent:id', exact: true })).toHaveCount(1)
  expect(await readMap(page)).toEqual({ ...original, nodes: { ...original.nodes, 'A/independent:id': { ...audit, blocked_by: ['B'] } } })
  expect(inventory(root)['untouched.json']).toBe(before['untouched.json'])
  expect(inventory(bots)).toEqual(botsBefore)
  expect(server!.herdrCalls()).toEqual([])
})

// P6 P7 P10 P16 P26 P31: a real non-weather node must not inherit any
// starter field. A stale graph or accidental replacement of adjacent definitions
// disagrees with these independent HTTP and visible edge observations.
test('editing a real node keeps identity, open parameters and all non-target definitions while updating every current view', async ({ page }) => {
  const root = await setup(page, { ...original, nodes: { ...original.nodes, A: audit } }), before = inventory(root)
  await page.getByRole('region', { name: '节点列表', exact: true }).getByRole('button', { name: '独立审计 A', exact: true }).click()
  await page.getByRole('button', { name: '修改 node', exact: true }).click()
  const id = page.getByLabel('node ID', { exact: true }), json = page.getByLabel('node 内容 JSON', { exact: true })
  await expect(id).toHaveValue('A')
  await expect(id).toHaveAttribute('readonly', '')
  expect(JSON.parse(await json.inputValue())).toEqual(audit)
  const changed = { title: '审计新版本', bot_id: '', input: JSON.stringify({ id: 'edited-business-id', undeclared: { children: [null, true, { amount: 13 }] }, zone: 'unknown', bot: 'absent' }), zone: 'unregistered-zone', blocked_by: ['C'] }
  await json.fill(JSON.stringify(changed, null, 3))
  await page.getByRole('button', { name: '保存 node', exact: true }).click()
  await expect(page.getByRole('form', { name: 'node 编辑', exact: true })).toHaveCount(0)
  await expect(page.getByRole('button', { name: 'work.JSON', exact: true })).toHaveAttribute('aria-pressed', 'true')
  await expect(page.getByRole('region', { name: '节点列表', exact: true }).getByRole('button', { name: '审计新版本 A', exact: true })).toHaveAttribute('aria-pressed', 'true')
  await expect(page.getByRole('region', { name: 'node 详情', exact: true })).toContainText('edited-business-id')
  await expect(page.getByRole('group', { name: 'Map 依赖图', exact: true }).getByRole('button', { name: '审计新版本 A', exact: true })).toHaveAttribute('aria-pressed', 'true')
  await expect(page.getByRole('img', { name: 'C → A', exact: true })).toHaveCount(1)
  await expect(page.getByRole('img', { name: 'B → C', exact: true })).toHaveCount(1)
  expect(await readMap(page)).toEqual({ ...original, nodes: { ...original.nodes, A: changed } })
  expect(await draft(page, 'A')).toMatchObject({ id: 'A', content: changed, create: false })
  expect(inventory(root)['untouched.json']).toBe(before['untouched.json'])
  expect(server!.herdrCalls()).toEqual([])
})

// P16: a registered bot declares an input the node deliberately omits.
// A conditional inputs validator could pass the missing-bot cases above,
// but must fail this literal expectation. No parameter defaults may appear.
test('registered Bot Markdown guidance does not constrain, complete or validate node input', async ({ page }) => {
  const root = await setup(page), before = inventory(root)
  const bots = path.join(server!.cwd, '.data/bots')
  fs.writeFileSync(path.join(bots, 'known.json'), JSON.stringify({ id: 'declared-inputs', name: '已登记能力', when: '', plan_guide: '说明不是校验规则', node_guide: '# 必须填写城市\n这是工作指引，不是必填规则。' }))
  const botsBefore = inventory(bots)
  await page.getByRole('button', { name: '刷新全部', exact: true }).click()
  await page.getByRole('button', { name: '新增 node', exact: true }).click()
  const content = { ...audit, bot_id: 'declared-inputs', input: '' }
  await page.getByLabel('node ID', { exact: true }).fill('open-parameters')
  await page.getByLabel('node 内容 JSON', { exact: true }).fill(JSON.stringify(content))
  await page.getByRole('button', { name: '保存 node', exact: true }).click()
  await expect(page.getByRole('form', { name: 'node 编辑', exact: true })).toHaveCount(0)
  await expect(page.getByRole('region', { name: 'node 详情', exact: true }).getByRole('alert')).toHaveCount(0)
  expect(await readMap(page)).toEqual({ ...original, nodes: { ...original.nodes, 'open-parameters': content } })
  const edit = await draft(page, 'open-parameters')
  expect((await save(page, 'open-parameters', { ...content, input: JSON.stringify({}) }, edit.revision, false)).ok()).toBe(true)
  expect(await readMap(page)).toEqual({ ...original, nodes: { ...original.nodes, 'open-parameters': { ...content, input: JSON.stringify({}) } } })
  expect(inventory(root)['untouched.json']).toBe(before['untouched.json'])
  expect(inventory(bots)).toEqual(botsBefore)
})

// P7 P14 P17: independent malformed inputs must fail for both creation and
// modification, without a single definition byte changing. The page preserves
// raw whitespace rather than replacing a failed draft with parsed JSON.
test('strict node JSON rejects top-level identity, missing fields, wrong types and unknown fields without losing drafts', async ({ page }) => {
  const root = await setup(page, { ...original, nodes: { ...original.nodes, A: audit } }), before = inventory(root)
  const edit = await draft(page, 'A')
  await page.getByRole('region', { name: '节点列表', exact: true }).getByRole('button', { name: '独立审计 A', exact: true }).click()
  await page.getByRole('button', { name: '修改 node', exact: true }).click()
  const id = page.getByLabel('node ID', { exact: true }), json = page.getByLabel('node 内容 JSON', { exact: true })
  const cases = [
    { raw: '  { "title": ', diagnostic: 'JSON' },
    { raw: JSON.stringify({ ...audit, id: 'do-not-override' }), diagnostic: '移除' },
    { raw: JSON.stringify({ ...audit, typo: true }), diagnostic: 'typo' },
    { raw: JSON.stringify({ ...audit, implementation: { type: 'chat' } }), diagnostic: 'implementation' },
    { raw: '[]', diagnostic: '对象' },
    { raw: 'null', diagnostic: '对象' },
  ]
  for (const key of Object.keys(audit)) {
    const missing: Record<string, unknown> = { ...audit }; delete missing[key]
    cases.push({ raw: JSON.stringify(missing), diagnostic: key })
  }
  for (const [key, value] of [['title', null], ['title', 5], ['bot_id', []], ['input', []], ['input', null], ['zone', 7], ['zone', null], ['blocked_by', null], ['blocked_by', {}], ['blocked_by', [7]]] as const) {
    cases.push({ raw: JSON.stringify({ ...audit, [key]: value }), diagnostic: key })
  }
  for (const item of cases) {
    await json.fill(item.raw)
    await page.getByRole('button', { name: '保存 node', exact: true }).click()
    await expect(page.getByRole('alert').filter({ hasText: item.diagnostic }).first()).toBeVisible()
    await expect(id).toHaveValue('A')
    await expect(json).toHaveValue(item.raw)
    await expect(page.getByRole('form', { name: 'node 编辑', exact: true })).toBeVisible()
    for (const create of [true, false]) {
      const response = await save(page, create ? 'new-invalid' : 'A', item.raw, edit.revision, create)
      expect(response.ok(), item.raw).toBe(false)
      expect((await response.json()).message).toContain(item.diagnostic)
      expect(inventory(root)).toEqual(before)
    }
  }
  const validEmpty = { title: '', bot_id: '', input: JSON.stringify({}), zone: 'unregistered-zone', blocked_by: [] }
  await json.fill(JSON.stringify(validEmpty))
  await page.getByRole('button', { name: '保存 node', exact: true }).click()
  await expect(page.getByRole('form', { name: 'node 编辑', exact: true })).toHaveCount(0)
  expect(await readMap(page)).toEqual({ ...original, nodes: { ...original.nodes, A: validEmpty } })
})

// P13 P17: overlapping production requests share one actual Map version. A
// check-then-write race would overwrite a winner; global-ID enforcement would
// reject the same ID in a second Map. Neither is an acceptable substitute.
test('same-Map duplicates and concurrent creates are rejected without overwrite while another Map can reuse the ID', async ({ page }) => {
  const root = await setup(page), before = inventory(root)
  await page.getByRole('button', { name: '新增 node', exact: true }).click()
  const raw = '  ' + JSON.stringify(audit, null, 4) + '\n'
  await page.getByLabel('node 内容 JSON', { exact: true }).fill(raw)
  const initial = await draft(page)
  await page.getByRole('button', { name: '保存 node', exact: true }).click()
  await expect(page.getByRole('alert').filter({ hasText: 'ID 不能为空' })).toBeVisible()
  expect((await save(page, '', raw, initial.revision)).ok()).toBe(false)
  await page.getByLabel('node ID', { exact: true }).fill('B')
  await page.getByRole('button', { name: '保存 node', exact: true }).click()
  await expect(page.getByRole('alert').filter({ hasText: 'ID 已存在' })).toBeVisible()
  await expect(page.getByLabel('node ID', { exact: true })).toHaveValue('B')
  await expect(page.getByLabel('node 内容 JSON', { exact: true })).toHaveValue(raw)
  const duplicate = await save(page, 'B', audit, initial.revision)
  expect(duplicate.ok()).toBe(false)
  expect((await duplicate.json()).message).toContain('ID 已存在')
  expect(inventory(root)).toEqual(before)
  page.once('dialog', dialog => dialog.accept())
  await page.getByRole('button', { name: '放弃编辑', exact: true }).click()
  const contenders = [{ ...audit, title: 'first contender' }, { ...audit, title: 'second contender' }]
  const responses = await Promise.all(contenders.map(content => save(page, 'one-local-id', content, initial.revision)))
  expect(responses.filter(response => response.ok())).toHaveLength(1)
  expect(responses.filter(response => !response.ok())).toHaveLength(1)
  expect((await responses.find(response => !response.ok())!.json()).message).toContain('重新加载')
  const expected = contenders[responses[0].ok() ? 0 : 1]
  expect(await readMap(page)).toEqual({ ...original, nodes: { ...original.nodes, 'one-local-id': expected } })
  expect(inventory(root)['untouched.json']).toBe(before['untouched.json'])
  const other = await draft(page, null, 'untouched.json')
  expect((await save(page, 'one-local-id', audit, other.revision, true, 'untouched.json')).ok()).toBe(true)
  expect(await readMap(page, 'untouched.json')).toEqual({ ...original, nodes: { ...original.nodes, 'one-local-id': audit } })
  expect(await readMap(page)).toEqual({ ...original, nodes: { ...original.nodes, 'one-local-id': expected } })
})

// P14 P15 S2: reject the full candidate Map, not just the submitted node.
// B -> C already exists; adding C -> B creates a real multi-node cycle.
// Successful A/B parallel, C join and D independent have prescribed edges.
test('node saves preserve Map schema rules and reject unknown, self and cyclic dependencies without repairing edges', async ({ page }) => {
  const root = await setup(page), before = inventory(root)
  let edit = await draft(page)
  for (const [content, diagnostic] of [[{ ...audit, blocked_by: ['missing'] }, '未知 node'], [{ ...audit, blocked_by: ['A'] }, '自身']] as const) {
    const response = await save(page, 'A', content, edit.revision)
    expect(response.ok()).toBe(false)
    expect((await response.json()).message).toContain(diagnostic)
    expect(inventory(root)).toEqual(before)
  }
  const cycle = await save(page, 'B', { ...original.nodes.B, blocked_by: ['C'] }, edit.revision, false)
  expect(cycle.ok()).toBe(false)
  expect((await cycle.json()).message).toContain('依赖环')
  expect(inventory(root)).toEqual(before)
  expect((await save(page, 'A', audit, edit.revision)).ok()).toBe(true)
  edit = await draft(page, 'C')
  const joined = { ...original.nodes.C, blocked_by: ['A', 'B'] }
  expect((await save(page, 'C', joined, edit.revision, false)).ok()).toBe(true)
  expect(await readMap(page)).toEqual({ ...original, nodes: { ...original.nodes, A: audit, C: joined } })
  expect((await readMap(page)).nodes).toMatchObject({ A: { blocked_by: [] }, B: { blocked_by: [] }, C: { blocked_by: ['A', 'B'] }, D: { blocked_by: [] } })
  expect(inventory(root)['untouched.json']).toBe(before['untouched.json'])
  const valid = fs.readFileSync(path.join(root, 'work.JSON'), 'utf8')
  const invalidMaps: { value: unknown; diagnostic: string }[] = [
    { value: { ...original, schema_version: 2 }, diagnostic: 'schema_version' },
    { value: { ...original, schema_version: 1.5 }, diagnostic: 'schema_version' },
    { value: { ...original, schema_version: '1' }, diagnostic: 'schema_version' },
    { value: { ...original, unknown_map_field: true }, diagnostic: 'unknown_map_field' },
    { value: { ...original, title: null }, diagnostic: 'title' },
    { value: { ...original, description: [] }, diagnostic: 'description' },
    { value: { ...original, nodes: [] }, diagnostic: 'nodes' },
    { value: { ...original, nodes: { B: ['title', 'bot', {}, []] } }, diagnostic: 'B' },
    { value: { ...original, nodes: { ...original.nodes, D: { ...original.nodes.D, input: [] } } }, diagnostic: 'input' },
  ]
  for (const key of Object.keys(original)) {
    const missing: Record<string, unknown> = { ...original }; delete missing[key]
    invalidMaps.push({ value: missing, diagnostic: key })
  }
  for (const item of invalidMaps) {
    const raw = JSON.stringify(item.value)
    fs.writeFileSync(path.join(root, 'work.JSON'), raw)
    const invalidBefore = inventory(root)
    const response = await save(page, 'new-node', audit, { file: 'work.JSON', original: raw })
    expect(response.ok()).toBe(false)
    expect((await response.json()).message).toContain(item.diagnostic)
    expect(inventory(root)).toEqual(invalidBefore)
  }
  fs.writeFileSync(path.join(root, 'work.JSON'), valid)
})

// P17 P18: editing A must conflict when a real second page edits B. Also
// exercise external replacement/removal immediately before submit. Comparing
// the whole newer inventory catches overwrites and accidental file recreation.
test('another node or external editor changing the whole Map rejects old drafts without merging or overwriting', async ({ page, context }) => {
  const source = { ...original, nodes: { ...original.nodes, A: audit } }
  const root = await setup(page, source), file = path.join(root, 'work.JSON')
  const raw = '  ' + JSON.stringify({ ...audit, title: 'unsaved first-client A' }, null, 4) + '\n'
  for (const mode of ['other-node-client', 'external-change', 'external-remove']) {
    fs.writeFileSync(file, JSON.stringify(source))
    await page.getByRole('button', { name: '刷新全部', exact: true }).click()
    await page.getByRole('region', { name: '节点列表', exact: true }).getByRole('button', { name: '独立审计 A', exact: true }).click()
    await page.getByRole('button', { name: '修改 node', exact: true }).click()
    const stale = await draft(page, 'A')
    await page.getByLabel('node 内容 JSON', { exact: true }).fill(raw)
    if (mode === 'other-node-client') {
      const other = await context.newPage()
      await other.goto(`${server!.baseURL}?layout=expanded`)
      await other.getByTestId('tab-scheduler').click()
      await other.getByRole('button', { name: 'work.JSON', exact: true }).click()
      await other.getByRole('region', { name: '节点列表', exact: true }).getByRole('button', { name: '保留的工作 B', exact: true }).click()
      await other.getByRole('button', { name: '修改 node', exact: true }).click()
      await other.getByLabel('node 内容 JSON', { exact: true }).fill(JSON.stringify({ ...original.nodes.B, title: 'second-client B' }))
      await other.getByRole('button', { name: '保存 node', exact: true }).click()
      await expect(other.getByRole('form', { name: 'node 编辑', exact: true })).toHaveCount(0)
      expect(await readMap(page)).toEqual({ ...source, nodes: { ...source.nodes, B: { ...original.nodes.B, title: 'second-client B' } } })
      await other.close()
    } else if (mode === 'external-change') {
      fs.writeFileSync(file, JSON.stringify({ ...source, nodes: { ...source.nodes, B: { ...original.nodes.B, input: JSON.stringify({ external: [null, 19] }) } } }) + '\n')
    } else fs.unlinkSync(file)
    const newer = inventory(root)
    await page.getByRole('button', { name: '保存 node', exact: true }).click()
    await expect(page.getByRole('alert').filter({ hasText: '重新加载' })).toBeVisible()
    await expect(page.getByLabel('node ID', { exact: true })).toHaveValue('A')
    await expect(page.getByLabel('node 内容 JSON', { exact: true })).toHaveValue(raw)
    await expect(page.getByRole('form', { name: 'node 编辑', exact: true })).toBeVisible()
    for (const create of [false, true]) {
      const response = await save(page, create ? 'new-stale-node' : 'A', raw, stale.revision, create)
      expect(response.ok()).toBe(false)
      expect((await response.json()).message).toContain('重新加载')
      expect(inventory(root)).toEqual(newer)
    }
    page.once('dialog', dialog => dialog.accept())
    await page.getByRole('button', { name: '放弃编辑', exact: true }).click()
  }
})

// P17 S1/S2: deny actual file creation in the owned directory, leaving the
// real parser, conflict checks and persistence path exercised. Both create and
// edit retain exact raw drafts and succeed only after the denial is removed.
test('filesystem write failures keep node identity and raw JSON without changing definitions or reporting success', async ({ page }) => {
  const root = await setup(page)
  const user = execFileSync('whoami.exe', [], { encoding: 'utf8' }).trim()
  for (const create of [true, false]) {
    const before = inventory(root), current = await draft(page, create ? null : 'write-denied')
    await page.getByRole('button', { name: create ? '新增 node' : '修改 node', exact: true }).click()
    if (create) await page.getByLabel('node ID', { exact: true }).fill('write-denied')
    const content = { ...audit, title: create ? 'create after retry' : 'edit after retry' }
    const raw = '  ' + JSON.stringify(content, null, 4) + '\n'
    await page.getByLabel('node 内容 JSON', { exact: true }).fill(raw)
    try {
      execFileSync('icacls.exe', [root, '/deny', `${user}:(WD)`])
      await page.getByRole('button', { name: '保存 node', exact: true }).click()
      await expect(page.getByRole('alert').filter({ hasText: '写入失败' })).toBeVisible()
      await expect(page.getByLabel('node ID', { exact: true })).toHaveValue('write-denied')
      await expect(page.getByLabel('node 内容 JSON', { exact: true })).toHaveValue(raw)
      await expect(page.getByRole('form', { name: 'node 编辑', exact: true })).toBeVisible()
      const response = await save(page, 'write-denied', raw, current.revision, create)
      expect(response.ok()).toBe(false)
      expect((await response.json()).message).toContain('写入失败')
      expect(inventory(root)).toEqual(before)
    } finally { execFileSync('icacls.exe', [root, '/remove:d', user]) }
    await page.getByRole('button', { name: '保存 node', exact: true }).click()
    await expect(page.getByRole('form', { name: 'node 编辑', exact: true })).toHaveCount(0)
    expect(await readMap(page)).toEqual({ ...original, nodes: { ...original.nodes, 'write-denied': content } })
    expect(inventory(root)['untouched.json']).toBe(before['untouched.json'])
  }
})

// P19 S1: cancel each actual exit and then accept the same exit. Native
// beforeunload is verified by behavior/type only. Both create and edit use the
// real shared guard; edit also runs at the narrow delivered viewport.
test('modified node drafts confirm return, Map and page switches, application refresh and native browser refresh', async ({ page }) => {
  const root = await setup(page), before = inventory(root)
  const raw = '  ' + JSON.stringify(audit, null, 3) + '\n'
  for (const create of [true, false]) {
    await page.setViewportSize({ width: create ? 1280 : 390, height: create ? 900 : 844 })
    for (const action of ['map', 'file', 'bot-tab', 'refresh', 'tab', 'keyboard-tab', 'server', 'browser']) {
      if (await page.getByTestId('server-back').isVisible()) await page.getByTestId('server-back').click()
      await page.getByTestId('tab-scheduler').click()
      const back = page.getByRole('button', { name: '返回 Map', exact: true })
      if (await back.isVisible()) await back.click()
      await page.getByRole('button', { name: 'work.JSON', exact: true }).click()
      if (!create) await page.getByRole('region', { name: '节点列表', exact: true }).getByRole('button', { name: '保留的工作 B', exact: true }).click()
      await page.getByRole('button', { name: create ? '新增 node' : '修改 node', exact: true }).click()
      if (create) await page.getByLabel('node ID', { exact: true }).fill('navigation-draft')
      await page.getByLabel('node 内容 JSON', { exact: true }).fill(raw)
      if (action === 'browser') {
        const browserPage = await page.context().newCDPSession(page)
        const cancelled = page.waitForEvent('dialog'), reload = browserPage.send('Page.reload')
        const dialog = await cancelled
        expect(dialog.type()).toBe('beforeunload')
        await dialog.dismiss(); await reload
        await expect(page.getByLabel('node 内容 JSON', { exact: true })).toHaveValue(raw)
        await expect(page.getByLabel('node ID', { exact: true })).toHaveValue(create ? 'navigation-draft' : 'B')
        const accepted = page.waitForEvent('dialog'), loaded = page.reload()
        const acceptedDialog = await accepted
        expect(acceptedDialog.type()).toBe('beforeunload')
        await acceptedDialog.accept(); await loaded
        await expect(page.getByTestId('tab-sessions')).toHaveAttribute('aria-selected', 'true')
        await browserPage.detach()
      } else {
        const exit = async () => {
          if (action === 'keyboard-tab') { await page.getByTestId('tab-scheduler').focus(); await page.keyboard.press('Home') }
          else if (action === 'tab') await page.getByTestId('tab-tasks').click()
          else if (action === 'server') await page.getByTestId('server-button').click()
          else if (action === 'bot-tab') await page.getByTestId('tab-bot').click()
          else await page.getByRole('button', { name: ({ map: '返回 Map', file: 'untouched.json', refresh: '刷新全部' } as Record<string, string>)[action], exact: true }).click()
        }
        page.once('dialog', dialog => { expect(dialog.type()).toBe('confirm'); return dialog.dismiss() })
        await exit()
        await expect(page.getByLabel('node ID', { exact: true })).toHaveValue(create ? 'navigation-draft' : 'B')
        await expect(page.getByLabel('node 内容 JSON', { exact: true })).toHaveValue(raw)
        page.once('dialog', dialog => { expect(dialog.type()).toBe('confirm'); return dialog.accept() })
        await exit()
        await expect(page.getByRole('form', { name: 'node 编辑', exact: true })).toHaveCount(0)
        if (action === 'file') await expect(page.getByRole('button', { name: 'untouched.json', exact: true })).toHaveAttribute('aria-pressed', 'true')
        else if (action === 'bot-tab') await expect(page.getByRole('region', { name: '共享 Bot 列表', exact: true })).toBeVisible()
        else if (action === 'tab') await expect(page.getByTestId('tab-tasks')).toHaveAttribute('aria-selected', 'true')
        else if (action === 'keyboard-tab') await expect(page.getByTestId('tab-sessions')).toHaveAttribute('aria-selected', 'true')
        else if (action === 'server') await expect(page.getByTestId('server-page')).toBeVisible()
        else await expect(page.getByRole('region', { name: 'Map 预览', exact: true })).toBeVisible()
      }
      expect(inventory(root)).toEqual(before)
    }
  }
})
