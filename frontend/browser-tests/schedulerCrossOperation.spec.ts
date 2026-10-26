import fs from 'node:fs'
import path from 'node:path'
import { expect, request, test, type APIRequestContext, type APIResponse, type Page, type TestInfo } from '@playwright/test'
import { startServer, type E2EServer } from './support'

let server: E2EServer | null = null
test.afterEach(async () => { await server?.cleanup(); server = null })
const capability = { schema_version: 1, name: '账目核对能力', use_when: '人工核对', description: '仅保存定义', inputs: {}, outputs: {}, implementation: { type: 'chat' } }
const work = { title: '核对工作', bot_id: 'audit', parameters: { amount: 37, nested: [null, true] }, blocked_by: [] as string[] }
const plan = { schema_version: 1, title: '季度核对', description: '静态定义不是执行计划', nodes: {} }
const botsAPI = '/api/scheduler/bots'
const mapAPI = '/api/scheduler/maps/quarter.JSON'
const nodesAPI = `${mapAPI}/nodes`
const roots = () => ({ maps: path.join(server!.cwd, '.data/workflow-maps'), bots: path.join(server!.cwd, '.data/bots') })

// S1/S2/P32: specify retained and removed identities/content before mutation.
// An in-memory success, losing an edit on restart, or resurrecting a deleted
// object must disagree with exact real files, production rereads AND the page.
test('P32 complete bot and node CRUD sequence survives a real service restart', async ({ page, request: client }) => {
  const { maps, bots } = await prepare()
  const retained = { ...capability, name: '保留核对能力', description: '重启后仍保留修订二', inputs: { ledger: '账册说明' } }
  const removed = { ...capability, name: '删除的临时能力', description: '修订后删除' }
  const retainedWork = { ...work, title: '保留核对工作', bot_id: 'retained', parameters: { amount: 73, nested: [false, { city: '上海' }] } }
  const removedWork = { ...work, title: '删除的临时工作', bot_id: 'removed' }
  const expectedCatalog = [{ ...capability, id: 'audit' }, { ...retained, id: 'retained' }]
  const expectedMap = { ...plan, nodes: { retainedNode: retainedWork } }
  async function persistedMap(expected: unknown) {
    expect(await read(client, mapAPI)).toEqual(expected)
    expect(JSON.parse(fs.readFileSync(path.join(maps, 'quarter.JSON'), 'utf8'))).toEqual(expected)
  }
  async function persistedBot(id: string, expected: unknown) {
    const draft = await accepted(client, `${botsAPI}/edit`, { id })
    expect(JSON.parse(fs.readFileSync(path.join(bots, draft.revision.file), 'utf8'))).toEqual(expected)
    expect((await read(client, botsAPI)).bots).toContainEqual(expected)
    return draft.revision.file as string
  }
  const files: Record<string, string> = {}
  for (const [id, finalContent] of [['retained', retained], ['removed', removed]] as const) {
    const created = { ...capability, name: `初版 ${id}` }
    await accepted(client, `${botsAPI}/save`, { id, content: JSON.stringify(created), revision: null })
    files[id] = await persistedBot(id, { ...created, id })
    const draft = await accepted(client, `${botsAPI}/edit`, { id })
    await accepted(client, `${botsAPI}/save`, { id, content: JSON.stringify(finalContent), revision: draft.revision })
    expect(await persistedBot(id, { ...finalContent, id })).toBe(files[id])
  }
  const nodes: Record<string, unknown> = {}
  for (const [id, finalContent] of [['retainedNode', retainedWork], ['removedNode', removedWork]] as const) {
    const draft = await read(client, `${nodesAPI}/new`)
    const created = { ...work, title: `初版 ${id}` }
    await accepted(client, `${nodesAPI}/save`, { id, content: JSON.stringify(created), revision: draft.revision, create: true })
    nodes[id] = created
    await persistedMap({ ...plan, nodes })
    const editing = await accepted(client, `${nodesAPI}/edit`, { id })
    await accepted(client, `${nodesAPI}/save`, { id, content: JSON.stringify(finalContent), revision: editing.revision, create: false })
    nodes[id] = finalContent
    await persistedMap({ ...plan, nodes })
  }
  const nodeTarget = await accepted(client, `${nodesAPI}/check-delete`, { id: 'removedNode' })
  await accepted(client, `${nodesAPI}/delete`, nodeTarget)
  await persistedMap(expectedMap)
  const botTarget = await accepted(client, `${botsAPI}/check-delete`, { id: 'removed' })
  await accepted(client, `${botsAPI}/delete`, botTarget)
  expect(fs.existsSync(path.join(bots, files.removed))).toBe(false)
  expect(Object.fromEntries((await read(client, botsAPI)).bots.map((bot: { id: string }) => [bot.id, bot]))).toEqual(Object.fromEntries(expectedCatalog.map(bot => [bot.id, bot])))
  await server!.restart()
  await persistedMap(expectedMap)
  expect(Object.fromEntries((await read(client, botsAPI)).bots.map((bot: { id: string }) => [bot.id, bot]))).toEqual(Object.fromEntries(expectedCatalog.map(bot => [bot.id, bot])))
  expect(JSON.parse(fs.readFileSync(path.join(bots, files.retained), 'utf8'))).toEqual({ ...retained, id: 'retained' })
  expect(fs.existsSync(path.join(bots, files.removed))).toBe(false)
  expect(fs.readdirSync(bots).sort()).toEqual(['business-name.JSON', files.retained].sort())
  await page.goto(server!.baseURL)
  await page.getByTestId('tab-scheduler').click()
  await page.getByRole('button', { name: 'quarter.JSON', exact: true }).click()
  await page.getByRole('region', { name: '节点列表', exact: true }).getByRole('button', { name: '保留核对工作 retainedNode', exact: true }).click()
  await expect(page.getByRole('region', { name: 'node 详情', exact: true })).toContainText('73')
  await page.getByRole('button', { name: 'Bot 明细', exact: true }).click()
  await expect(page.getByRole('region', { name: 'Bot 预览', exact: true })).toContainText('重启后仍保留修订二')
  await expect(page.getByRole('region', { name: '共享 Bot 列表', exact: true })).toBeVisible()
  await expect(page.getByRole('region', { name: '共享 Bot 列表', exact: true })).not.toContainText('删除的临时能力')
  expect(server!.herdrCalls()).toEqual([])
})
async function prepare(nodes: Record<string, unknown> = {}) {
  server = await startServer({ separateCwd: true, herdr: {}, safeShutdown: true })
  const { maps, bots } = roots()
  fs.mkdirSync(maps, { recursive: true }); fs.mkdirSync(bots, { recursive: true })
  fs.writeFileSync(path.join(maps, 'quarter.JSON'), JSON.stringify({ ...plan, nodes }))
  fs.writeFileSync(path.join(bots, 'business-name.JSON'), JSON.stringify({ ...capability, id: 'audit' }))
  return { maps, bots }
}
const post = (client: APIRequestContext, endpoint: string, data: unknown) => client.post(`${server!.baseURL}${endpoint}`, { data })
async function accepted(client: APIRequestContext, endpoint: string, data: unknown) {
  const response = await post(client, endpoint, data)
  expect(response.ok(), await response.text()).toBe(true)
  return response.json()
}
async function evidence(info: TestInfo, name: string, value: unknown) {
  const file = info.outputPath(`${name}.json`)
  fs.writeFileSync(file, JSON.stringify(value, null, 2))
  await info.attach(name, { path: file, contentType: 'application/json' })
}
async function read(client: APIRequestContext, endpoint: string) {
  const response = await client.get(`${server!.baseURL}${endpoint}`)
  expect(response.ok(), await response.text()).toBe(true)
  return response.json()
}
// S2/P33: actual outside bytes are a prewritten independent secret, not a
// response-derived expectation. Sanitizing only the page or '/' would expose
// valid outside JSON through Windows separators, a forged revision, or a junction.
test('P33 direct read save and delete requests cannot escape through paths or linked entries', async ({ request: client }, info) => {
  const { maps, bots } = await prepare({ A: work })
  const outside = path.join(server!.cwd, 'outside')
  fs.mkdirSync(outside)
  const secretMap = { ...plan, title: 'PRIVATE MAP SENTINEL 73691', nodes: { A: work } }
  const secretBot = { ...capability, id: 'outside-only', description: 'PRIVATE TEMPLATE SENTINEL 28463' }
  const mapFile = path.join(outside, 'secret-map.json'), botFile = path.join(outside, 'secret-template.json')
  const originals = new Map([[mapFile, JSON.stringify(secretMap)], [botFile, JSON.stringify(secretBot)]])
  for (const [file, bytes] of originals) fs.writeFileSync(file, bytes)
  fs.symlinkSync(outside, path.join(maps, 'linked.json'), 'junction')
  fs.symlinkSync(outside, path.join(bots, 'linked.json'), 'junction')
  // These controls establish genuine links to readable valid outside files.
  expect(fs.readFileSync(path.join(maps, 'linked.json/secret-map.json'), 'utf8')).toBe(originals.get(mapFile))
  expect(fs.readFileSync(path.join(bots, 'linked.json/secret-template.json'), 'utf8')).toBe(originals.get(botFile))
  const mapBefore = fs.readFileSync(path.join(maps, 'quarter.JSON'), 'utf8')
  const botBefore = fs.readFileSync(path.join(bots, 'business-name.JSON'), 'utf8')
  const nodeDraft = await accepted(client, `${nodesAPI}/edit`, { id: 'A' })
  const botDraft = await accepted(client, `${botsAPI}/edit`, { id: 'audit' })
  async function rejected(response: APIResponse) {
    expect(response.ok()).toBe(false)
    const text = await response.text()
    expect(text).not.toContain('PRIVATE MAP SENTINEL 73691')
    expect(text).not.toContain('PRIVATE TEMPLATE SENTINEL 28463')
    for (const [file, bytes] of originals) expect(fs.readFileSync(file, 'utf8')).toBe(bytes)
  }
  const names = [mapFile, '../outside/secret-map.json', '..\\outside\\secret-map.json', 'quarter.JSON:stream', 'linked.json', 'linked.json/secret-map.json', 'linked.json\\secret-map.json']
  for (const name of names) {
    const endpoint = `/api/scheduler/maps/${encodeURIComponent(name)}`
    await rejected(await client.get(`${server!.baseURL}${endpoint}`))
    await rejected(await client.get(`${server!.baseURL}${endpoint}/nodes/new`))
    for (const operation of ['edit', 'check-delete', 'save', 'delete']) {
      const data = operation === 'save' ? { id: 'A', content: JSON.stringify({ ...work, title: 'must not overwrite outside' }), revision: { file: name, original: originals.get(mapFile) }, create: false }
        : operation === 'delete' ? { id: 'A', revision: { file: name, original: originals.get(mapFile) } } : { id: 'A' }
      await rejected(await post(client, `${endpoint}/nodes/${operation}`, data))
    }
    // A valid selected URL cannot be redirected by the revision's file field.
    await rejected(await post(client, `${nodesAPI}/save`, { ...nodeDraft, content: JSON.stringify(work), revision: { file: name, original: mapBefore }, create: false }))
    await rejected(await post(client, `${nodesAPI}/delete`, { id: 'A', revision: { file: name, original: mapBefore } }))
    await rejected(await post(client, `${botsAPI}/save`, { id: 'audit', content: JSON.stringify(capability), revision: { file: name, original: botBefore } }))
    await rejected(await post(client, `${botsAPI}/delete`, { id: 'audit', revision: { file: name, original: botBefore } }))
  }
  for (const endpoint of [botsAPI, mapAPI]) {
    await rejected(await client.get(`${server!.baseURL}${endpoint}?path=${encodeURIComponent(botFile)}`))
  }
  for (const operation of ['edit', 'check-delete', 'delete']) {
    await rejected(await post(client, `${botsAPI}/${operation}`, operation === 'delete' ? { id: 'outside-only', revision: { file: botFile, original: originals.get(botFile) } } : { id: 'outside-only' }))
  }
  // Bot identity text may contain paths; it must be data, not a filename.
  const pathId = '../outside/secret-template.json'
  await accepted(client, `${botsAPI}/save`, { id: pathId, content: JSON.stringify(capability), revision: null })
  const created = await accepted(client, `${botsAPI}/edit`, { id: pathId })
  expect(created.content).toEqual(capability)
  expect(path.dirname(path.join(bots, created.revision.file))).toBe(bots)
  await accepted(client, `${botsAPI}/delete`, { id: pathId, revision: created.revision })
  expect(fs.readFileSync(path.join(maps, 'quarter.JSON'), 'utf8')).toBe(mapBefore)
  expect(fs.readFileSync(path.join(bots, botDraft.revision.file), 'utf8')).toBe(botBefore)
  for (const [file, bytes] of originals) expect(fs.readFileSync(file, 'utf8')).toBe(bytes)
  expect(fs.readdirSync(outside).sort()).toEqual(['secret-map.json', 'secret-template.json'])
  await evidence(info, 'platform-path-boundary', { platform: process.platform, directoryJunctions: true, fileSymlinks: 'separate privilege-aware scenario', names })
})

// File symbolic links are manual-only; ordinary runs keep junction coverage
// without creating privileged links or requesting elevation.
test('P33 file symbolic links cannot expose or mutate outside definitions', async ({ request: client }, info) => {
  test.skip(process.env.HERDR_BOT_MANUAL_SYMLINKS !== '1', 'Manual-only: set HERDR_BOT_MANUAL_SYMLINKS=1 to exercise real symbolic links')
  const { maps, bots } = await prepare()
  const outside = path.join(server!.cwd, 'outside')
  fs.mkdirSync(outside)
  const mapBytes = JSON.stringify({ ...plan, title: 'PRIVATE SYMLINK MAP 4721', nodes: { A: work } })
  const botBytes = JSON.stringify({ ...capability, id: 'outside-only', name: 'PRIVATE SYMLINK TEMPLATE 6942' })
  const sourceMap = path.join(outside, 'map.json'), sourceBot = path.join(outside, 'bot.json')
  fs.writeFileSync(sourceMap, mapBytes); fs.writeFileSync(sourceBot, botBytes)
  try {
    fs.symlinkSync(sourceMap, path.join(maps, 'alias.json'), 'file')
    fs.symlinkSync(sourceBot, path.join(bots, 'alias.json'), 'file')
  } catch (error) {
    if (!(error instanceof Error) || !('code' in error) || !['EPERM', 'EACCES'].includes(String(error.code))) throw error
    await evidence(info, 'file-symlink-unavailable', { platform: process.platform, reason: String(error) })
    test.skip(true, `File symlink privilege unavailable: ${String(error)}`)
  }
  expect(fs.readFileSync(path.join(maps, 'alias.json'), 'utf8')).toBe(mapBytes)
  expect(fs.readFileSync(path.join(bots, 'alias.json'), 'utf8')).toBe(botBytes)
  const endpoint = '/api/scheduler/maps/alias.json'
  const rejected: APIResponse[] = [await client.get(`${server!.baseURL}${endpoint}`), await client.get(`${server!.baseURL}${endpoint}/nodes/new`)]
  for (const operation of ['edit', 'check-delete', 'save', 'delete']) {
    rejected.push(await post(client, `${endpoint}/nodes/${operation}`, operation === 'save'
      ? { id: 'A', content: JSON.stringify(work), revision: { file: 'alias.json', original: mapBytes }, create: false }
      : operation === 'delete' ? { id: 'A', revision: { file: 'alias.json', original: mapBytes } } : { id: 'A' }))
    rejected.push(await post(client, `${botsAPI}/${operation}`, operation === 'save'
      ? { id: 'outside-only', content: JSON.stringify(capability), revision: { file: 'alias.json', original: botBytes } }
      : operation === 'delete' ? { id: 'outside-only', revision: { file: 'alias.json', original: botBytes } } : { id: 'outside-only' }))
  }
  for (const response of rejected) {
    expect(response.ok()).toBe(false)
    expect(await response.text()).not.toMatch(/PRIVATE SYMLINK (MAP 4721|TEMPLATE 6942)/)
  }
  expect(await read(client, botsAPI)).toEqual({ bots: [{ ...capability, id: 'audit' }] })
  expect(fs.readFileSync(sourceMap, 'utf8')).toBe(mapBytes)
  expect(fs.readFileSync(sourceBot, 'utf8')).toBe(botBytes)
  expect(fs.existsSync(path.join(maps, 'alias.json'))).toBe(true)
  expect(fs.existsSync(path.join(bots, 'alias.json'))).toBe(true)
  await evidence(info, 'file-symlink-covered', { platform: process.platform, reads: true, saves: true, deletes: true })
})

// S2/P33: keep the real ancestor checks in every read/write path. A stale
// revision from a valid directory must not authorize mutation after configuration
// points through a real Windows reparse point (including a missing child).
for (const kind of ['maps', 'bots'] as const) {
  for (const location of ['root', 'ancestor'] as const) {
    test(`P33 ${kind} ${location} junction blocks production reads saves and deletes`, async ({ request: client }) => {
      await prepare({ A: work })
      const outside = path.join(server!.cwd, 'outside'), alias = path.join(server!.cwd, 'linked-root')
      fs.mkdirSync(outside)
      const name = kind === 'maps' ? 'quarter.JSON' : 'business-name.JSON'
      const content = kind === 'maps' ? { ...plan, title: 'PRIVATE ROOT SENTINEL 5821', nodes: { A: work } } : { ...capability, id: 'audit', description: 'PRIVATE ROOT SENTINEL 5821' }
      const bytes = JSON.stringify(content)
      fs.writeFileSync(path.join(outside, name), bytes)
      fs.symlinkSync(outside, alias, 'junction')
      expect(fs.readFileSync(path.join(alias, name), 'utf8')).toBe(bytes)
      const endpoint = kind === 'maps' ? nodesAPI : botsAPI, id = kind === 'maps' ? 'A' : 'audit'
      const draft = await accepted(client, `${endpoint}/edit`, { id })
      fs.appendFileSync(server!.configPath, `\n[scheduler]\n${kind === 'maps' ? 'map_directory' : 'bot_directory'} = 'linked-root${location === 'ancestor' ? '/missing/definitions' : ''}'\n`)
      await server!.restart()
      const receipts: APIResponse[] = [await client.get(`${server!.baseURL}${kind === 'maps' ? mapAPI : botsAPI}`)]
      if (kind === 'maps') receipts.push(await client.get(`${server!.baseURL}${nodesAPI}/new`))
      for (const operation of ['edit', 'check-delete', 'save', 'delete']) {
        receipts.push(await post(client, `${endpoint}/${operation}`, operation === 'save'
          ? { id, content: JSON.stringify(kind === 'maps' ? work : capability), revision: draft.revision, ...(kind === 'maps' ? { create: false } : {}) }
          : operation === 'delete' ? { id, revision: draft.revision } : { id }))
      }
      if (kind === 'bots') receipts.push(await post(client, `${botsAPI}/save`, { id: 'must-not-create', content: JSON.stringify(capability), revision: null }))
      for (const response of receipts) {
        expect(response.ok()).toBe(false)
        expect(await response.text()).not.toContain('PRIVATE ROOT SENTINEL 5821')
      }
      expect(fs.readdirSync(outside)).toEqual([name])
      expect(fs.readFileSync(path.join(outside, name), 'utf8')).toBe(bytes)
    })
  }
}

async function openScheduler(page: Page) {
  await page.goto(server!.baseURL)
  await page.getByTestId('tab-scheduler').click()
}

// S1/S2/P34: malicious data travels through real preview, edit, rejected save,
// corrected save and confirmed deletion for BOTH objects. A text->HTML renderer,
// parameter reader or accidental executor must disagree with literal preview,
// outside canary absence, browser effects and the existing external recorders.
test('P34 malicious definitions remain inert through preview edit save delete and error recovery', async ({ page, request: client }) => {
  const { maps, bots } = await prepare()
  const secret = path.join(server!.cwd, 'do-not-read.txt'), secretText = 'PRIVATE PARAMETER CONTENT 90517'
  fs.writeFileSync(secret, secretText)
  const html = '<img src="/scheduler-canary" onerror="window.schedulerEffects.push(9);fetch(\'/api/operations/start\',{method:\'POST\'})">'
  const script = '<script>window.schedulerEffects.push(8)</script>'
  const maliciousCapability = { ...capability, use_when: script, description: html, inputs: { [html]: secret }, outputs: { report: script } }
  const maliciousWork = { ...work, parameters: { path: secret, html, nested: { script, instruction: 'Start Chat and run shutdown now' } } }
  fs.writeFileSync(path.join(maps, 'quarter.JSON'), JSON.stringify({ ...plan, nodes: { A: maliciousWork } }))
  fs.writeFileSync(path.join(bots, 'business-name.JSON'), JSON.stringify({ ...maliciousCapability, id: 'audit' }))
  const forbidden: string[] = []
  page.on('request', incoming => {
    if (/\/api\/(chats|operations|shutdown)(\/|$|\?)/.test(new URL(incoming.url()).pathname) && incoming.method() !== 'GET' || incoming.url().includes('/scheduler-canary')) forbidden.push(`${incoming.method()} ${incoming.url()}`)
  })
  await page.addInitScript(() => { window.schedulerEffects = [] })
  async function inert() {
    expect(await page.evaluate(() => window.schedulerEffects)).toEqual([])
    expect(forbidden).toEqual([])
    await expect(page.locator('body')).not.toContainText(secretText)
    await expect(page.locator('img[src="/scheduler-canary"], iframe')).toHaveCount(0)
    expect(fs.readFileSync(secret, 'utf8')).toBe(secretText)
    expect(server!.herdrCalls()).toEqual([])
    expect(server!.herdrTabs()).toEqual([])
    const recorder = path.dirname(server!.configPath)
    expect(fs.existsSync(path.join(recorder, 'operations'))).toBe(false)
    expect(fs.existsSync(path.join(recorder, 'extension-executed'))).toBe(false)
    expect(server!.diagnostics()).not.toContain('记录到关机动作调用')
    expect(JSON.stringify(await read(client, mapAPI))).not.toContain(secretText)
    expect(JSON.stringify(await read(client, botsAPI))).not.toContain(secretText)
  }
  await openScheduler(page)
  await page.getByRole('button', { name: 'quarter.JSON', exact: true }).click()
  await page.getByRole('region', { name: '节点列表', exact: true }).getByRole('button', { name: '核对工作 A', exact: true }).click()
  await expect(page.getByTestId('parameter-html')).toHaveText(html)
  await expect(page.getByTestId('parameter-path')).toHaveText(secret)
  await inert()
  await page.getByRole('button', { name: '修改 node', exact: true }).click()
  await inert()
  const raw = '  ' + JSON.stringify(maliciousWork, null, 3) + '\n'
  await page.getByLabel('node 内容 JSON', { exact: true }).fill(raw + '!')
  await page.getByRole('button', { name: '保存 node', exact: true }).click()
  await expect(page.getByRole('alert').filter({ hasText: /JSON/ })).toBeVisible()
  await expect(page.getByLabel('node 内容 JSON', { exact: true })).toHaveValue(raw + '!')
  expect(JSON.parse(fs.readFileSync(path.join(maps, 'quarter.JSON'), 'utf8')).nodes.A).toEqual(maliciousWork)
  await inert()
  const changedWork = { ...maliciousWork, title: '保存后的核对工作' }
  await page.getByLabel('node 内容 JSON', { exact: true }).fill(JSON.stringify(changedWork))
  await page.getByRole('button', { name: '保存 node', exact: true }).click()
  await expect(page.getByRole('form', { name: 'node 编辑', exact: true })).toHaveCount(0)
  await expect(page.getByTestId('parameter-html')).toHaveText(html)
  expect((await read(client, mapAPI)).nodes.A).toEqual(changedWork)
  await inert()
  await page.getByRole('button', { name: 'Bot 明细', exact: true }).click()
  await expect(page.getByRole('region', { name: 'Bot 预览', exact: true }).getByText(html, { exact: true }).first()).toBeVisible()
  await inert()
  await page.getByRole('button', { name: '修改 Bot', exact: true }).click()
  await inert()
  const invalid = JSON.stringify(maliciousCapability) + '!'
  await page.getByLabel('Bot 内容 JSON', { exact: true }).fill(invalid)
  await page.getByRole('button', { name: '保存 Bot', exact: true }).click()
  await expect(page.getByRole('alert').filter({ hasText: /JSON/ })).toBeVisible()
  await expect(page.getByLabel('Bot 内容 JSON', { exact: true })).toHaveValue(invalid)
  await inert()
  const changedCapability = { ...maliciousCapability, name: '保存后的核对能力' }
  await page.getByLabel('Bot 内容 JSON', { exact: true }).fill(JSON.stringify(changedCapability))
  await page.getByRole('button', { name: '保存 Bot', exact: true }).click()
  await expect(page.getByRole('form', { name: 'Bot 编辑', exact: true })).toHaveCount(0)
  expect((await read(client, botsAPI)).bots).toEqual([{ ...changedCapability, id: 'audit' }])
  await inert()
  // Referenced delete error is recovered through actual node deletion, not
  // by changing files behind the application or forcing the bot deletion.
  await page.getByRole('button', { name: '删除 Bot', exact: true }).click()
  await expect(page.getByRole('alert').filter({ hasText: 'quarter.JSON / node A' })).toBeVisible()
  await inert()
  await page.getByRole('button', { name: '返回 Map', exact: true }).click()
  page.once('dialog', dialog => dialog.accept())
  await page.getByRole('button', { name: '删除 node', exact: true }).click()
  await expect(page.getByRole('region', { name: 'node 详情', exact: true })).toHaveCount(0)
  expect(await read(client, mapAPI)).toEqual(plan)
  await inert()
  await page.getByRole('button', { name: 'Bot 管理', exact: true }).click()
  await page.getByRole('button', { name: '保存后的核对能力 · audit', exact: true }).click()
  page.once('dialog', dialog => dialog.accept())
  await page.getByRole('button', { name: '删除 Bot', exact: true }).click()
  await expect(page.getByTestId('scheduler-bot-empty')).toBeVisible()
  expect(await read(client, botsAPI)).toEqual({ bots: [] })
  expect(fs.existsSync(path.join(bots, 'business-name.JSON'))).toBe(false)
  expect(JSON.parse(fs.readFileSync(path.join(maps, 'quarter.JSON'), 'utf8'))).toEqual(plan)
  await inert()
})

async function simultaneous<T>(jobs: (() => Promise<T>)[]) {
  // Every independent client is waiting before any request is released. There
  // is deliberately no await of a response before dispatching the next request.
  const gate = Promise.withResolvers<void>()
  const pending = jobs.map(async job => { await gate.promise; return job() })
  gate.resolve()
  return Promise.all(pending)
}

// Approved S2: real production requests in one isolated running instance. P25
// requires a single winner, independently specified before either request; a
// check outside the commit boundary could let both saves replace the same file.
// Both candidates use the same revision, not an ordered stale-editor scenario.
for (const kind of ['bot', 'node'] as const) {
  test(`P25 concurrent same-target ${kind} updates have one durable winner`, async ({ request: client }, info) => {
    const { maps, bots } = await prepare({ A: work })
    const endpoint = kind === 'bot' ? botsAPI : nodesAPI
    const draft = await accepted(client, `${endpoint}/edit`, { id: kind === 'bot' ? 'audit' : 'A' })
    const clients = await Promise.all([request.newContext(), request.newContext()])
    try {
      const candidates = ['独立候选甲', '独立候选乙'].map(label => ({ ...draft.content, [kind === 'bot' ? 'name' : 'title']: label }))
      const responses = await simultaneous(clients.map((other, index) => () => post(other, `${endpoint}/save`, {
        id: draft.id, content: JSON.stringify(candidates[index]), revision: draft.revision, ...(kind === 'node' ? { create: false } : {}),
      })))
      const receipts = await Promise.all(responses.map(async response => ({ status: response.status(), body: await response.json() })))
      await evidence(info, 'concurrent-receipts', receipts)
      expect(responses.filter(response => response.ok())).toHaveLength(1)
      const winner = responses.findIndex(response => response.ok())
      expect(receipts[1 - winner].body.code).toBe(kind === 'bot' ? 'bot-conflict' : 'node-conflict')
      const expected = kind === 'bot' ? { ...candidates[winner], id: 'audit' } : { ...plan, nodes: { A: candidates[winner] } }
      expect(receipts[winner].body).toEqual(expected)
      expect(JSON.parse(fs.readFileSync(path.join(kind === 'bot' ? bots : maps, kind === 'bot' ? 'business-name.JSON' : 'quarter.JSON'), 'utf8'))).toEqual(expected)
      const reread = await read(client, kind === 'bot' ? botsAPI : mapAPI)
      expect(kind === 'bot' ? reread.bots : reread).toEqual(kind === 'bot' ? [expected] : expected)
    } finally { await Promise.all(clients.map(other => other.dispose())) }
  })
}

// S2/P25: a bot-delete preflight is obtained BEFORE an overlapping node
// reference commit. The independent invariant is that a committed reference
// protects a still-present bot. A recursive real-directory observer records
// that state (never completion order of HTTP responses). Delete-first remains
// legal: P16 permits subsequently saving a missing-bot reference.
for (const operation of ['create', 'modify'] as const) {
  test(`P25 bot deletion competes safely with node reference ${operation}`, async ({ request: client }, info) => {
    const { maps, bots } = await prepare()
    const clients = await Promise.all([request.newContext(), request.newContext()])
    const races: unknown[] = []
    try {
      for (let round = 0; round < 8; round++) {
        const id = `shared-${round}`, nodeId = `reference-${round}`
        await accepted(client, `${botsAPI}/save`, { id, content: JSON.stringify(capability), revision: null })
        if (operation === 'modify') {
          const draft = await read(client, `${nodesAPI}/new`)
          await accepted(client, `${nodesAPI}/save`, { id: nodeId, content: JSON.stringify({ ...work, bot_id: '' }), revision: draft.revision, create: true })
        }
        const target = await accepted(client, `${botsAPI}/check-delete`, { id })
        const draft = operation === 'create' ? await read(client, `${nodesAPI}/new`) : await accepted(client, `${nodesAPI}/edit`, { id: nodeId })
        const reference = { ...work, bot_id: id }
        const file = path.join(bots, target.revision.file)
        let committedReferenceWithPresentBot = false
        const observations: unknown[] = []
        const commitEvents: string[] = []
        const watcher = fs.watch(server!.cwd, { recursive: true }, (event, name) => {
          const relative = name?.toString().replaceAll('\\', '/')
          if (event === 'rename' && relative === `.data/bots/${target.revision.file}`) commitEvents.push('bot removed')
          if (relative !== '.data/workflow-maps/quarter.JSON') return
          if (event === 'rename') commitEvents.push('Map replaced')
          try {
            const saved = JSON.parse(fs.readFileSync(path.join(maps, 'quarter.JSON'), 'utf8'))
            const hasReference = saved.nodes[nodeId]?.bot_id === id
            const botPresent = fs.existsSync(file)
            observations.push({ hasReference, botPresent })
            if (hasReference && botPresent) committedReferenceWithPresentBot = true
          } catch (error) {
            // Rename notifications can include the old name disappearing. Keep
            // the observation, but never turn an unavailable read into proof.
            observations.push({ unavailable: String(error) })
          }
        })
        try {
          const jobs = [
            () => post(clients[0], `${nodesAPI}/save`, { id: nodeId, content: JSON.stringify(reference), revision: draft.revision, create: operation === 'create' }),
            () => post(clients[1], `${botsAPI}/delete`, target),
          ]
          // Reverse dispatch on alternating rounds without imposing a commit order.
          const responses = await simultaneous(round % 2 ? jobs.toReversed() : jobs)
          const [saved, deleted] = round % 2 ? responses.toReversed() : responses
          expect(saved.ok(), await saved.text()).toBe(true)
          const expectedMap = await saved.json()
          expect(expectedMap.nodes[nodeId]).toEqual(reference)
          expect(await read(client, mapAPI)).toEqual(expectedMap)
          expect(JSON.parse(fs.readFileSync(path.join(maps, 'quarter.JSON'), 'utf8'))).toEqual(expectedMap)
          const receipt = await deleted.json()
          // Drain notifications from this single recursive watcher, not two
          // unrelated response clocks. Only successful delete-first commits may
          // remove the bot; reference-first must return a refusal.
          await expect.poll(() => commitEvents.includes('Map replaced')).toBe(true)
          if (deleted.ok()) {
            await expect.poll(() => commitEvents.includes('bot removed')).toBe(true)
            expect(commitEvents.indexOf('bot removed')).toBeLessThan(commitEvents.indexOf('Map replaced'))
            expect(fs.existsSync(file)).toBe(false)
            expect(committedReferenceWithPresentBot, 'reference committed before removal: final deletion must refuse').toBe(false)
          } else {
            expect(receipt.code).toBe('bot-referenced')
            expect(receipt.message).toContain(`quarter.JSON / node ${nodeId}`)
            expect(JSON.parse(fs.readFileSync(file, 'utf8'))).toEqual({ ...capability, id })
            // Repeat the final request AFTER observing the committed reference;
            // an earlier no-reference preflight must not authorize this delete.
            expect((await post(client, `${botsAPI}/delete`, target)).ok()).toBe(false)
          }
          const catalog = await read(client, botsAPI)
          expect(catalog.bots.some((item: { id: string }) => item.id === id)).toBe(!deleted.ok())
          races.push({ round, deletionStatus: deleted.status(), receipt, commitEvents, observations })
        } finally { watcher.close() }
      }
      // Deterministic reference-first control: even a successful earlier check
      // cannot survive a subsequently committed reference on the same instance.
      const target = await accepted(client, `${botsAPI}/check-delete`, { id: 'audit' })
      const draft = await read(client, `${nodesAPI}/new`)
      await accepted(client, `${nodesAPI}/save`, { id: 'committed-first', content: JSON.stringify(work), revision: draft.revision, create: true })
      const refused = await post(client, `${botsAPI}/delete`, target)
      expect(refused.ok()).toBe(false)
      expect((await refused.json()).code).toBe('bot-referenced')
      expect(JSON.parse(fs.readFileSync(path.join(bots, 'business-name.JSON'), 'utf8'))).toEqual({ ...capability, id: 'audit' })
      await evidence(info, 'reference-race-commit-observations', races)
    } finally { await Promise.all(clients.map(other => other.dispose())) }
  })
}
