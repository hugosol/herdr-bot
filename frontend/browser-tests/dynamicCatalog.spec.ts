import fs from 'node:fs'
import path from 'node:path'
import { expect, test } from '@playwright/test'
import { startServer, type E2EServer } from './support'

let server: E2EServer | null = null
test.afterEach(async () => { await server?.cleanup(); server = null })

// S1/S2/S3, P25/P26/P40. Independent labels and mappings are prewritten below.
// Violation: editor only refreshes on tab activation, or launch still uses compiled mappings.
test('同一真实进程重新打开编辑、保存和创建各自读取当前独立元数据', async ({ page }) => {
  server = await startServer({ herdr: {} })
  const current = server
  const original = fs.readFileSync(current.metadataFile, 'utf8')
  const literal = { name: '动态模型 Bot', chatName: '{zone.name}-{chat.id}', agent: 'omp', model: 'gpt-6.1-sol', thinking: 'high', skill: 'teach', prompt: '固定第一行\n固定第二行' }
  async function post(endpoint: string, body: unknown) {
    return fetch(`${current.baseURL}${endpoint}`, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body) })
  }
  const savedResponse = await post('/api/bots', literal)
  expect(savedResponse.ok).toBe(true)
  const saved = (await savedResponse.json()).bots[0]
  await page.goto(current.baseURL)
  await page.getByTestId('tab-bots').click()
  await page.getByTestId('bot-edit').click()
  await expect(page.getByTestId('bot-prompt')).toHaveValue('固定第一行\n固定第二行')
  await page.getByTestId('bot-cancel').click()

  // No tab change, page reload or process restart: reopening itself must fetch current choices.
  fs.writeFileSync(current.metadataFile, original.replace("label = 'GPT-6.1-Sol'", "label = '当前 GPT 显示名'") + `
[[models]]
id = 'lab-model'
label = '新增实验模型'
provider_model = 'example/new-model'
thinking = { omp = ['low', 'high'], pi = ['off'] }
`)
  await page.getByTestId('bot-edit').click()
  await expect(page.getByTestId('bot-model').locator('option[value="lab-model"]')).toHaveText('新增实验模型')
  await expect(page.getByTestId('bot-model').locator('option[value="gpt-6.1-sol"]')).toHaveText('当前 GPT 显示名')
  await page.getByTestId('bot-model').selectOption('lab-model')
  await expect(page.getByTestId('bot-thinking')).toHaveValue('high')
  await page.getByTestId('bot-cancel').click()

  // Same saved identifiers, new native mappings: must not require re-saving the bot.
  fs.writeFileSync(current.metadataFile, original.replace('openai-codex/gpt-6.1-sol', 'catalog-provider/updated-gpt').replace("native_name = 'teach'", "native_name = 'updated-teach'"))
  const zonePath = path.join(path.dirname(current.metadataFile), 'catalog-zone')
  fs.mkdirSync(zonePath)
  const zonesResponse = await post('/api/zones', { path: zonePath })
  const zone = (await zonesResponse.json()).zones[0].path
  const previewResponse = await post('/api/chats/preview', { zone, botId: saved.id })
  const preview = await previewResponse.json()
  const createdResponse = await post('/api/chats', { zone, botId: saved.id, chatId: preview.chatId, input: '本次第一行\n本次第二行' })
  expect(createdResponse.status).toBe(202)
  const created = await createdResponse.json()
  await expect.poll(async () => (await (await fetch(`${current.baseURL}/api/chats/${created.id}`)).json()).stage).toBe('succeeded')
  const start = current.herdrCalls().find((call) => call.kind === 'agent_start')
  expect((start?.args as string[]).slice(0, 4)).toEqual(['--model', 'catalog-provider/updated-gpt', '--thinking', 'high'])
  expect(start?.initialMessage).toBe('/skill:updated-teach 固定第一行\n固定第二行\n\n本次第一行\n本次第二行')
  expect(current.herdrCalls().filter(call => call.kind === 'agent_prompt')).toHaveLength(0)
  expect((await current.getBots()).bots[0]).toEqual(saved)
  const persisted = JSON.parse(fs.readFileSync(current.storageFile, 'utf8')).bots[0]
  expect(Object.keys(persisted).sort()).toEqual(['agent', 'chatName', 'extensions', 'id', 'model', 'name', 'prompt', 'skill', 'thinking'])

  // Saving rechecks current capabilities independently from the options previously displayed.
  fs.writeFileSync(current.metadataFile, original.replace("omp = ['low', 'medium', 'high', 'xhigh', 'max']", "omp = ['low']"))
  const rejected = await post('/api/bots', literal)
  expect(rejected.status).toBe(400)
  const accepted = await post('/api/bots', { ...literal, thinking: 'low', skill: null, prompt: '' })
  expect(accepted.status).toBe(200)
  const acceptedBody = await accepted.json()
  expect(acceptedBody.bots.at(-1)).toMatchObject({ thinking: 'low', skill: null, prompt: '' })
})
