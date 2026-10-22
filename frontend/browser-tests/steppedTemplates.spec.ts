import fs from 'node:fs'
import { expect, test } from '@playwright/test'
import { startServer, type E2EServer } from './support'
import { serializedBotRule } from './ui'

let server: E2EServer | null = null
test.afterEach(async () => { await server?.cleanup(); server = null })

const PRESET = {
  name: '发布巡检', botName: '巡检 {zone.name} / {template.name} / {agent} / {bot.id}',
  agent: 'omp', model: 'gpt-6.1-sol', thinking: 'high', skill: 'code-review',
  prompt: '首行：检查发布\n次行：保留 "引号" 和路径 C:\\alpha',
}

async function save(body: unknown, id?: string) {
  return fetch(`${server!.baseURL}/api/bot-templates${id ? `/${id}` : ''}`, {
    method: id ? 'PUT' : 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body),
  })
}

// S1/S2; P1–P6/P12/P14/P16/P18. The literals below are independent of UI rendering.
// Violation: navigation writes early, loses fields, mislabels fixed/optional choices, or saves a different summary.
test('三步新增和编辑保留完整草稿，摘要与公开持久结果一致，删除不关闭既有 Bot', async ({ page }) => {
  server = await startServer({ herdr: {} })
  await page.goto(server.baseURL)
  await page.getByTestId('tab-templates').click()
  await page.getByTestId('template-create-open').click()
  await expect(page.getByTestId('template-step')).toHaveText('1 / 3 · 模板预设')
  await expect(page.getByTestId('save-template')).toHaveCount(0)
  await page.getByTestId('template-name').fill(PRESET.name)
  const rule = page.getByTestId('template-bot-name')
  await rule.fill('')
  for (const id of ['insert-zone-name', 'insert-template-name', 'insert-agent', 'insert-bot-id']) {
    await page.getByTestId(id).click()
  }
  expect(await serializedBotRule(rule)).toBe('{zone.name}{template.name}{agent}{bot.id}')
  await expect(page.getByTestId('template-dialog').getByTestId('bot-name-preview')).toHaveText('alpha发布巡检ompa7f3c2')
  await rule.fill(PRESET.botName)
  await expect(page.getByTestId('template-dialog').getByTestId('bot-name-preview')).toHaveText('巡检 alpha / 发布巡检 / omp / a7f3c2')
  await page.getByTestId('template-model').selectOption('gpt-6.1-sol')
  await page.getByTestId('template-thinking').selectOption('high')
  await page.getByTestId('template-skill').selectOption('code-review')
  await page.getByTestId('template-prompt').fill(PRESET.prompt)
  await page.getByTestId('template-next').click()
  await expect(page.getByTestId('template-step')).toHaveText('2 / 3 · 扩展配置')
  const fixed = page.getByTestId('template-fixed-extensions')
  await expect(fixed).toHaveText('固定扩展（始终加载，不可取消）model-cost')
  await expect(fixed.getByRole('checkbox')).toHaveCount(0)
  await expect(page.getByRole('checkbox')).toHaveCount(2)
  await expect(page.getByTestId('extension-omp-codebase-tools')).not.toBeChecked()
  await expect(page.getByTestId('extension-omp-connect-bot')).not.toBeChecked()
  await page.getByTestId('extension-omp-codebase-tools').check()
  await page.getByTestId('extension-omp-connect-bot').check()
  await page.getByTestId('template-back').click()
  await expect(page.getByTestId('template-name')).toHaveValue('发布巡检')
  expect(await serializedBotRule(rule)).toBe(PRESET.botName)
  await expect(page.getByTestId('template-agent')).toHaveValue('omp')
  await expect(page.getByTestId('template-model')).toHaveValue('gpt-6.1-sol')
  await expect(page.getByTestId('template-thinking')).toHaveValue('high')
  await expect(page.getByTestId('template-skill')).toHaveValue('code-review')
  await expect(page.getByTestId('template-prompt')).toHaveValue(PRESET.prompt)
  await page.getByTestId('template-next').click()
  await expect(page.getByTestId('extension-omp-codebase-tools')).toBeChecked()
  await expect(page.getByTestId('extension-omp-connect-bot')).toBeChecked()
  await page.getByTestId('template-next').click()
  await expect(page.getByTestId('template-step')).toHaveText('3 / 3 · 确认保存')
  const summary = page.getByTestId('template-summary')
  for (const [id, value] of Object.entries({
    name: '发布巡检', botName: PRESET.botName, agent: 'OMP', model: 'gpt-6.1-sol', thinking: 'high',
    skill: '/code-review', prompt: PRESET.prompt, fixed: 'model-cost', optional: 'codebase-tools、connect-bot', discovery: '关闭',
  })) await expect(summary.getByTestId(`summary-${id}`)).toHaveText(value)
  expect((await server.getTemplates()).templates).toEqual([])
  await page.getByTestId('save-template').click()
  await expect(page.getByTestId('template-feedback')).toHaveAttribute('data-kind', 'success')
  const saved = (await server.getTemplates()).templates[0]
  expect(saved).toEqual({ id: expect.any(String), ...PRESET, extensions: ['omp-codebase-tools', 'omp-connect-bot'] })
  await expect(page.getByTestId('bot-template-item')).toContainText('固定扩展')
  await expect(page.getByTestId('bot-template-item')).toContainText('connect-bot')

  // The externally hosted Bot already exists; deletion must leave that object untouched.
  const zoneDir = `${server.storageFile}.zone`
  fs.mkdirSync(zoneDir)
  const zoneResponse = await fetch(`${server.baseURL}/api/zones`, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ path: zoneDir }) })
  const zone = (await zoneResponse.json()).zones[0].path
  const previewResponse = await fetch(`${server.baseURL}/api/bots/preview`, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ zone, templateId: saved.id }) })
  const preview = await previewResponse.json()
  const botResponse = await fetch(`${server.baseURL}/api/bots`, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ zone, templateId: saved.id, botId: preview.botId, input: '检查' }) })
  expect(botResponse.status).toBe(202)
  const bot = await botResponse.json()
  await expect.poll(async () => (await (await fetch(`${server!.baseURL}/api/bots/${bot.id}`)).json()).stage).toBe('succeeded')
  const calls = server.herdrCalls()
  const tabs = server.herdrTabs()

  await page.getByTestId('template-edit').click()
  await expect(page.getByTestId('template-step')).toHaveText('1 / 3 · 模板预设')
  await page.getByTestId('template-name').fill('编辑后的巡检')
  await rule.fill('固定 {agent} · {template.name} · {bot.id}')
  await page.getByTestId('template-agent').selectOption('pi')
  await page.getByTestId('template-model').selectOption('deepseek-flash')
  await page.getByTestId('template-thinking').selectOption('off')
  await page.getByTestId('template-skill').selectOption('teach')
  await page.getByTestId('template-prompt').fill('编辑首行\n编辑次行')
  await page.getByTestId('template-next').click()
  await expect(page.getByRole('checkbox')).toHaveCount(1)
  await page.getByTestId('extension-pi-codebase-tools').check()
  await page.getByTestId('template-next').click()
  await page.getByTestId('template-back').click()
  await expect(page.getByTestId('extension-pi-codebase-tools')).toBeChecked()
  expect((await server.getTemplates()).templates).toEqual([saved])
  await page.getByTestId('template-next').click()
  await expect(summary.getByTestId('summary-prompt')).toHaveText('编辑首行\n编辑次行')
  await page.getByTestId('save-template').click()
  await expect(page.getByTestId('template-dialog')).toHaveCount(0)
  expect((await server.getTemplates()).templates).toEqual([{
    id: saved.id, name: '编辑后的巡检', botName: '固定 {agent} · {template.name} · {bot.id}', agent: 'pi',
    model: 'deepseek-flash', thinking: 'off', skill: 'teach', prompt: '编辑首行\n编辑次行', extensions: ['pi-codebase-tools'],
  }])
  await page.getByTestId('template-delete').click()
  await expect(page.getByTestId('template-empty')).toBeVisible()
  expect((await server.getTemplates()).templates).toEqual([])
  expect(server.herdrCalls()).toEqual(calls)
  expect(server.herdrTabs()).toEqual(tabs)
})

// S1/S2; P14/P17. Violation: hidden OMP choices reappear, or same-label Pi choices inherit them.
test('切换 Agent 清空扩展且切回不恢复，新模板零选明确保存为空', async ({ page }) => {
  server = await startServer()
  await page.goto(server.baseURL)
  await page.getByTestId('tab-templates').click()
  await page.getByTestId('template-create-open').click()
  await page.getByTestId('template-name').fill('零选样例')
  await page.getByTestId('template-model').selectOption('gpt-6.1-sol')
  await page.getByTestId('template-thinking').selectOption('max')
  await page.getByTestId('template-next').click()
  await page.getByTestId('extension-omp-codebase-tools').check()
  await page.getByTestId('extension-omp-connect-bot').check()
  await page.getByTestId('template-back').click()
  await page.getByTestId('template-agent').selectOption('pi')
  await expect(page.getByTestId('template-model')).toHaveValue('default')
  await expect(page.getByTestId('template-thinking')).toHaveValue('default')
  await page.getByTestId('template-next').click()
  await expect(page.getByRole('checkbox')).toHaveCount(1)
  await expect(page.getByTestId('extension-pi-codebase-tools')).not.toBeChecked()
  await page.getByTestId('template-back').click()
  await page.getByTestId('template-agent').selectOption('omp')
  await expect(page.getByTestId('template-model')).toHaveValue('default')
  await expect(page.getByTestId('template-thinking')).toHaveValue('default')
  await page.getByTestId('template-next').click()
  await expect(page.getByTestId('extension-omp-codebase-tools')).not.toBeChecked()
  await expect(page.getByTestId('extension-omp-connect-bot')).not.toBeChecked()
  await page.getByTestId('template-next').click()
  await expect(page.getByTestId('summary-optional')).toHaveText('无')
  await expect(page.getByTestId('summary-fixed')).toHaveText('model-cost')
  await page.getByTestId('save-template').click()
  await expect(page.getByTestId('template-dialog')).toHaveCount(0)
  expect((await server.getTemplates()).templates[0]).toMatchObject({ name: '零选样例', model: 'default', thinking: 'default', skill: null, prompt: '', extensions: [] })
})

// S1/S2/S3; P25/P26. Violation: editor reuses cached labels, or save trusts stale eligibility.
test('扩展元数据在同一进程重新编辑和保存时生效，只持久保存稳定选择', async ({ page }) => {
  server = await startServer()
  const original = fs.readFileSync(server.metadataFile, 'utf8')
  expect((await save({ ...PRESET, extensions: ['omp-codebase-tools'] })).status).toBe(200)
  const saved = (await server.getTemplates()).templates[0]
  await page.goto(server.baseURL)
  await page.getByTestId('tab-templates').click()
  await page.getByTestId('template-edit').click()
  await page.getByTestId('template-next').click()
  await expect(page.getByRole('checkbox', { name: 'codebase-tools', exact: true })).toBeChecked()
  await page.getByTestId('template-cancel').click()
  const updated = original.replace("label = 'codebase-tools'", "label = '当前代码工具'").replace('omp-auto\\codebase-tools.ts', 'omp-auto\\changed-tools.ts')
  fs.writeFileSync(server.metadataFile, updated)
  await page.getByTestId('template-edit').click()
  await page.getByTestId('template-next').click()
  await expect(page.getByRole('checkbox', { name: '当前代码工具', exact: true })).toBeChecked()
  await page.getByTestId('template-next').click()
  await expect(page.getByTestId('summary-optional')).toHaveText('当前代码工具')
  // Alter eligibility after opening: the later save must use current metadata, not the page snapshot.
  fs.writeFileSync(server.metadataFile, updated.replace("kind = 'optional'", "kind = 'fixed'"))
  await page.getByTestId('save-template').click()
  await expect(page.getByTestId('template-form-error')).toContainText('不是当前 Agent 的可选扩展')
  await expect(page.getByTestId('template-feedback')).toHaveCount(0)
  expect((await server.getTemplates()).templates).toEqual([saved])
  fs.writeFileSync(server.metadataFile, updated)
  await page.getByTestId('save-template').click()
  await expect(page.getByTestId('template-dialog')).toHaveCount(0)
  expect((await server.getTemplates()).templates).toEqual([saved])
  expect(JSON.parse(fs.readFileSync(server.storageFile, 'utf8')).templates).toEqual([saved])
})

// S1/S2/S3; P4/P19/P20. A real directory blocks the same-directory temporary write.
// Violation: optimistic success or destructive replacement would lose the old literal after restart.
test('真实磁盘写入故障明确失败且重读和重启保留旧模板与扩展', async ({ page }) => {
  server = await startServer()
  expect((await save({ ...PRESET, extensions: ['omp-codebase-tools', 'omp-connect-bot'] })).status).toBe(200)
  const saved = (await server.getTemplates()).templates[0]
  await page.goto(server.baseURL)
  await page.getByTestId('tab-templates').click()
  await page.getByTestId('template-edit').click()
  await page.getByTestId('template-name').fill('不应保存的名字')
  await page.getByTestId('template-prompt').fill('失败首行\n失败次行')
  await page.getByTestId('template-next').click()
  await page.getByTestId('extension-omp-connect-bot').uncheck()
  await page.getByTestId('template-next').click()
  expect((await server.getTemplates()).templates).toEqual([saved])
  fs.mkdirSync(`${server.storageFile}.tmp`)
  await page.getByTestId('save-template').click()
  await expect(page.getByTestId('template-form-error')).toContainText('保存 Bot 模板失败')
  await expect(page.getByTestId('template-feedback')).toHaveCount(0)
  await expect(page.getByTestId('template-dialog')).toBeVisible()
  expect((await server.getTemplates()).templates).toEqual([saved])
  fs.rmdirSync(`${server.storageFile}.tmp`)
  await page.getByTestId('template-back').click()
  await expect(page.getByTestId('extension-omp-connect-bot')).not.toBeChecked()
  await page.getByTestId('template-back').click()
  await expect(page.getByTestId('template-name')).toHaveValue('不应保存的名字')
  await expect(page.getByTestId('template-prompt')).toHaveValue('失败首行\n失败次行')
  await page.getByTestId('template-cancel').click()
  await server.restart()
  const independent = await fetch(`${server.baseURL}/api/bot-templates/${saved.id}`)
  expect((await independent.json()).template).toEqual(saved)
  expect((await server.getTemplates()).templates).toEqual([saved])
})

// S2/S3; P12–P16/P19/P25. Source: the six approved literal combinations.
// Violation: accepting IDs without Agent/kind validation, or dropping selections on disk/restart.
test('公开保存六种扩展组合并在独立客户端和真实重启后保留稳定标识', async () => {
  server = await startServer()
  const options = (await (await fetch(`${server.baseURL}/api/bot-templates`)).json()).options
  expect(options.extensions.map(({ id, agent, kind }: { id: string; agent: string; kind: string }) => ({ id, agent, kind }))).toEqual([
    { id: 'omp-model-cost', agent: 'omp', kind: 'fixed' },
    { id: 'omp-codebase-tools', agent: 'omp', kind: 'optional' },
    { id: 'omp-connect-bot', agent: 'omp', kind: 'optional' },
    { id: 'pi-model-cost', agent: 'pi', kind: 'fixed' },
    { id: 'pi-codebase-tools', agent: 'pi', kind: 'optional' },
  ])
  const cases = [
    { agent: 'omp', extensions: [] },
    { agent: 'omp', extensions: ['omp-codebase-tools'] },
    { agent: 'omp', extensions: ['omp-connect-bot'] },
    { agent: 'omp', extensions: ['omp-codebase-tools', 'omp-connect-bot'] },
    { agent: 'pi', extensions: [] },
    { agent: 'pi', extensions: ['pi-codebase-tools'] },
  ]
  const saved = []
  for (const selection of cases) {
    const literal = { ...PRESET, ...selection }
    const response = await save(literal)
    expect(response.status).toBe(200)
    const template = (await response.json()).templates.at(-1)
    expect(template).toEqual({ id: expect.any(String), ...literal })
    saved.push(template)
  }
  for (const extensions of [['pi-codebase-tools'], ['omp-model-cost'], ['missing-extension']]) {
    const rejected = await save({ ...PRESET, extensions })
    expect(rejected.status).toBe(400)
    expect((await server.getTemplates()).templates).toEqual(saved)
  }
  expect((await save({ ...PRESET, agent: 'pi', extensions: ['omp-codebase-tools'] })).status).toBe(400)
  expect((await save({ ...PRESET, extensions: ['omp-codebase-tools', 'omp-codebase-tools'] })).status).toBe(400)
  expect((await server.getTemplates()).templates).toEqual(saved)
  const stored = JSON.parse(fs.readFileSync(server.storageFile, 'utf8')).templates
  expect(stored).toEqual(saved)
  expect(Object.keys(stored[3]).sort()).toEqual(['agent', 'botName', 'extensions', 'id', 'model', 'name', 'prompt', 'skill', 'thinking'])
  await server.restart()
  expect((await server.getTemplates()).templates).toEqual(saved)
  for (const template of saved) {
    const response = await fetch(`${server.baseURL}/api/bot-templates/${template.id}`)
    expect((await response.json()).template).toEqual(template)
  }
})
