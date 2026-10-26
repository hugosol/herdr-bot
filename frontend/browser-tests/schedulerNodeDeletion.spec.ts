import fs from 'node:fs'
import path from 'node:path'
import { execFileSync } from 'node:child_process'
import { expect, test, type Page } from '@playwright/test'
import { startServer, type E2EServer } from './support'

let server: E2EServer | null = null
test.afterEach(async () => { await server?.cleanup(); server = null })
const original = { schema_version: 1, title: '保留 Map 标题', description: '保留 Map 说明', nodes: {
  A: { title: '独立审计', node_template_id: 'audit', parameters: { id: 'business-id', extra: { list: [1, null, { approved: false }] } }, blocked_by: [] as string[] },
  B: { title: '后继工作', node_template_id: 'summary', parameters: { nested: { order: ['keep', null, 7], enabled: true } }, blocked_by: ['A'] },
  D: { title: '独立工作', node_template_id: 'missing', parameters: { untouched: [{ count: 9 }, null] }, blocked_by: [] as string[] },
} }
const inventory = (root: string) => Object.fromEntries(fs.readdirSync(root).sort().map(name => [name, fs.readFileSync(path.join(root, name), 'utf8')]))
async function setup(page: Page, source: unknown = original) {
  server = await startServer({ herdr: {}, separateCwd: true })
  const root = path.join(server.cwd, '.data/workflow-maps')
  fs.mkdirSync(root, { recursive: true })
  fs.writeFileSync(path.join(root, 'work.JSON'), JSON.stringify(source))
  fs.writeFileSync(path.join(root, 'untouched.json'), '  ' + JSON.stringify(original) + '\n')
  await page.goto(server.baseURL)
  await page.getByTestId('tab-scheduler').click()
  await page.getByRole('button', { name: 'work.JSON', exact: true }).click()
  await expect(page.getByRole('region', { name: 'Map 预览', exact: true })).toBeVisible()
  return root
}
async function request(page: Page, operation: string, data: unknown) {
  return page.request.post(`${server!.baseURL}/api/scheduler/maps/work.JSON/nodes/${operation}`, { data })
}
async function select(page: Page, name: string) {
  await page.getByRole('region', { name: '节点列表', exact: true }).getByRole('button', { name, exact: true }).click()
}
async function readMap(page: Page) {
  const response = await page.request.get(`${server!.baseURL}/api/scheduler/maps/work.JSON`)
  expect(response.ok()).toBe(true)
  return response.json()
}

// Approved seams S1 delivered page and S2 production HTTP. P20 requires the
// literal dependent B to block A; a cascade or trusting only the page would
// disagree with both the HTTP re-read and unchanged real file inventory.
test('P20 referenced node deletion is refused by the page and direct production requests with the dependent identity', async ({ page }) => {
  const root = await setup(page), before = inventory(root)
  await select(page, '独立审计 A')
  let confirmations = 0
  page.on('dialog', async dialog => { confirmations++; await dialog.accept() })
  const checked = await request(page, 'check-delete', { id: 'A' })
  expect(checked.ok()).toBe(false)
  expect((await checked.json()).message).toContain('B')
  const removed = await request(page, 'delete', { id: 'A', revision: { file: 'work.JSON', original: before['work.JSON'] } })
  expect(removed.ok()).toBe(false)
  expect((await removed.json()).message).toContain('B')
  await page.getByRole('button', { name: '删除 node', exact: true }).click()
  await expect(page.getByRole('alert').filter({ hasText: '阻止删除' })).toContainText('B')
  expect(confirmations).toBe(0)
  expect(await readMap(page)).toEqual(original)
  expect(inventory(root)).toEqual(before)
  expect(server!.herdrCalls()).toEqual([])
})

// P23/P27/P31: a cancel must leave bytes intact; accepting D must remove
// only D, retain Map context and update the already-rendered dependency graph.
// A final deletion must expose the genuine empty state, not stale details.
test('P23 P27 P31 cancellation preserves bytes and confirmed deletion updates all current views through the last node', async ({ page }) => {
  const root = await setup(page), before = inventory(root)
  await page.getByText('依赖图', { exact: true }).click()
  await select(page, '独立工作 D')
  let checked = false, checkedBeforeDialog = false, deleteRequests = 0
  page.on('response', response => {
    if (response.url().endsWith('/nodes/check-delete') && response.ok()) checked = true
  })
  page.on('request', request => { if (request.url().endsWith('/nodes/delete')) deleteRequests++ })
  page.once('dialog', async dialog => {
    checkedBeforeDialog = checked
    await dialog.dismiss()
  })
  await page.getByRole('button', { name: '删除 node', exact: true }).click()
  await expect(page.getByRole('button', { name: '删除 node', exact: true })).toBeEnabled()
  expect(checkedBeforeDialog).toBe(true)
  expect(deleteRequests).toBe(0)
  expect(await readMap(page)).toEqual(original)
  expect(inventory(root)).toEqual(before)
  await expect(page.getByRole('region', { name: 'node 详情', exact: true })).toContainText('独立工作')
  page.once('dialog', dialog => dialog.accept())
  await page.getByRole('button', { name: '删除 node', exact: true }).click()
  await expect(page.getByRole('region', { name: 'node 详情', exact: true })).toHaveCount(0)
  await expect(page.getByRole('button', { name: 'work.JSON', exact: true })).toHaveAttribute('aria-pressed', 'true')
  await expect(page.getByRole('region', { name: '节点列表', exact: true })).not.toContainText('独立工作')
  await expect(page.getByRole('group', { name: 'Map 依赖图', exact: true })).not.toContainText('独立工作')
  await expect(page.getByRole('img', { name: 'A → B', exact: true })).toHaveCount(1)
  const expected = { schema_version: 1, title: '保留 Map 标题', description: '保留 Map 说明', nodes: { A: original.nodes.A, B: original.nodes.B } }
  expect(await readMap(page)).toEqual(expected)
  expect(JSON.parse(fs.readFileSync(path.join(root, 'work.JSON'), 'utf8'))).toEqual(expected)
  expect(inventory(root)['untouched.json']).toBe(before['untouched.json'])
  await select(page, '后继工作 B')
  page.once('dialog', dialog => dialog.accept())
  await page.getByRole('button', { name: '删除 node', exact: true }).click()
  await expect(page.getByRole('region', { name: 'node 详情', exact: true })).toHaveCount(0)
  await select(page, '独立审计 A')
  page.once('dialog', dialog => dialog.accept())
  await page.getByRole('button', { name: '删除 node', exact: true }).click()
  await expect(page.getByRole('region', { name: '节点列表', exact: true }).getByText('此 Map 没有 node。', { exact: true })).toBeVisible()
  await expect(page.locator('details').filter({ has: page.getByText('依赖图', { exact: true }) }).getByText('此 Map 没有 node。', { exact: true })).toBeVisible()
  await expect(page.getByRole('region', { name: 'node 详情', exact: true })).toHaveCount(0)
  await expect(page.getByRole('button', { name: 'work.JSON', exact: true })).toHaveAttribute('aria-pressed', 'true')
  await expect(page.getByRole('button', { name: '新增 node', exact: true })).toBeVisible()
  expect(await readMap(page)).toEqual({ schema_version: 1, title: '保留 Map 标题', description: '保留 Map 说明', nodes: {} })
  expect(JSON.parse(fs.readFileSync(path.join(root, 'work.JSON'), 'utf8'))).toEqual({ schema_version: 1, title: '保留 Map 标题', description: '保留 Map 说明', nodes: {} })
  expect(inventory(root)['untouched.json']).toBe(before['untouched.json'])
  expect(deleteRequests).toBe(3)
  expect(server!.herdrCalls()).toEqual([])
})

// P24: mutate the real owning Map while the native confirmation is open.
// Both external edits and an application save must invalidate the final HTTP
// request. Trusting preflight or identifying the target by position would lose
// the newer content/dependency. The final request and full inventory expose it.
for (const change of ['external-target', 'external-dependent', 'application-dependent', 'removed-target', 'invalid-map', 'removed-map'] as const) {
  test(`P24 final deletion rejects ${change} made after confirmation opens`, async ({ page }) => {
    const root = await setup(page), file = path.join(root, 'work.JSON')
    await select(page, '独立工作 D')
    const preflight = await request(page, 'check-delete', { id: 'D' })
    expect(preflight.ok()).toBe(true)
    const target = await preflight.json()
    const dialogPromise = page.waitForEvent('dialog')
    const clicking = page.getByRole('button', { name: '删除 node', exact: true }).click()
    const dialog = await dialogPromise
    expect(dialog.message()).toContain('D')
    if (change === 'application-dependent') {
      const editing = await request(page, 'edit', { id: 'B' })
      expect(editing.ok()).toBe(true)
      const draft = await editing.json()
      const saved = await request(page, 'save', { id: 'B', content: JSON.stringify({ ...original.nodes.B, blocked_by: ['A', 'D'] }), revision: draft.revision, create: false })
      expect(saved.ok()).toBe(true)
    } else if (change === 'external-target') {
      fs.writeFileSync(file, JSON.stringify({ ...original, nodes: { ...original.nodes, D: { ...original.nodes.D, title: 'newer target', parameters: { external: [null, 19] } } } }))
    } else if (change === 'external-dependent') {
      fs.writeFileSync(file, JSON.stringify({ ...original, nodes: { ...original.nodes, B: { ...original.nodes.B, blocked_by: ['A', 'D'] } } }))
    } else if (change === 'removed-target') {
      fs.writeFileSync(file, JSON.stringify({ ...original, nodes: { A: original.nodes.A, B: original.nodes.B } }))
    } else if (change === 'invalid-map') {
      fs.writeFileSync(file, '{broken Map')
    } else fs.unlinkSync(file)
    const newer = inventory(root)
    await dialog.accept()
    await clicking
    await expect(page.getByRole('alert').filter({ hasText: /阻止删除|重新加载|Map.*无效|Map.*失败/ })).toBeVisible()
    await expect(page.getByRole('region', { name: 'node 详情', exact: true })).toContainText('独立工作')
    const direct = await request(page, 'delete', target)
    expect(direct.ok()).toBe(false)
    expect(inventory(root)).toEqual(newer)
    if (change !== 'invalid-map' && change !== 'removed-map') {
      expect(await readMap(page)).toEqual(JSON.parse(newer['work.JSON']))
    }
    expect(server!.herdrCalls()).toEqual([])
  })
}

// Complete-read prerequisite: unrelated invalid nodes, invalid dependencies and
// unavailable files must fail closed before confirmation, not become zero refs.
test('unreadable or invalid Map and ambiguous identities never authorize deletion or show final confirmation', async ({ page }) => {
  const root = await setup(page), file = path.join(root, 'work.JSON')
  await select(page, '独立工作 D')
  let confirmations = 0
  page.on('dialog', async dialog => { confirmations++; await dialog.dismiss() })
  const safe = await request(page, 'check-delete', { id: 'D' })
  expect(safe.ok()).toBe(true)
  const target = await safe.json()
  for (const [source, reason] of [
    ['{invalid', /Map.*无效/],
    [JSON.stringify({ ...original, nodes: { ...original.nodes, B: { ...original.nodes.B, blocked_by: ['absent'] } } }), /node B 的 blocked_by 引用了未知 node：absent/],
    [JSON.stringify({ ...original, nodes: { ...original.nodes, B: { title: 'invalid unrelated node' } } }), /Map.*无效/],
    [null, /Map.*失败/],
  ] as const) {
    if (source === null) fs.unlinkSync(file)
    else fs.writeFileSync(file, source)
    const before = inventory(root)
    const checked = await request(page, 'check-delete', { id: 'D' })
    expect(checked.ok()).toBe(false)
    expect((await checked.json()).message).toMatch(reason)
    const removed = await request(page, 'delete', target)
    expect(removed.ok()).toBe(false)
    expect((await removed.json()).message).toMatch(reason)
    await page.getByRole('button', { name: '删除 node', exact: true }).click()
    await expect(page.getByRole('alert').filter({ hasText: reason })).toBeVisible()
    await expect(page.getByRole('button', { name: '删除 node', exact: true })).toBeEnabled()
    expect(confirmations).toBe(0)
    expect(inventory(root)).toEqual(before)
  }
  fs.writeFileSync(file, JSON.stringify(original))
  const before = inventory(root)
  for (const payload of [{ id: 'D' }, { ...target, revision: null }, { ...target, id: 'unknown' }, { ...target, revision: { ...target.revision, file: 'untouched.json' } }, { ...target, revision: { ...target.revision, original: '' } }]) {
    expect((await request(page, 'delete', payload)).ok()).toBe(false)
    expect(inventory(root)).toEqual(before)
  }
  expect(await readMap(page)).toEqual(original)
  expect(server!.herdrCalls()).toEqual([])
})

// Actual OS write denial leaves validation/coordination/commit exercised rather
// than mocking a helper. Failures must preserve both file bytes and UI selection.
test('filesystem deletion commit failure is explicit and preserves definitions until a new confirmed retry', async ({ page }) => {
  const root = await setup(page), before = inventory(root)
  const user = execFileSync('whoami.exe', [], { encoding: 'utf8' }).trim()
  await select(page, '独立工作 D')
  const checked = await request(page, 'check-delete', { id: 'D' })
  expect(checked.ok()).toBe(true)
  const target = await checked.json()
  try {
    execFileSync('icacls.exe', [root, '/deny', `${user}:(WD)`])
    page.once('dialog', dialog => dialog.accept())
    await page.getByRole('button', { name: '删除 node', exact: true }).click()
    await expect(page.getByRole('alert').filter({ hasText: '删除写入失败' })).toBeVisible()
    await expect(page.getByRole('region', { name: 'node 详情', exact: true })).toContainText('独立工作')
    const removed = await request(page, 'delete', target)
    expect(removed.ok()).toBe(false)
    expect((await removed.json()).message).toContain('删除写入失败')
    expect(await readMap(page)).toEqual(original)
    expect(inventory(root)).toEqual(before)
  } finally { execFileSync('icacls.exe', [root, '/remove:d', user]) }
  page.once('dialog', dialog => dialog.accept())
  await page.getByRole('button', { name: '删除 node', exact: true }).click()
  await expect(page.getByRole('region', { name: 'node 详情', exact: true })).toHaveCount(0)
  expect(await readMap(page)).toEqual({ ...original, nodes: { A: original.nodes.A, B: original.nodes.B } })
  expect(inventory(root)['untouched.json']).toBe(before['untouched.json'])
  expect(server!.herdrCalls()).toEqual([])
})
