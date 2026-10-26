import fs from 'node:fs'
import path from 'node:path'
import { execFileSync } from 'node:child_process'
import { expect, test, type Page } from '@playwright/test'
import { startServer, type E2EServer } from './support'

let server: E2EServer | null = null
test.afterEach(async () => { await server?.cleanup(); server = null })
const template = { schema_version: 1, id: 'shared', title: '共享审计', use_when: '', description: '仅核对账目', inputs: {}, outputs: {}, implementation: { type: 'chat' } }
const node = { title: '审计任务', node_template_id: 'shared', parameters: { id: { nested: true } }, blocked_by: [] }
const map = (nodes: Record<string, unknown>) => ({ schema_version: 1, title: '审计计划', description: '不执行工作', nodes })
async function prepare() {
  server = await startServer({ separateCwd: true, herdr: {} })
  const templates = path.join(server.cwd, '.data/node-templates'), maps = path.join(server.cwd, '.data/workflow-maps')
  fs.mkdirSync(templates, { recursive: true }); fs.mkdirSync(maps, { recursive: true })
  const file = path.join(templates, 'business-name.JSON')
  fs.writeFileSync(file, JSON.stringify(template) + '\n')
  return { templates, maps, file }
}
async function open(page: Page) {
  await page.goto(server!.baseURL)
  await page.getByTestId('tab-scheduler').click()
  await page.getByRole('button', { name: '模板管理', exact: true }).click()
  await page.getByRole('button', { name: '共享审计 · shared', exact: true }).click()
}
const api = (page: Page, operation: string, data: unknown) => page.request.post(`${server!.baseURL}/api/scheduler/templates/${operation}`, { data })

// Approved seams S1 and S2. P21's independent expectation is both literal
// Map/node locations and byte-preserved definitions. A current-Map-only check
// (or a check only in the page) must fail this scenario; the server is real.
test('P21 cross-Map references block both page and direct final deletion and list every location', async ({ page }) => {
  const { maps, file } = await prepare()
  const before = fs.readFileSync(file, 'utf8')
  const first = path.join(maps, 'north.json'), second = path.join(maps, 'south.json')
  const north = JSON.stringify(map({ accountA: node, accountB: node })), south = JSON.stringify(map({ auditZ: node }))
  fs.writeFileSync(first, north); fs.writeFileSync(second, south)
  await open(page)
  let confirmations = 0
  page.on('dialog', async dialog => { confirmations++; await dialog.dismiss() })
  await page.getByRole('button', { name: '删除模板', exact: true }).click()
  const alert = page.getByRole('alert')
  for (const location of ['north.json', 'accountA', 'accountB', 'south.json', 'auditZ']) await expect(alert).toContainText(location)
  expect(confirmations).toBe(0)
  for (const operation of ['check-delete', 'delete']) {
    const response = await api(page, operation, operation === 'delete' ? { id: 'shared', revision: { file: 'business-name.JSON', original: before } } : { id: 'shared' })
    expect(response.ok()).toBe(false)
    const value = await response.json()
    expect(value.code).toBe('template-referenced')
    for (const location of ['north.json', 'accountA', 'accountB', 'south.json', 'auditZ']) expect(value.message).toContain(location)
  }
  expect(fs.readFileSync(file, 'utf8')).toBe(before)
  expect(fs.readFileSync(first, 'utf8')).toBe(north); expect(fs.readFileSync(second, 'utf8')).toBe(south)
  expect((await (await page.request.get(`${server!.baseURL}/api/scheduler/templates`)).json()).templates).toEqual([template])
  expect(server!.herdrCalls()).toEqual([])
})

// S1/S2 P22: real parse failure, file ReadData denial, and directory ListDirectory
// denial are distinct conditions. Treating any of them as an empty Map set
// would show a final confirmation or remove the independently known target.
test('P22 damaged and unreadable Maps and unreadable directory block deletion with the failing path and reason', async ({ page }) => {
  const { maps, file } = await prepare()
  const before = fs.readFileSync(file, 'utf8')
  const broken = path.join(maps, 'broken.json'), unreadable = path.join(maps, 'private.json')
  const user = execFileSync('whoami.exe', [], { encoding: 'utf8' }).trim()
  await open(page)
  let confirmations = 0
  page.on('dialog', async dialog => { confirmations++; await dialog.dismiss() })
  async function refused(location: string, reason: string) {
    await page.getByRole('button', { name: '删除模板', exact: true }).click()
    const alert = page.getByRole('alert').filter({ hasText: '无法完整检查' })
    // Rust preserves configured '/' segments while Node path.join uses '\\'.
    // Compare the complete independently owned path without fixing spelling.
    await expect.poll(async () => (await alert.textContent())?.replaceAll('\\', '/')).toContain(location.replaceAll('\\', '/'))
    await expect(alert).toContainText(reason)
    expect(confirmations).toBe(0)
    for (const operation of ['check-delete', 'delete']) {
      const response = await api(page, operation, operation === 'delete' ? { id: 'shared', revision: { file: 'business-name.JSON', original: before } } : { id: 'shared' })
      expect(response.ok()).toBe(false)
      const value = await response.json()
      expect(value.code).toBe('template-reference-check-failed')
      expect(value.message.replaceAll('\\', '/')).toContain(location.replaceAll('\\', '/'))
      expect(value.message).toContain(reason)
    }
    expect(fs.readFileSync(file, 'utf8')).toBe(before)
    await expect(page.getByRole('region', { name: '模板预览', exact: true })).toContainText('shared')
    expect((await (await page.request.get(`${server!.baseURL}/api/scheduler/templates`)).json()).templates).toEqual([template])
  }
  fs.writeFileSync(broken, '{ broken JSON')
  await refused('broken.json', '结构无效')
  expect(fs.readFileSync(broken, 'utf8')).toBe('{ broken JSON')
  fs.unlinkSync(broken)
  const privateBytes = JSON.stringify(map({}))
  fs.writeFileSync(unreadable, privateBytes)
  try {
    execFileSync('icacls.exe', [unreadable, '/deny', `${user}:(RD)`])
    expect(() => fs.readFileSync(unreadable)).toThrow()
    expect(fs.readdirSync(maps)).toEqual(['private.json'])
    await refused('private.json', '读取失败')
  } finally { execFileSync('icacls.exe', [unreadable, '/remove:d', user]) }
  expect(fs.readFileSync(unreadable, 'utf8')).toBe(privateBytes)
  try {
    execFileSync('icacls.exe', [maps, '/deny', `${user}:(RD)`])
    expect(() => fs.readdirSync(maps)).toThrow()
    await refused(maps, '读取失败')
  } finally { execFileSync('icacls.exe', [maps, '/remove:d', user]) }
  expect(fs.readFileSync(unreadable, 'utf8')).toBe(privateBytes)
  expect(server!.herdrCalls()).toEqual([])
})

// S1/S2 P23/P29: cancellation must issue no final request; actual confirmation
// removes the real file and returns a fresh list. Two literal identities expose
// both ordinary deletion and last-item empty state without refreshing the page.
test('P23 P29 cancel preserves a template, confirm returns the updated list and deleting the last template shows empty state', async ({ page }) => {
  const { templates, file } = await prepare()
  const before = fs.readFileSync(file, 'utf8')
  const other = { ...template, id: 'remaining', title: '保留能力' }
  const otherFile = path.join(templates, 'remaining.json')
  const otherBytes = JSON.stringify(other)
  fs.writeFileSync(otherFile, otherBytes)
  await open(page)
  const requests: string[] = []
  page.on('request', request => {
    if (/\/templates\/(check-delete|delete)$/.test(request.url())) requests.push(request.url().split('/').at(-1)!)
  })
  const button = page.getByRole('button', { name: '删除模板', exact: true })
  page.once('dialog', async dialog => {
    expect(dialog.type()).toBe('confirm')
    expect(dialog.message()).toContain('shared')
    expect(fs.readFileSync(file, 'utf8')).toBe(before)
    await dialog.dismiss()
  })
  await button.click()
  await expect(button).toBeEnabled()
  expect(requests).toEqual(['check-delete'])
  expect(fs.readFileSync(file, 'utf8')).toBe(before)
  expect((await (await page.request.get(`${server!.baseURL}/api/scheduler/templates`)).json()).templates).toEqual([template, other])
  // A direct caller must supply a current target revision, not just an ID.
  const bypass = await api(page, 'delete', { id: 'shared' })
  expect(bypass.ok()).toBe(false)
  expect(fs.readFileSync(file, 'utf8')).toBe(before)
  page.once('dialog', dialog => dialog.accept())
  await button.click()
  const list = page.getByRole('region', { name: '共享模板列表', exact: true })
  await expect(list.getByRole('button', { name: '保留能力 · remaining', exact: true })).toBeVisible()
  await expect(page.getByRole('region', { name: '模板预览', exact: true })).toHaveCount(0)
  await expect(list.getByRole('button', { name: '共享审计 · shared', exact: true })).toHaveCount(0)
  expect(requests).toEqual(['check-delete', 'check-delete', 'delete'])
  expect(fs.existsSync(file)).toBe(false)
  expect(fs.readFileSync(otherFile, 'utf8')).toBe(otherBytes)
  expect((await (await page.request.get(`${server!.baseURL}/api/scheduler/templates`)).json()).templates).toEqual([other])
  expect((await api(page, 'edit', { id: 'shared' })).ok()).toBe(false)
  await list.getByRole('button', { name: '保留能力 · remaining', exact: true }).click()
  page.once('dialog', dialog => dialog.accept())
  await button.click()
  await expect(page.getByTestId('scheduler-template-empty')).toBeVisible()
  await expect(page.getByRole('region', { name: '模板预览', exact: true })).toHaveCount(0)
  expect(fs.readdirSync(templates)).toEqual([])
  expect((await (await page.request.get(`${server!.baseURL}/api/scheduler/templates`)).json()).templates).toEqual([])
  expect(server!.herdrCalls()).toEqual([])
})

// S1/S2: deny both target Delete and parent DeleteChild while retaining reads.
// Reporting success or navigating to an empty list would hide the real failure.
test('a real filesystem deletion failure keeps the template preview and reports the error without repair', async ({ page }) => {
  const { file, templates } = await prepare()
  const before = fs.readFileSync(file, 'utf8')
  const user = execFileSync('whoami.exe', [], { encoding: 'utf8' }).trim()
  await open(page)
  try {
    execFileSync('icacls.exe', [file, '/deny', `${user}:(DE)`])
    execFileSync('icacls.exe', [templates, '/deny', `${user}:(DC)`])
    page.once('dialog', dialog => dialog.accept())
    await page.getByRole('button', { name: '删除模板', exact: true }).click()
    await expect(page.getByRole('alert')).toContainText('删除失败')
    await expect(page.getByRole('region', { name: '模板预览', exact: true })).toContainText('shared')
    await expect(page.getByRole('region', { name: '共享模板列表', exact: true })).toHaveCount(0)
    const target = await (await api(page, 'check-delete', { id: 'shared' })).json()
    const failed = await api(page, 'delete', target)
    expect(failed.ok()).toBe(false)
    expect(await failed.json()).toMatchObject({ code: 'template-delete-failed', message: expect.stringContaining('business-name.JSON') })
    expect(fs.readFileSync(file, 'utf8')).toBe(before)
    expect((await (await page.request.get(`${server!.baseURL}/api/scheduler/templates`)).json()).templates).toEqual([template])
    expect(server!.herdrCalls()).toEqual([])
  } finally {
    execFileSync('icacls.exe', [file, '/remove:d', user])
    execFileSync('icacls.exe', [templates, '/remove:d', user])
  }
})

// S2 and S1: the approved ordering permits delete followed by saving a missing
// template reference. Safety must not silently strengthen save validation.
test('deleting an unused template still permits a later node with that missing template reference', async ({ page }) => {
  const { file, maps } = await prepare()
  fs.writeFileSync(path.join(maps, 'later.json'), JSON.stringify(map({})))
  const target = await (await api(page, 'check-delete', { id: 'shared' })).json()
  const deleted = await api(page, 'delete', target)
  expect(deleted.status()).toBe(200)
  expect(await deleted.json()).toEqual({ templates: [] })
  expect(fs.existsSync(file)).toBe(false)
  const base = `${server!.baseURL}/api/scheduler/maps/later.json/nodes`
  const draft = await (await page.request.get(`${base}/new`)).json()
  const saved = await page.request.post(`${base}/save`, { data: { id: 'allowedMissing', content: JSON.stringify(node), revision: draft.revision, create: true } })
  expect(saved.status()).toBe(200)
  expect(await (await page.request.get(`${server!.baseURL}/api/scheduler/maps/later.json`)).json()).toEqual(map({ allowedMissing: node }))
  expect(JSON.parse(fs.readFileSync(path.join(maps, 'later.json'), 'utf8'))).toEqual(map({ allowedMissing: node }))
  await page.goto(server!.baseURL)
  await page.getByTestId('tab-scheduler').click()
  await page.getByRole('button', { name: 'later.json', exact: true }).click()
  await page.getByRole('region', { name: '节点列表', exact: true }).getByRole('button', { name: '审计任务 allowedMissing', exact: true }).click()
  await expect(page.getByRole('region', { name: 'node 详情', exact: true }).getByRole('alert')).toContainText('缺失共享模板：shared')
  expect(fs.existsSync(file)).toBe(false)
  expect(server!.herdrCalls()).toEqual([])
})

// S1/S2 P24: modify the real definitions while the native confirmation is open.
// Reusing an earlier no-reference result or selecting the first duplicate would
// remove a file here. The final production request must instead reject it.
for (const change of ['target edit', 'target removal', 'new reference', 'ambiguous ID'] as const) {
  test(`P24 confirmation cannot authorize deletion after ${change}`, async ({ page }) => {
    const { templates, maps, file } = await prepare()
    await open(page)
    const before = fs.readFileSync(file, 'utf8')
    const preflight = await api(page, 'check-delete', { id: 'shared' })
    expect(preflight.status()).toBe(200)
    const target = await preflight.json()
    expect(target).toEqual({ id: 'shared', revision: { file: 'business-name.JSON', original: before } })
    const duplicate = path.join(templates, 'duplicate.json'), late = path.join(maps, 'late.json')
    const changed = JSON.stringify({ ...template, description: '外部编辑的新版本' })
    const duplicateBytes = JSON.stringify({ ...template, title: '重复身份，不得任选' })
    const lateBytes = JSON.stringify(map({ lateNode: node }))
    const diagnostic = change === 'ambiguous ID' ? '冲突' : change === 'new reference' ? 'late.json / node lateNode' : '变化或移除'
    page.once('dialog', async dialog => {
      expect(dialog.type()).toBe('confirm')
      if (change === 'target edit') fs.writeFileSync(file, changed)
      if (change === 'target removal') fs.unlinkSync(file)
      if (change === 'new reference') fs.writeFileSync(late, lateBytes)
      if (change === 'ambiguous ID') fs.writeFileSync(duplicate, duplicateBytes)
      await dialog.accept()
    })
    await page.getByRole('button', { name: '删除模板', exact: true }).click()
    await expect(page.getByRole('alert').filter({ hasText: diagnostic })).toBeVisible()
    await expect(page.getByRole('region', { name: '模板预览', exact: true })).toBeVisible()
    const direct = await api(page, 'delete', target)
    expect(direct.ok()).toBe(false)
    expect((await direct.json()).message).toContain(diagnostic)
    if (change === 'target removal') expect(fs.existsSync(file)).toBe(false)
    else expect(fs.readFileSync(file, 'utf8')).toBe(change === 'target edit' ? changed : before)
    const catalog = await (await page.request.get(`${server!.baseURL}/api/scheduler/templates`)).json()
    if (change === 'ambiguous ID') {
      expect(fs.readFileSync(duplicate, 'utf8')).toBe(duplicateBytes)
      expect(catalog.templates).toEqual([])
      expect(catalog.problems).toContainEqual(expect.objectContaining({ code: 'template-id-conflict', template_id: 'shared', files: ['business-name.JSON', 'duplicate.json'] }))
    } else expect(catalog.templates).toEqual(change === 'target removal' ? [] : [{ ...template, ...(change === 'target edit' ? { description: '外部编辑的新版本' } : {}) }])
    if (change === 'new reference') {
      expect(fs.readFileSync(late, 'utf8')).toBe(lateBytes)
      expect(await (await page.request.get(`${server!.baseURL}/api/scheduler/maps/late.json`)).json()).toEqual(map({ lateNode: node }))
    }
    expect(server!.herdrCalls()).toEqual([])
  })
}
