import fs from 'node:fs'
import path from 'node:path'
import { expect, test, type Page, type TestInfo } from '@playwright/test'
import { startServer, type E2EServer, type BotView } from './support'
import { zoneRowFor } from './ui'

test.use({ trace: 'on' })
let server: E2EServer
const responses: { endpoint: string; status: number; body: unknown }[] = []
test.beforeEach(() => { responses.length = 0 })
test.afterEach(async ({ page }, info) => {
  if (!server) return
  await info.attach('public-requests-and-external-observation', { body: JSON.stringify({ responses, calls: server.herdrCalls(), tabs: server.herdrTabs() }, null, 2), contentType: 'application/json' })
  await server.cleanup()
})

// Independent, prewritten catalogue examples, not production parser/resolver output.
const MODEL = `[[models]]
id = 'archive-model'
label = '归档模型'
provider_model = 'archive/original'
thinking = { omp = ['low', 'high'], pi = ['low'] }
`
const PRESET = { name: '待修正 Bot', chatName: '{zone.name}-{chat.id}', agent: 'omp', model: 'archive-model', thinking: 'high', skill: null, prompt: '保留第一行\n保留第二行', extensions: ['archive-tools'] }
function catalog(model = MODEL, optional = true, recovered = false) {
  const root = path.dirname(server.metadataFile)
  const fixed = path.join(root, recovered ? 'repaired-cost.ts' : 'cost.ts')
  fs.writeFileSync(fixed, '// file check only; no execution\n')
  const entry = path.join(root, 'tools.ts')
  fs.writeFileSync(entry, '// file check only; no execution\n')
  return `skills = []
[[agents]]
id = 'omp'
label = 'OMP'
[[agents]]
id = 'pi'
label = 'Pi'
[[models]]
id = 'default'
label = '沿用 Agent 默认值'
${model}
[[extensions]]
id = 'cost'
label = '固定费用'
agent = 'omp'
kind = 'fixed'
entrypoint = '${fixed}'
[[extensions]]
id = 'pi-cost'
label = 'Pi 固定费用'
agent = 'pi'
kind = 'fixed'
entrypoint = '${fixed}'
${optional ? `[[extensions]]
id = 'archive-tools'
label = '归档工具'
agent = 'omp'
kind = 'optional'
entrypoint = '${entry}'` : ''}
`
}
async function request(endpoint: string, body?: unknown, method = 'POST') {
  const response = await fetch(`${server.baseURL}${endpoint}`, body === undefined ? undefined : { method, headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body) })
  const value = await response.json()
  responses.push({ endpoint, status: response.status, body: value })
  return { status: response.status, body: value }
}
async function setup() {
  server = await startServer({ herdr: {} })
  fs.writeFileSync(server.metadataFile, catalog())
  const zonePath = path.join(path.dirname(server.metadataFile), 'recovery-zone')
  fs.mkdirSync(zonePath)
  const zone = (await request('/api/zones', { path: zonePath })).body.zones[0].path as string
  const saved = await request('/api/bots', PRESET)
  expect(saved.status).toBe(200)
  return { zone, saved: saved.body.bots[0] as BotView }
}
async function edit(page: Page, id: string) {
  await page.goto(server.baseURL)
  await page.getByTestId('tab-bots').click()
  await page.locator(`[data-bot-id="${id}"]`).getByTestId('bot-edit').click()
}
async function summary(page: Page) {
  await page.getByTestId('bot-next').click()
  await page.getByTestId('bot-next').click()
}
async function screenshot(page: Page, info: TestInfo, name: string) {
  await page.screenshot({ path: info.outputPath(`${name}.png`), fullPage: true })
  await info.attach(name, { path: info.outputPath(`${name}.png`), contentType: 'image/png' })
}
async function create(zone: string, id: string, chatId = 'a7f3c2') {
  return request('/api/chats', { zone, botId: id, chatId, input: '恢复检查' })
}
async function succeeded(result: { status: number; body: { id: string } }) {
  expect(result.status).toBe(202)
  await expect.poll(async () => (await request(`/api/chats/${result.body.id}`)).body.stage).toBe('succeeded')
}

// S1/S2/S3, P28/P29: removing the saved model must not make the select display default.
// Violation: a missing option silently renders the first option while the draft retains an invisible ID.
// Observe the actual selected ID, explicit unavailable message, unchanged public read after navigation,
// and a user-selected replacement independently read back after explicit save.
test('已移除的模型仍可辨认，导航不修正，显式更换后保存准确', async ({ page }, info) => {
  const { zone, saved } = await setup()
  fs.writeFileSync(server.metadataFile, catalog(''))
  await edit(page, saved.id)
  await expect(page.getByTestId('bot-model')).toHaveValue('archive-model')
  await expect(page.getByTestId('bot-model').locator('option:checked')).toContainText('archive-model（不可用）')
  await expect(page.getByTestId('bot-selection-error')).toContainText('archive-model')
  await summary(page)
  await expect(page.getByTestId('bot-selection-error')).toContainText('不可用')
  await page.getByTestId('save-bot').click()
  await expect(page.getByTestId('bot-form-error')).toContainText('archive-model')
  expect((await request(`/api/bots/${saved.id}`)).body.bot).toEqual(saved)
  expect((await request(`/api/bots/${saved.id}`, PRESET, 'PUT')).status).toBe(400)
  expect((await create(zone, saved.id)).status).toBe(400)
  expect(server.herdrCalls()).toEqual([])
  expect(server.herdrTabs()).toEqual([])
  await screenshot(page, info, 'removed-model-before-correction')
  await page.getByTestId('bot-back').click()
  await page.getByTestId('bot-back').click()
  await page.getByTestId('bot-model').selectOption('default')
  await expect(page.getByTestId('bot-thinking')).toHaveValue('default')
  await summary(page)
  await page.getByTestId('save-bot').click()
  await expect(page.getByTestId('bot-feedback')).toHaveAttribute('data-kind', 'success')
  expect((await request(`/api/bots/${saved.id}`)).body.bot).toEqual({ ...saved, model: 'default', thinking: 'default' })
  await succeeded(await create(zone, saved.id))
  await screenshot(page, info, 'removed-model-corrected')
})

// P28/P29: a formerly supported level remains selected and visibly unavailable, not defaulted.
// High still exists for Pi, but is no longer applicable to this OMP model.
test('适用 thinking 被移除时保留原级别，用户明确改选后才能保存创建', async ({ page }, info) => {
  const { zone, saved } = await setup()
  fs.writeFileSync(server.metadataFile, catalog(MODEL.replace("omp = ['low', 'high'], pi = ['low']", "omp = ['low'], pi = ['low', 'high']")))
  await edit(page, saved.id)
  await expect(page.getByTestId('bot-model')).toHaveValue('archive-model')
  await expect(page.getByTestId('bot-thinking')).toHaveValue('high')
  await expect(page.getByTestId('bot-thinking').locator('option:checked')).toHaveText('high（不可用）')
  await expect(page.getByTestId('bot-selection-error')).toContainText('thinking「high」不可用')
  await summary(page)
  await page.getByTestId('save-bot').click()
  await expect(page.getByTestId('bot-form-error')).toContainText('high')
  expect((await request(`/api/bots/${saved.id}`)).body.bot).toEqual(saved)
  expect((await request(`/api/bots/${saved.id}`, PRESET, 'PUT')).status).toBe(400)
  expect((await create(zone, saved.id)).status).toBe(400)
  expect(server.herdrCalls()).toEqual([])
  expect(server.herdrTabs()).toEqual([])
  await screenshot(page, info, 'removed-thinking-before-correction')
  await page.getByTestId('bot-back').click()
  await page.getByTestId('bot-back').click()
  await page.getByTestId('bot-thinking').selectOption('low')
  await summary(page)
  await page.getByTestId('save-bot').click()
  await expect(page.getByTestId('bot-feedback')).toHaveAttribute('data-kind', 'success')
  expect((await request(`/api/bots/${saved.id}`)).body.bot).toEqual({ ...saved, thinking: 'low' })
  await succeeded(await create(zone, saved.id))
})

// P28/P29: metadata removal cannot hide a selected extension or silently drop it on save.
// A checked unavailable item must remain user-removable; all three external side effects stay absent.
test('失效扩展在手机视口可显式移除，修正前页面与公开创建都拒绝且无副作用', async ({ page }, info) => {
  await page.setViewportSize({ width: 390, height: 844 })
  const { zone, saved } = await setup()
  fs.writeFileSync(server.metadataFile, catalog(MODEL, false))
  await page.goto(server.baseURL)
  await page.getByTestId('tab-files').click()
  await zoneRowFor(page, zone).getByTestId('zone-create-chat').click()
  await expect(page.getByTestId('chat-name-preview')).toHaveAttribute('data-state', 'ready')
  await page.locator('#chat-workspace-target').selectOption('default')
  await page.getByTestId('chat-create-submit').click()
  await expect(page.getByTestId('chat-request-error')).toContainText('archive-tools')
  await expect(page.getByTestId('chat-request-error')).toContainText('未创建 Herdr Tab、未启动 agent、未提交输入')
  await expect(page.getByTestId('chat-create-success')).toHaveCount(0)
  expect((await create(zone, saved.id)).status).toBe(400)
  expect(server.herdrCalls()).toEqual([])
  expect(server.herdrTabs()).toEqual([])
  await page.getByTestId('chat-create-close').click()
  await edit(page, saved.id)
  await expect(page.getByTestId('bot-selection-error')).toContainText('archive-tools')
  await page.getByTestId('bot-next').click()
  const unavailable = page.getByTestId('extension-archive-tools')
  await expect(unavailable).toBeChecked()
  await expect(page.getByText('archive-tools（不可用）', { exact: true })).toBeVisible()
  await page.getByTestId('bot-next').click()
  await expect(page.getByTestId('summary-optional')).toHaveText('archive-tools')
  await page.getByTestId('save-bot').click()
  await expect(page.getByTestId('bot-form-error')).toContainText('archive-tools')
  expect((await request(`/api/bots/${saved.id}`)).body.bot).toEqual(saved)
  expect((await request(`/api/bots/${saved.id}`, PRESET, 'PUT')).status).toBe(400)
  await page.getByTestId('bot-back').click()
  await screenshot(page, info, 'unavailable-extension-mobile')
  // This choice disappears on removal; click observes the user action without requiring a detached checkbox state.
  await unavailable.click()
  await page.getByTestId('bot-back').click()
  await page.getByTestId('bot-next').click()
  await expect(unavailable).toHaveCount(0)
  expect((await request(`/api/bots/${saved.id}`)).body.bot).toEqual(saved)
  await page.getByTestId('bot-next').click()
  await expect(page.getByTestId('summary-optional')).toHaveText('无')
  await page.getByTestId('save-bot').click()
  await expect(page.getByTestId('bot-feedback')).toHaveAttribute('data-kind', 'success')
  expect((await request(`/api/bots/${saved.id}`)).body.bot).toEqual({ ...saved, extensions: [] })
  await page.getByTestId('tab-files').click()
  await zoneRowFor(page, zone).getByTestId('zone-create-chat').click()
  await expect(page.getByTestId('chat-name-preview')).toHaveAttribute('data-state', 'ready')
  await page.locator('#chat-workspace-target').selectOption('default')
  await page.getByTestId('chat-create-submit').click()
  await expect(page.getByTestId('chat-create-success')).toBeVisible()
  expect(server.herdrTabs()).toHaveLength(1)
  expect(server.herdrCalls().filter(call => call.kind === 'agent_start')).toHaveLength(1)
  expect(server.herdrCalls().filter(call => call.kind === 'agent_prompt')).toHaveLength(1)
  await screenshot(page, info, 'corrected-extension-mobile-creation')
})

// S1/S2/S3, P27: both pages have genuinely loaded the old valid choices before the file changes.
// Violation: trusting a browser snapshot or cached catalogue permits a stale save/create.
for (const scenario of [
  { name: '移除模型', model: '', optional: true, reason: 'archive-model' },
  { name: '改变模型 Agent 适用关系', model: MODEL.replace("omp = ['low', 'high'], ", ''), optional: true, reason: 'archive-model' },
  { name: '移除适用 thinking', model: MODEL.replace("omp = ['low', 'high']", "omp = ['low']"), optional: true, reason: 'high' },
  { name: '移除扩展', model: MODEL, optional: false, reason: 'archive-tools' },
]) {
  test(`旧页面${scenario.name}后保存及创建按当前目录拒绝`, async ({ page, context }, info) => {
    const { zone, saved } = await setup()
    await edit(page, saved.id)
    await expect(page.getByTestId('bot-model')).toHaveValue('archive-model')
    await expect(page.getByTestId('bot-thinking')).toHaveValue('high')
    const creator = await context.newPage()
    await creator.goto(server.baseURL)
    await creator.getByTestId('tab-files').click()
    await zoneRowFor(creator, zone).getByTestId('zone-create-chat').click()
    await expect(creator.getByTestId('chat-name-preview')).toHaveAttribute('data-state', 'ready')
    const preview = await request('/api/chats/preview', { zone, botId: saved.id })
    expect(preview.status).toBe(200)
    fs.writeFileSync(server.metadataFile, catalog(scenario.model, scenario.optional))
    await summary(page)
    await page.getByTestId('save-bot').click()
    await expect(page.getByTestId('bot-form-error')).toContainText(scenario.reason)
    await expect(page.getByTestId('bot-feedback')).toHaveCount(0)
    await creator.locator('#chat-workspace-target').selectOption('default')
    await creator.getByTestId('chat-create-submit').click()
    await expect(creator.getByTestId('chat-request-error')).toContainText(scenario.reason)
    await expect(creator.getByTestId('chat-create-success')).toHaveCount(0)
    expect((await request('/api/bots', PRESET)).status).toBe(400)
    expect((await request(`/api/bots/${saved.id}`, PRESET, 'PUT')).status).toBe(400)
    expect((await create(zone, saved.id, preview.body.chatId)).status).toBe(400)
    expect((await request(`/api/bots/${saved.id}`)).body.bot).toEqual(saved)
    expect(server.herdrCalls()).toEqual([])
    expect(server.herdrTabs()).toEqual([])
    await screenshot(page, info, 'stale-editor-rejected')
    await screenshot(creator, info, 'stale-creation-rejected')
  })
}

// P30: a genuinely unknown persisted model must not poison parsing of a neighboring bot.
// The literal JSON fixture is externally written; expected records are not obtained from the parser.
test('同一真实持久文件的未知模型不阻止有效 Bot 读取编辑和创建', async ({ page }, info) => {
  const { zone } = await setup()
  const invalid = { id: 'unknown-model', ...PRESET, model: 'never-known-model' }
  const valid = { id: 'valid-neighbor', ...PRESET, name: '有效邻居', model: 'default', thinking: 'default', extensions: [] }
  fs.writeFileSync(server.storageFile, JSON.stringify({ bots: [invalid, valid] }))
  const listed = await request('/api/bots')
  expect(listed.status).toBe(200)
  expect(listed.body.bots).toEqual([invalid, valid])
  expect((await request('/api/bots/unknown-model')).body.bot).toEqual(invalid)
  expect((await request('/api/bots/valid-neighbor')).body.bot).toEqual(valid)
  expect((await create(zone, invalid.id)).status).toBe(400)
  expect(server.herdrCalls()).toEqual([])
  expect(server.herdrTabs()).toEqual([])
  await edit(page, invalid.id)
  await expect(page.getByTestId('bot-model')).toHaveValue('never-known-model')
  await expect(page.getByTestId('bot-selection-error')).toContainText('never-known-model')
  await page.getByTestId('bot-cancel').click()
  await page.locator('[data-bot-id="valid-neighbor"]').getByTestId('bot-edit').click()
  await expect(page.getByTestId('bot-selection-error')).toHaveCount(0)
  await page.getByTestId('bot-name').fill('有效邻居已编辑')
  await page.getByTestId('bot-prompt').fill('邻居新指示\n第二行')
  await summary(page)
  await page.getByTestId('save-bot').click()
  await expect(page.getByTestId('bot-feedback')).toHaveAttribute('data-kind', 'success')
  expect((await request('/api/bots/valid-neighbor')).body.bot).toEqual({ ...valid, name: '有效邻居已编辑', prompt: '邻居新指示\n第二行' })
  expect((await request('/api/bots/unknown-model')).body.bot).toEqual(invalid)
  await succeeded(await create(zone, valid.id))
  expect(server.herdrCalls().filter(call => call.kind === 'agent_prompt').at(-1)?.text).toBe('邻居新指示\n第二行\n\n恢复检查')
  await screenshot(page, info, 'valid-neighbor-preserves-invalid-bot')
})

const REPAIRED_MODEL = `[[models]]
id = 'fresh-model'
label = '修复后的新模型'
provider_model = 'repaired-provider/second-model'
thinking = { omp = ['medium'], pi = [] }
`
// P32, S1/S2/S3: break the real file after both pages are open, then write a different valid catalogue.
// Violation: a last-known-good cache hides failure or continues using the old native mapping after repair.
for (const fault of ['损坏', '移走']) {
  test(`运行中${fault}元数据明确失败，同一实例修复后使用新选项及映射`, async ({ page, context }, info) => {
    const { zone, saved } = await setup()
    const originalStorage = fs.readFileSync(server.storageFile, 'utf8')
    const originalURL = server.baseURL
    await edit(page, saved.id)
    const creator = await context.newPage()
    await creator.goto(server.baseURL)
    await creator.getByTestId('tab-files').click()
    await zoneRowFor(creator, zone).getByTestId('zone-create-chat').click()
    await expect(creator.getByTestId('chat-name-preview')).toHaveAttribute('data-state', 'ready')
    if (fault === '损坏') fs.writeFileSync(server.metadataFile, 'models = [not valid TOML')
    else fs.renameSync(server.metadataFile, `${server.metadataFile}.moved`)
    const reason = fault === '损坏' ? 'TOML' : '读取失败'
    for (const endpoint of ['/api/bots', `/api/bots/${saved.id}`]) {
      const result = await request(endpoint)
      expect(result.status).toBe(500)
      expect(result.body.message).toContain(server.metadataFile)
      expect(result.body.message).toContain(reason)
    }
    for (const result of [
      await request('/api/bots', PRESET),
      await request(`/api/bots/${saved.id}`, PRESET, 'PUT'),
      await create(zone, saved.id),
    ]) {
      expect(result.status).toBe(500)
      expect(result.body.message).toContain(server.metadataFile)
      expect(result.body.message).toContain(reason)
    }
    await summary(page)
    await page.getByTestId('save-bot').click()
    await expect(page.getByTestId('bot-form-error')).toContainText(server.metadataFile)
    await expect(page.getByTestId('bot-form-error')).toContainText(reason)
    await expect(page.getByTestId('bot-feedback')).toHaveCount(0)
    await creator.locator('#chat-workspace-target').selectOption('default')
    await creator.getByTestId('chat-create-submit').click()
    await expect(creator.getByTestId('chat-request-error')).toContainText(server.metadataFile)
    await expect(creator.getByTestId('chat-request-error')).toContainText(reason)
    await expect(creator.getByTestId('chat-create-success')).toHaveCount(0)
    expect(server.herdrCalls()).toEqual([])
    expect(server.herdrTabs()).toEqual([])
    expect(fs.readFileSync(server.storageFile, 'utf8')).toBe(originalStorage)
    await screenshot(page, info, 'runtime-metadata-save-error')
    await screenshot(creator, info, 'runtime-metadata-create-error')
    await creator.getByTestId('chat-create-close').click()
    await zoneRowFor(creator, zone).getByTestId('zone-create-chat').click()
    await expect(creator.getByTestId('chat-load-error')).toContainText(server.metadataFile)
    await creator.getByTestId('chat-create-close').click()
    await page.getByTestId('bot-cancel').click()
    await page.locator(`[data-bot-id="${saved.id}"]`).getByTestId('bot-edit').click()
    await expect(page.getByTestId('bot-feedback')).toHaveAttribute('data-kind', 'error')
    await expect(page.getByTestId('bot-feedback')).toContainText(server.metadataFile)
    await expect(page.getByTestId('bot-feedback')).toContainText(reason)

    // No restart, reload or restored original file: this independent replacement has new IDs and mappings.
    fs.writeFileSync(server.metadataFile, catalog(REPAIRED_MODEL, false, true))
    const recovered = await request('/api/bots')
    expect(recovered.status).toBe(200)
    expect(recovered.body.options.models).toEqual(['default', 'fresh-model'])
    expect(recovered.body.options.thinking.omp['fresh-model']).toEqual(['medium'])
    expect((await request(`/api/bots/${saved.id}`)).body.bot).toEqual(saved)
    await page.locator(`[data-bot-id="${saved.id}"]`).getByTestId('bot-edit').click()
    await expect(page.getByTestId('bot-model').locator('option[value="fresh-model"]')).toHaveText('修复后的新模型')
    await page.getByTestId('bot-model').selectOption('fresh-model')
    await page.getByTestId('bot-thinking').selectOption('medium')
    await page.getByTestId('bot-next').click()
    await page.getByTestId('extension-archive-tools').click()
    await page.getByTestId('bot-next').click()
    await page.getByTestId('save-bot').click()
    await expect(page.getByTestId('bot-feedback')).toHaveAttribute('data-kind', 'success')
    const fixed = { ...PRESET, model: 'fresh-model', thinking: 'medium', extensions: [] }
    expect((await request(`/api/bots/${saved.id}`)).body.bot).toEqual({ id: saved.id, ...fixed })
    expect((await request(`/api/bots/${saved.id}`, fixed, 'PUT')).status).toBe(200)
    await zoneRowFor(creator, zone).getByTestId('zone-create-chat').click()
    await expect(creator.getByTestId('chat-name-preview')).toHaveAttribute('data-state', 'ready')
    await creator.locator('#chat-workspace-target').selectOption('default')
    await creator.getByTestId('chat-create-submit').click()
    await expect(creator.getByTestId('chat-create-success')).toBeVisible()
    await succeeded(await create(zone, saved.id))
    const starts = server.herdrCalls().filter(call => call.kind === 'agent_start')
    expect(starts).toHaveLength(2)
    for (const start of starts) {
      const args = start.args as string[]
      expect(args.slice(0, 4)).toEqual(['--model', 'repaired-provider/second-model', '--thinking', 'medium'])
      expect(args.slice(4)).toEqual(['--no-extensions', '--extension', path.join(path.dirname(server.metadataFile), 'repaired-cost.ts')])
    }
    expect(server.baseURL).toBe(originalURL)
    await screenshot(creator, info, 'metadata-repaired-creation-success')
  })
}
