import fs from 'node:fs'
import { execFileSync } from 'node:child_process'
import path from 'node:path'
import { expect, test, type Page } from '@playwright/test'
import { startServer, type E2EServer } from './support'

let server: E2EServer | null = null
test.afterEach(async () => { await server?.cleanup(); server = null })
const content = { name: '人工审计', when: '', plan_guide: '只核对账目，不查询天气', node_guide: JSON.stringify({ inputs: {}, outputs: {} }) }
async function open(page: Page) {
  await page.goto(server!.baseURL)
  await page.getByTestId('tab-scheduler').click()
  await page.getByRole('button', { name: 'Bot 管理', exact: true }).click()
}
const inventory = (root: string) => Object.fromEntries(fs.readdirSync(root).sort().map(name => [name, fs.readFileSync(path.join(root, name), 'utf8')]))

// S1/S2 P1 P6 P8 P9 P11 P28: the literal weather fields come from the
// approved example, not the implementation. Opening/cancelling must not install
// it; saving a changed draft must reach the real file and latest preview.
test('empty bot list creates an independent-ID editable bundled draft and persists only on save', async ({ page }) => {
  server = await startServer({ herdr: {}, separateCwd: true })
  await open(page)
  const root = path.join(server.cwd, '.data/bots')
  const maps = path.join(server.cwd, '.data/workflow-maps')
  const mapBefore = inventory(maps)
  expect(mapBefore).toEqual({})
  expect(inventory(root)).toEqual({})
  await page.getByRole('button', { name: '新增 Bot', exact: true }).click()
  const id = page.getByLabel('Bot ID', { exact: true })
  const json = page.getByLabel('Bot 内容 JSON', { exact: true })
  await expect(id).toHaveValue('')
  expect(JSON.parse(await json.inputValue())).toEqual({ name: '目的地天气摘要', when: '出行前需要了解目的地天气与随身物品建议时。', plan_guide: '整理指定城市和日期的天气摘要及出行建议；不预订行程，不提供实时查询保证。', node_guide: '请在 input 中说明城市、计划出行日期与偏好。输出温度、降水与随身物品建议的文本摘要，标明信息来源和不确定性。这是工作指引，不是字段校验规则。' })
  expect(inventory(root)).toEqual({})
  await page.getByRole('button', { name: '放弃编辑', exact: true }).click()
  expect(inventory(root)).toEqual({})
  expect(inventory(maps)).toEqual(mapBefore)
  fs.writeFileSync(path.join(root, 'sentinel.json'), 'do not overwrite\n')
  await page.getByRole('button', { name: '新增 Bot', exact: true }).click()
  await id.fill('../outside:weather\\id')
  await json.fill(JSON.stringify(content, null, 2))
  await page.getByRole('button', { name: '保存 Bot', exact: true }).click()
  const preview = page.getByRole('region', { name: 'Bot 预览', exact: true })
  await expect(preview).toContainText('../outside:weather\\id')
  await expect(preview).toContainText('只核对账目，不查询天气')
  const response = await page.request.get(`${server.baseURL}/api/scheduler/bots`)
  expect((await response.json()).bots).toEqual([{ ...content, id: '../outside:weather\\id' }])
  const files = inventory(root)
  expect(files['sentinel.json']).toBe('do not overwrite\n')
  const created = Object.keys(files).filter(name => name !== 'sentinel.json')
  expect(created).toHaveLength(1)
  expect(created[0]).toMatch(/^bot-[a-zA-Z0-9-]+\.json$/)
  expect(JSON.parse(files[created[0]])).toEqual({ ...content, id: '../outside:weather\\id' })
  expect(fs.existsSync(path.join(server.cwd, 'outside:weather\\id'))).toBe(false)
  expect(inventory(maps)).toEqual(mapBefore)
  expect(server.herdrCalls()).toEqual([])
})

// P6 P10 P12 P28: this non-weather definition has no sample inputs. A merge
// with the starter or a filename derived from identity would fail the literal
// editor and file-location observations.
test('editing loads only real content, keeps immutable identity and original filename, and previews the saved result', async ({ page }) => {
  server = await startServer({ separateCwd: true })
  const root = path.join(server.cwd, '.data/bots')
  fs.mkdirSync(root, { recursive: true })
  const file = path.join(root, 'personal-filename.JSON')
  fs.writeFileSync(file, JSON.stringify({ ...content, id: 'audit-only' }))
  await open(page)
  await page.getByRole('button', { name: '人工审计 · audit-only', exact: true }).click()
  await page.getByRole('button', { name: '修改 Bot', exact: true }).click()
  const id = page.getByLabel('Bot ID', { exact: true })
  await expect(id).toHaveValue('audit-only')
  await expect(id).toHaveAttribute('readonly', '')
  const json = page.getByLabel('Bot 内容 JSON', { exact: true })
  expect(JSON.parse(await json.inputValue())).toEqual(content)
  const updated = { ...content, name: '', plan_guide: '审计新版本', node_guide: '独立核对意见' }
  await json.fill(JSON.stringify(updated))
  await page.getByRole('button', { name: '保存 Bot', exact: true }).click()
  await expect(page.getByRole('region', { name: 'Bot 预览', exact: true })).toContainText('审计新版本')
  expect(inventory(root)).toEqual({ 'personal-filename.JSON': expect.any(String) })
  expect(JSON.parse(fs.readFileSync(file, 'utf8'))).toEqual({ ...updated, id: 'audit-only' })
  const response = await page.request.post(`${server.baseURL}/api/scheduler/bots/edit`, { data: { id: 'audit-only' } })
  expect(response.status()).toBe(200)
  expect(await response.json()).toMatchObject({ id: 'audit-only', content: updated, revision: { file: 'personal-filename.JSON' } })
})

// P7 P14 P17 S1/S2: independent malformed examples must fail without changing
// any byte, and the real page must retain the exact (not reformatted) draft.
// A server that accepts positional implementation arrays violates object types.
test('invalid bot content cannot bypass validation and keeps the original ID and raw draft', async ({ page }) => {
  server = await startServer({ separateCwd: true })
  const root = path.join(server.cwd, '.data/bots')
  fs.mkdirSync(root, { recursive: true })
  fs.writeFileSync(path.join(root, 'preserve-existing.json'), JSON.stringify({ ...content, id: 'invalid-candidate' }) + '\n')
  await open(page)
  const before = inventory(root)
  await page.getByRole('button', { name: '人工审计 · invalid-candidate', exact: true }).click()
  await page.getByRole('button', { name: '修改 Bot', exact: true }).click()
  const edit = await (await page.request.post(`${server.baseURL}/api/scheduler/bots/edit`, { data: { id: 'invalid-candidate' } })).json()
  const id = page.getByLabel('Bot ID', { exact: true }), json = page.getByLabel('Bot 内容 JSON', { exact: true })
  const cases: { raw: string; diagnostic: string }[] = [
    { raw: '{ "name": ', diagnostic: 'JSON' },
    { raw: JSON.stringify({ ...content, id: 'silent-override' }), diagnostic: '移除' },
    { raw: JSON.stringify({ ...content, schema_version: 8 }), diagnostic: 'schema_version' },
    { raw: JSON.stringify({ ...content, schema_version: 1.5 }), diagnostic: 'schema_version' },
    { raw: JSON.stringify({ ...content, typo: true }), diagnostic: 'typo' },
    { raw: JSON.stringify({ ...content, implementation: { type: 'shell' } }), diagnostic: 'implementation' },
    { raw: JSON.stringify({ ...content, implementation: { type: 'chat', command: 'no' } }), diagnostic: 'implementation' },
    { raw: JSON.stringify({ ...content, implementation: ['chat'] }), diagnostic: 'implementation' },
    { raw: '[]', diagnostic: '对象' },
  ]
  for (const key of Object.keys(content)) {
    const missing: Record<string, unknown> = { ...content }; delete missing[key]
    cases.push({ raw: JSON.stringify(missing), diagnostic: key })
  }
  for (const [key, value] of [['name', null], ['when', 4], ['plan_guide', []], ['node_guide', {}], ['task_id', 2]] as const) {
    cases.push({ raw: JSON.stringify({ ...content, [key]: value }), diagnostic: key })
  }
  for (const item of cases) {
    await json.fill(item.raw)
    await page.getByRole('button', { name: '保存 Bot', exact: true }).click()
    await expect(page.getByRole('alert')).toContainText(item.diagnostic)
    await expect(id).toHaveValue('invalid-candidate')
    await expect(json).toHaveValue(item.raw)
    await expect(page.getByRole('region', { name: 'Bot 预览', exact: true })).toHaveCount(0)
    for (const revision of [null, edit.revision]) {
      const response = await page.request.post(`${server.baseURL}/api/scheduler/bots/save`, { data: { id: 'invalid-candidate', content: item.raw, revision } })
      expect(response.ok(), item.raw).toBe(false)
      expect((await response.json()).message).toContain(item.diagnostic)
      expect(inventory(root)).toEqual(before)
    }
  }
  const validEmpty = { name: '', when: '', plan_guide: '', node_guide: JSON.stringify({ inputs: {}, outputs: {} }) }
  await json.fill(JSON.stringify(validEmpty))
  await page.getByRole('button', { name: '保存 Bot', exact: true }).click()
  await expect(page.getByRole('region', { name: 'Bot 预览', exact: true })).toContainText('invalid-candidate')
  expect((await (await page.request.get(`${server.baseURL}/api/scheduler/bots`)).json()).bots).toEqual([{ ...validEmpty, id: 'invalid-candidate' }])
})

test('Bot save rejects legacy title fields and reads back only the name contract', async ({ request }) => {
  server = await startServer({ separateCwd: true })
  const initial = await request.get(`${server.baseURL}/api/scheduler/bots`)
  expect(initial.status()).toBe(200)
  const root = path.join(server.cwd, '.data/bots')
  const before = inventory(root)
  const { name, ...withoutName } = content
  const legacy = { ...withoutName, title: name }
  for (const candidate of [legacy, { ...content, title: 'Not a runtime alias' }]) {
    const response = await request.post(`${server.baseURL}/api/scheduler/bots/save`, {
      data: { id: 'name-contract', content: JSON.stringify(candidate), revision: null },
    })
    expect(response.ok()).toBe(false)
    expect(inventory(root)).toEqual(before)
  }
  const response = await request.post(`${server.baseURL}/api/scheduler/bots/save`, {
    data: { id: 'name-contract', content: JSON.stringify(content), revision: null },
  })
  expect(response.ok()).toBe(true)
  const expected = { ...content, id: 'name-contract' }
  expect(await response.json()).toEqual(expected)
  const catalog = await request.get(`${server.baseURL}/api/scheduler/bots`)
  expect((await catalog.json()).bots).toEqual([expected])
  const draft = await request.post(`${server.baseURL}/api/scheduler/bots/edit`, { data: { id: 'name-contract' } })
  const saved = await draft.json()
  expect(saved.content).toEqual(content)
  expect(JSON.parse(fs.readFileSync(path.join(root, saved.revision.file), 'utf8'))).toEqual(expected)
})

// P13 P17 S1/S2: actual overlapping HTTP creates of the same global identity
// must have one winner. Existing and ambiguous targets cannot be overwritten.
test('global duplicate and concurrent same-ID creation reject without overwriting definitions or drafts', async ({ page }) => {
  server = await startServer({ separateCwd: true })
  await open(page)
  const root = path.join(server.cwd, '.data/bots')
  const requests = await Promise.all(['first contender', 'second contender'].map(plan_guide => page.request.post(`${server!.baseURL}/api/scheduler/bots/save`, {
    data: { id: 'one-global-id', content: JSON.stringify({ ...content, plan_guide }), revision: null },
  })))
  expect(requests.filter(response => response.ok())).toHaveLength(1)
  expect(requests.filter(response => !response.ok())).toHaveLength(1)
  const loser = requests.find(response => !response.ok())!
  expect((await loser.json()).message).toContain('ID 已存在')
  const winner = await requests.find(response => response.ok())!.json()
  expect(['first contender', 'second contender']).toContain(winner.plan_guide)
  const before = inventory(root)
  expect(Object.keys(before)).toHaveLength(1)
  expect(JSON.parse(Object.values(before)[0])).toEqual(winner)
  await page.getByRole('button', { name: '新增 Bot', exact: true }).click()
  const raw = '  ' + JSON.stringify(content) + '\n'
  await page.getByLabel('Bot ID', { exact: true }).fill('one-global-id')
  await page.getByLabel('Bot 内容 JSON', { exact: true }).fill(raw)
  await page.getByRole('button', { name: '保存 Bot', exact: true }).click()
  await expect(page.getByRole('alert')).toContainText('ID 已存在')
  await expect(page.getByLabel('Bot ID', { exact: true })).toHaveValue('one-global-id')
  await expect(page.getByLabel('Bot 内容 JSON', { exact: true })).toHaveValue(raw)
  expect(inventory(root)).toEqual(before)
  const edit = await (await page.request.post(`${server.baseURL}/api/scheduler/bots/edit`, { data: { id: 'one-global-id' } })).json()
  fs.writeFileSync(path.join(root, 'ambiguous.json'), JSON.stringify({ ...content, id: 'one-global-id' }))
  const ambiguous = inventory(root)
  for (const endpoint of ['edit', 'save']) {
    const response = await page.request.post(`${server.baseURL}/api/scheduler/bots/${endpoint}`, { data: endpoint === 'edit' ? { id: 'one-global-id' } : { id: 'one-global-id', content: JSON.stringify(content), revision: edit.revision } })
    expect(response.ok()).toBe(false)
    expect((await response.json()).message).toContain('ID 冲突')
    expect(inventory(root)).toEqual(ambiguous)
  }
})

// P18 P17 S1/S2: opening the actual editor captures an old version. A second
// page, an external writer, and an external removal each invalidate that exact
// version. Re-reading only at save without comparing would overwrite newer data.
test('two clients and external modification or removal reject stale bot drafts and require reload', async ({ page, context }) => {
  server = await startServer({ separateCwd: true })
  const root = path.join(server.cwd, '.data/bots')
  fs.mkdirSync(root, { recursive: true })
  const file = path.join(root, 'original.json')
  const raw = '\n' + JSON.stringify({ ...content, plan_guide: '旧页面的未保存修改' }, null, 3) + '\n'
  for (const mode of ['second-client', 'external-change', 'external-remove']) {
    fs.writeFileSync(file, JSON.stringify({ ...content, id: 'revision-id' }))
    await open(page)
    await page.getByRole('button', { name: '人工审计 · revision-id', exact: true }).click()
    await page.getByRole('button', { name: '修改 Bot', exact: true }).click()
    await expect(page.getByLabel('Bot ID', { exact: true })).toHaveValue('revision-id')
    const edit = await (await page.request.post(`${server.baseURL}/api/scheduler/bots/edit`, { data: { id: 'revision-id' } })).json()
    await page.getByLabel('Bot 内容 JSON', { exact: true }).fill(raw)
    if (mode === 'second-client') {
      const other = await context.newPage()
      await open(other)
      await other.getByRole('button', { name: '人工审计 · revision-id', exact: true }).click()
      await other.getByRole('button', { name: '修改 Bot', exact: true }).click()
      await other.getByLabel('Bot 内容 JSON', { exact: true }).fill(JSON.stringify({ ...content, plan_guide: '第二客户端的新版本' }))
      await other.getByRole('button', { name: '保存 Bot', exact: true }).click()
      await expect(other.getByRole('region', { name: 'Bot 预览', exact: true })).toContainText('第二客户端的新版本')
      await other.close()
    } else if (mode === 'external-change') {
      fs.writeFileSync(file, JSON.stringify({ ...content, id: 'revision-id', plan_guide: '外部编辑器的新版本' }) + '\n')
    } else fs.unlinkSync(file)
    const newer = inventory(root)
    await page.getByRole('button', { name: '保存 Bot', exact: true }).click()
    await expect(page.getByRole('alert')).toContainText('重新加载')
    await expect(page.getByLabel('Bot ID', { exact: true })).toHaveValue('revision-id')
    await expect(page.getByLabel('Bot 内容 JSON', { exact: true })).toHaveValue(raw)
    await expect(page.getByRole('region', { name: 'Bot 预览', exact: true })).toHaveCount(0)
    const stale = await page.request.post(`${server.baseURL}/api/scheduler/bots/save`, { data: { id: 'revision-id', content: raw, revision: edit.revision } })
    expect(stale.ok()).toBe(false)
    expect((await stale.json()).message).toContain('重新加载')
    expect(inventory(root)).toEqual(newer)
    page.once('dialog', dialog => dialog.accept())
    await page.getByRole('button', { name: '放弃编辑', exact: true }).click()
  }
})

// P17: deny actual directory file creation, leaving real validation/commit in
// the path. A fetch mock cannot prove that a failed write preserves definitions.
test('a controlled filesystem write failure preserves the ID and raw JSON until retry succeeds', async ({ page }) => {
  server = await startServer({ separateCwd: true })
  await open(page)
  await page.getByRole('button', { name: '新增 Bot', exact: true }).click()
  const root = path.join(server.cwd, '.data/bots'), before = inventory(root)
  const raw = '  ' + JSON.stringify(content, null, 4) + '\n'
  await page.getByLabel('Bot ID', { exact: true }).fill('write-denied')
  await page.getByLabel('Bot 内容 JSON', { exact: true }).fill(raw)
  const user = execFileSync('whoami.exe', [], { encoding: 'utf8' }).trim()
  try {
    execFileSync('icacls.exe', [root, '/deny', `${user}:(WD)`])
    await page.getByRole('button', { name: '保存 Bot', exact: true }).click()
    await expect(page.getByRole('alert')).toContainText('写入失败')
    await expect(page.getByLabel('Bot ID', { exact: true })).toHaveValue('write-denied')
    await expect(page.getByLabel('Bot 内容 JSON', { exact: true })).toHaveValue(raw)
    await expect(page.getByRole('region', { name: 'Bot 预览', exact: true })).toHaveCount(0)
    const response = await page.request.post(`${server.baseURL}/api/scheduler/bots/save`, { data: { id: 'write-denied', content: raw, revision: null } })
    expect(response.ok()).toBe(false)
    expect((await response.json()).message).toContain('写入失败')
    expect(inventory(root)).toEqual(before)
  } finally { execFileSync('icacls.exe', [root, '/remove:d', user]) }
  await page.getByRole('button', { name: '保存 Bot', exact: true }).click()
  await expect(page.getByRole('region', { name: 'Bot 预览', exact: true })).toContainText('write-denied')
})

// P19 S1: every real exit first cancels (same exact draft), then confirms
// (original action runs). Native beforeunload is asserted by type, never text.
test('modified bot drafts confirm bot selection, return, tab and server navigation, app refresh and browser refresh', async ({ page }) => {
  server = await startServer({ separateCwd: true })
  const root = path.join(server.cwd, '.data/bots')
  fs.mkdirSync(root, { recursive: true })
  fs.writeFileSync(path.join(root, 'navigation-target.json'), JSON.stringify({ ...content, id: 'navigation-target', name: '切换目标能力' }))
  await open(page)
  const before = inventory(root)
  const raw = JSON.stringify(content, null, 3)
  for (const action of ['map', 'list', 'manage', 'refresh', 'tab', 'keyboard-tab', 'server']) {
    if (await page.getByTestId('server-back').isVisible()) await page.getByTestId('server-back').click()
    await page.getByTestId('tab-scheduler').click()
    await page.getByRole('button', { name: 'Bot 管理', exact: true }).click()
    await page.getByRole('button', { name: '新增 Bot', exact: true }).click()
    await page.getByLabel('Bot ID', { exact: true }).fill('unsaved-identity')
    await page.getByLabel('Bot 内容 JSON', { exact: true }).fill(raw)
    const listing = page.getByRole('region', { name: '共享 Bot 列表', exact: true })
    const destination = listing.getByRole('button', { name: '切换目标能力 · navigation-target', exact: true })
    await expect(listing).toBeVisible()
    await expect(destination).toBeEnabled()
    const exit = async () => {
      if (action === 'keyboard-tab') {
        await page.getByTestId('tab-scheduler').focus()
        await page.keyboard.press('ArrowLeft')
      } else if (action === 'tab') await page.getByTestId('tab-tasks').click()
      else if (action === 'server') await page.getByTestId('server-button').click()
      else if (action === 'list') await destination.click()
      else await page.getByRole('button', { name: ({ map: '返回 Map', manage: 'Bot 管理', refresh: '刷新全部' } as Record<string, string>)[action], exact: true }).click()
    }
    page.once('dialog', dialog => { expect(dialog.type()).toBe('confirm'); return dialog.dismiss() })
    await exit()
    await expect(page.getByLabel('Bot ID', { exact: true })).toHaveValue('unsaved-identity')
    await expect(page.getByLabel('Bot 内容 JSON', { exact: true })).toHaveValue(raw)
    page.once('dialog', dialog => { expect(dialog.type()).toBe('confirm'); return dialog.accept() })
    await exit()
    await expect(page.getByRole('form', { name: 'Bot 编辑', exact: true })).toHaveCount(0)
    if (action === 'map') await expect(page.getByRole('complementary', { name: 'Map 文件目录', exact: true })).toBeVisible()
    else if (action === 'list') {
      const preview = page.getByRole('region', { name: 'Bot 预览', exact: true })
      await expect(preview.getByText('navigation-target', { exact: true })).toBeVisible()
      await expect(preview.getByText('切换目标能力', { exact: true })).toBeVisible()
      await expect(destination).toHaveAttribute('aria-pressed', 'true')
      await expect(listing).toBeVisible()
    }
    else if (action === 'tab') await expect(page.getByTestId('tab-tasks')).toHaveAttribute('aria-selected', 'true')
    else if (action === 'keyboard-tab') await expect(page.getByTestId('tab-files')).toHaveAttribute('aria-selected', 'true')
    else if (action === 'server') await expect(page.getByTestId('server-page')).toBeVisible()
    else await expect(page.getByRole('region', { name: '共享 Bot 列表', exact: true })).toBeVisible()
    expect(inventory(root)).toEqual(before)
  }
  await page.getByTestId('server-back').click()
  await page.getByRole('button', { name: 'Bot 管理', exact: true }).click()
  await page.getByRole('button', { name: '新增 Bot', exact: true }).click()
  await page.getByLabel('Bot ID', { exact: true }).fill('native-refresh-draft')
  await page.getByLabel('Bot 内容 JSON', { exact: true }).fill(raw)
  // A cancelled reload has no load event; dispatch the browser command without
  // asking Playwright to wait for a navigation that the user explicitly stops.
  const browserPage = await page.context().newCDPSession(page)
  const cancelled = page.waitForEvent('dialog')
  const cancelledReload = browserPage.send('Page.reload')
  const cancelledDialog = await cancelled
  expect(cancelledDialog.type()).toBe('beforeunload')
  await cancelledDialog.dismiss()
  await cancelledReload
  await expect(page.getByLabel('Bot ID', { exact: true })).toHaveValue('native-refresh-draft')
  await expect(page.getByLabel('Bot 内容 JSON', { exact: true })).toHaveValue(raw)
  const accepted = page.waitForEvent('dialog')
  const acceptedReload = page.reload()
  const acceptedDialog = await accepted
  expect(acceptedDialog.type()).toBe('beforeunload')
  await acceptedDialog.accept()
  await acceptedReload
  await expect(page.getByTestId('tab-sessions')).toHaveAttribute('aria-selected', 'true')
  expect(inventory(root)).toEqual(before)
})

// P4 P30 S1/S2: one shared definition serves two nodes in one Map and a node
// in another. Return revalidates the original selection rather than retaining
// a removed node/Map, and no per-Map capability copies may appear in storage.
test('saving a shared capability preserves live source selection and updates all references, while removed origins are invalidated', async ({ page }) => {
  server = await startServer({ herdr: {}, separateCwd: true })
  const root = path.join(server.cwd, '.data/bots'), maps = path.join(server.cwd, '.data/workflow-maps')
  fs.mkdirSync(root, { recursive: true }); fs.mkdirSync(maps, { recursive: true })
  fs.writeFileSync(path.join(root, 'shared-original.json'), JSON.stringify({ ...content, id: 'shared-audit' }))
  const node = { title: '核对账目', bot_id: 'shared-audit', input: JSON.stringify({}), zone: 'unregistered-zone', blocked_by: [] }
  const first = { title: '原 Map', description: '保留元信息', nodes: { A: node, B: { ...node, title: '复核账目' } } }
  const second = { title: '另一 Map', description: '', nodes: { C: { ...node, title: '跨 Map 核对' } } }
  fs.writeFileSync(path.join(maps, 'first.json'), JSON.stringify(first))
  fs.writeFileSync(path.join(maps, 'second.json'), JSON.stringify(second))
  const mapBytes = inventory(maps)
  await page.goto(server.baseURL)
  await page.getByTestId('tab-scheduler').click()
  await page.getByRole('button', { name: 'first.json', exact: true }).click()
  await page.getByRole('button', { name: '核对账目 A', exact: true }).click()
  await page.getByRole('button', { name: 'Bot 明细', exact: true }).click()
  await page.getByRole('button', { name: '修改 Bot', exact: true }).click()
  await page.getByLabel('Bot 内容 JSON', { exact: true }).fill(JSON.stringify({ ...content, name: '共享能力新版', plan_guide: '三个引用者均看到新版核对说明' }))
  await page.getByRole('button', { name: '保存 Bot', exact: true }).click()
  await expect(page.getByRole('region', { name: 'Bot 预览', exact: true })).toContainText('三个引用者均看到新版核对说明')
  await page.getByRole('button', { name: '返回 Map', exact: true }).click()
  await expect(page.getByRole('button', { name: '核对账目 A', exact: true })).toHaveAttribute('aria-pressed', 'true')
  await expect(page.getByRole('region', { name: 'node 详情', exact: true })).toContainText('核对账目 · A')
  for (const [filename, button] of [['first.json', '核对账目 A'], ['first.json', '复核账目 B'], ['second.json', '跨 Map 核对 C']]) {
    await page.getByRole('button', { name: filename, exact: true }).click()
    await page.getByRole('button', { name: button, exact: true }).click()
    await page.getByRole('button', { name: 'Bot 明细', exact: true }).click()
    await expect(page.getByRole('region', { name: 'Bot 预览', exact: true })).toContainText('共享能力新版')
    await expect(page.getByRole('region', { name: 'Bot 预览', exact: true })).toContainText('三个引用者均看到新版核对说明')
    await page.getByRole('button', { name: '返回 Map', exact: true }).click()
    await expect(page.getByRole('button', { name: button, exact: true })).toHaveAttribute('aria-pressed', 'true')
    const response = await page.request.get(`${server.baseURL}/api/scheduler/maps/${filename}`)
    expect(await response.json()).toEqual(filename === 'first.json' ? first : second)
  }
  expect(inventory(maps)).toEqual(mapBytes)
  expect(Object.keys(inventory(root))).toEqual(['shared-original.json'])
  expect((await (await page.request.get(`${server.baseURL}/api/scheduler/bots`)).json()).bots).toEqual([{ ...content, id: 'shared-audit', name: '共享能力新版', plan_guide: '三个引用者均看到新版核对说明' }])
  await page.getByRole('button', { name: 'Bot 明细', exact: true }).click()
  fs.writeFileSync(path.join(maps, 'second.json'), JSON.stringify({ ...second, nodes: {} }))
  await page.getByRole('button', { name: '返回 Map', exact: true }).click()
  await expect(page.getByRole('alert')).toContainText('来源 node C 已移除')
  await expect(page.getByRole('region', { name: 'node 详情', exact: true })).toHaveCount(0)
  await page.getByRole('button', { name: 'first.json', exact: true }).click()
  await page.getByRole('button', { name: '核对账目 A', exact: true }).click()
  await page.getByRole('button', { name: 'Bot 明细', exact: true }).click()
  fs.unlinkSync(path.join(maps, 'first.json'))
  await page.getByRole('button', { name: '返回 Map', exact: true }).click()
  await expect(page.getByRole('alert')).toContainText('来源 Map 已失效')
  await expect(page.getByRole('region', { name: 'Map 预览', exact: true })).toHaveCount(0)
  expect(server.herdrCalls()).toEqual([])
})
