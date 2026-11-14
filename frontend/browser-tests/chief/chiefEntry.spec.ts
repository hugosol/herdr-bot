import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { createHash } from 'node:crypto'
import { fileURLToPath } from 'node:url'
import { expect, test } from '@playwright/test'
import { startServer, type E2EServer } from '../support/support'

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../..')
const environmentKeys = ['HOME', 'USERPROFILE', 'APPDATA', 'LOCALAPPDATA', 'XDG_CONFIG_HOME', 'XDG_DATA_HOME', 'XDG_STATE_HOME', 'PI_CODING_AGENT_DIR', 'OMP_CODING_AGENT_DIR']
let environment: Record<string, string | undefined> = {}
let isolatedHome: string | null = null
let server: E2EServer | null = null
test.beforeEach(async ({ browser }) => {
  // Chrome must launch with its own Playwright profile before Windows known
  // folders are isolated for backend/native resources (no debugging bypass).
  void browser.version()
  isolatedHome = fs.mkdtempSync(path.join(os.tmpdir(), 'chief-entry-home-'))
  environment = Object.fromEntries(environmentKeys.map(key => [key, process.env[key]]))
  for (const key of environmentKeys) {
    const directory = path.join(isolatedHome, key)
    fs.mkdirSync(directory)
    process.env[key] = directory
  }
})
test.afterEach(async ({ browser }, info) => {
  try {
    const target = process.env.CARGO_TARGET_DIR ?? path.join(root, 'backend/target')
    const files = ['frontend/src/chats/ChiefEntry.tsx', 'frontend/src/chats/ChatCreationDialog.tsx', 'frontend/src/app/App.tsx', 'frontend/src/app/navigation.css', 'frontend/browser-tests/chief/chiefEntry.spec.ts', 'frontend/bun.lock', 'backend/Cargo.lock', 'backend/static/index.html', ...fs.readdirSync(path.join(root, 'backend/static/assets')).map(file => `backend/static/assets/${file}`), path.join(target, 'debug/herdr-bot.exe'), path.join(target, 'debug/herdr-fixture.exe')]
    const evidence = info.outputPath('chief-entry-evidence.json')
    fs.writeFileSync(evidence, JSON.stringify({ browser: { channel: 'chrome', type: browser.browserType().name(), version: browser.version() }, controlledExternalHerdr: true, modelCalls: 0, isolatedHome, status: info.status, files: files.map(file => ({ file, sha256: createHash('sha256').update(fs.readFileSync(path.resolve(root, file))).digest('hex') })), calls: server?.herdrCalls(), tabs: server?.herdrTabs(), diagnostics: server?.diagnostics() }, null, 2))
    await info.attach('chief-entry-evidence', { contentType: 'application/json', path: evidence })
  } finally {
    try {
      await server?.cleanup()
      server = null
      if (isolatedHome) fs.rmSync(isolatedHome, { recursive: true, force: true })
    } finally {
      for (const key of environmentKeys) {
        if (environment[key] === undefined) delete process.env[key]
        else process.env[key] = environment[key]
      }
      isolatedHome = null
    }
  }
})

async function post(endpoint: string, body: unknown) {
  const response = await fetch(`${server!.baseURL}${endpoint}`, {
    method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify(body),
  })
  expect(response.ok, await response.clone().text()).toBe(true)
  return response.json()
}

async function prepare(defaultZone = true) {
  server = await startServer({ herdr: {} })
  // Test-owned reusable definition, with the delivered identity; no user data.
  fs.writeFileSync(server.storageFile, JSON.stringify({ tasks: [{
    id: 'chief-bot', name: 'chief-bot', chatName: 'chat with chief bot', agent: 'omp',
    taskType: 'chief', model: 'gpt-6-luna', thinking: 'medium', skill: null, prompt: '', suite: null,
  }] }))
  const cwd = path.join(server.cwd, 'bot-playground')
  fs.mkdirSync(cwd)
  await post('/api/zones', { id: defaultZone ? 'default/bot-playground' : 'test/valid', path: cwd })
  return cwd
}

// Approved S1/S2. P1/P4 literals are contract values, not computed from Task data.
// A header-only decoration, misplaced button, or Task-name-derived Chat default
// would fail the real click/hit-test or the independently specified values.
test('P1 P4 chief entry is hit-testable left of server and displays the three defaults', async ({ page }, info) => {
  await prepare()
  await page.goto(server!.baseURL)
  const chief = page.getByRole('button', { name: 'chief bot', exact: true })
  await expect(chief).toBeVisible()
  const chiefBox = (await chief.boundingBox())!
  const serverBox = (await page.getByTestId('server-button').boundingBox())!
  expect(chiefBox.x + chiefBox.width).toBeLessThanOrEqual(serverBox.x)
  expect(Math.abs(chiefBox.y - serverBox.y)).toBeLessThan(2)
  expect(await chief.evaluate(element => {
    const box = element.getBoundingClientRect()
    return element.contains(document.elementFromPoint(box.x + box.width / 2, box.y + box.height / 2))
  })).toBe(true)
  await chief.click()
  const dialog = page.getByTestId('chat-creation-dialog')
  await expect(dialog).toBeVisible()
  await expect(dialog.getByTestId('task')).toHaveValue('chief-bot')
  await expect(dialog.getByTestId('chat-zone')).toHaveValue('default/bot-playground')
  await expect(dialog.getByTestId('chat-name-preview')).toHaveText('chat with chief bot')
  await page.screenshot({ path: info.outputPath('chief-defaults.png'), fullPage: true })
})

// P5 oracle: only two chief definitions are eligible, and the external CLI
// must receive the user's literal zone/model/thinking/name rather than presets.
test('P5 chief-only Task selection applies one-shot settings without rewriting presets', async ({ page }, info) => {
  await prepare()
  const alternatePath = path.join(server!.cwd, 'alternate')
  fs.mkdirSync(alternatePath)
  await post('/api/zones', { id: 'test/alternate', path: alternatePath })
  for (const taskType of ['normal', 'manager', 'chief']) {
    await post('/api/tasks', { name: `choice-${taskType}`, taskType, agent: 'pi', chatName: 'preset name', model: 'default', thinking: 'default', skill: null, prompt: 'Preset instruction', suite: null })
  }
  const before = (await server!.getTasks()).tasks
  const alternate = before.find(task => task.name === 'choice-chief')!
  const native = path.join(server!.cwd, 'alternate-native.jsonl')
  fs.writeFileSync(native, JSON.stringify({ type: 'session', version: 3, id: 'alternate-native', cwd: alternatePath }) + '\n')
  server!.setHerdrBehavior({ native_session: { source: 'herdr:pi', agent: 'pi', kind: 'path', value: native } })
  await page.goto(server!.baseURL)
  await page.getByRole('button', { name: 'chief bot', exact: true }).click()
  const dialog = page.getByTestId('chat-creation-dialog')
  await expect(dialog.getByTestId('task')).toHaveValue('chief-bot')
  expect(await dialog.getByTestId('task').locator('option').evaluateAll(options => options.map(option => (option as HTMLOptionElement).value).filter(Boolean))).toEqual(['chief-bot', alternate.id])
  await dialog.getByTestId('task').selectOption(alternate.id)
  await dialog.getByTestId('chat-zone').selectOption('test/alternate')
  await dialog.locator('#chat-workspace-target').selectOption('default')
  await dialog.locator('#chat-model').selectOption('gpt-6-astra')
  await dialog.locator('#chat-model').selectOption('gpt-6-luna')
  await dialog.locator('#chat-thinking').selectOption('high')
  await expect(dialog.getByTestId('chat-name-preview')).toHaveAttribute('data-state', 'ready')
  await dialog.locator('#chat-name-rule').fill('One-shot chief conversation')
  await dialog.getByTestId('chat-input').fill('One-shot user instruction')
  const accepted = page.waitForResponse(response => response.url().endsWith('/api/chats') && response.request().method() === 'POST')
  await dialog.getByTestId('chat-create-submit').click()
  const response = await accepted
  expect(response.status()).toBe(202)
  expect(response.request().postDataJSON()).toMatchObject({ taskId: alternate.id, zone: 'test/alternate', model: 'gpt-6-luna', thinking: 'high', expectedName: 'One-shot chief conversation' })
  await expect(dialog.getByTestId('chat-create-success')).toBeVisible()
  expect(server!.herdrTabs()).toEqual([expect.objectContaining({ label: 'One-shot chief conversation', cwd: alternatePath })])
  const calls = server!.herdrCalls()
  expect(calls.find(call => call.kind === 'agent_start')?.args).toEqual(expect.arrayContaining(['--model', 'openai-codex/gpt-6-luna', '--thinking', 'high']))
  expect(calls.find(call => call.kind === 'agent_prompt')?.text).toContain('One-shot user instruction')
  expect((await server!.getTasks()).tasks).toEqual(before)
  fs.writeFileSync(info.outputPath('chief-one-shot.json'), JSON.stringify({ request: response.request().postDataJSON(), calls, definitions: before }, null, 2))
})

// P6: absent default binding must not be invented or silently substituted.
// An implementation auto-selecting the only registered zone fails the empty
// selection/disabled action; implicit registration or directories fail readback.
test('P6 missing default zone requires an explicit registered selection before launch', async ({ page }, info) => {
  const cwd = await prepare(false)
  const registry = await (await fetch(`${server!.baseURL}/api/zones`)).json()
  const directories = fs.readdirSync(server!.cwd, { withFileTypes: true }).filter(entry => entry.isDirectory()).map(entry => entry.name)
  await page.setViewportSize({ width: 390, height: 844 })
  await page.goto(server!.baseURL)
  await page.getByRole('button', { name: 'chief bot', exact: true }).click()
  const dialog = page.getByTestId('chat-creation-dialog')
  await expect(dialog.getByTestId('task')).toHaveValue('chief-bot')
  await expect(dialog.getByTestId('chat-zone')).toHaveValue('')
  await expect(dialog.getByTestId('chat-create-submit')).toBeDisabled()
  expect(await (await fetch(`${server!.baseURL}/api/zones`)).json()).toEqual(registry)
  expect(fs.readdirSync(server!.cwd, { withFileTypes: true }).filter(entry => entry.isDirectory()).map(entry => entry.name)).toEqual(directories)
  expect(server!.herdrTabs()).toEqual([])
  await page.screenshot({ path: info.outputPath('missing-chief-zone.png'), fullPage: true })
  const native = path.join(server!.cwd, 'chosen-zone.jsonl')
  fs.writeFileSync(native, JSON.stringify({ type: 'session', version: 3, id: 'chosen-zone-native', cwd }) + '\n')
  server!.setHerdrBehavior({ native_session: { source: 'herdr:omp', agent: 'omp', kind: 'path', value: native } })
  await dialog.getByTestId('chat-zone').selectOption('test/valid')
  await dialog.getByTestId('chat-input').fill('Use explicitly selected zone')
  await expect(dialog.getByTestId('chat-create-submit')).toBeEnabled()
  await dialog.getByTestId('chat-create-submit').click()
  await expect(dialog.getByTestId('chat-create-success')).toBeVisible()
  expect(server!.herdrTabs()).toEqual([expect.objectContaining({ cwd })])
  expect(await (await fetch(`${server!.baseURL}/api/zones`)).json()).toEqual(registry)
})

// P2/P3/P4: exercise actual HTTP launches and legal unregister transitions.
// A browser caching “no chief”, or creating another Task per Chat, fails the
// second-browser focus and the public definition readback after both launches.
test('P2 P3 P4 repeated legal launches reuse chief-bot and other browsers open the authoritative Chat', async ({ page, browser }, info) => {
  const cwd = await prepare()
  const original = (await server!.getTasks()).tasks
  // A changed preset name must not redefine the chief-entry default.
  const { id: _id, ...preset } = original[0]
  const edited = await fetch(`${server!.baseURL}/api/tasks/chief-bot`, {
    method: 'PUT', headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ ...preset, chatName: 'Task preset must remain separate' }),
  })
  expect(edited.ok).toBe(true)
  const expectedDefinitions = (await server!.getTasks()).tasks
  await page.goto(server!.baseURL)
  const other = await browser.newContext()
  try {
    const second = await other.newPage()
    await second.goto(server!.baseURL)
    // Prime another browser with the empty-authority path before first launch.
    await second.getByRole('button', { name: 'chief bot', exact: true }).click()
    await expect(second.getByTestId('chat-creation-dialog')).toBeVisible()
    await second.getByTestId('chat-create-close').click()
    const observations = []
    for (const number of [1, 2, 3]) {
      const sessionId = `chief-entry-native-${number}`
      const native = path.join(server!.cwd, `${sessionId}.jsonl`)
      fs.writeFileSync(native, JSON.stringify({ type: 'session', version: 3, id: sessionId, cwd }) + '\n')
      server!.setHerdrBehavior({ native_session: { source: 'herdr:omp', agent: 'omp', kind: 'path', value: native } })
      await page.getByRole('button', { name: 'chief bot', exact: true }).click()
      const dialog = page.getByTestId('chat-creation-dialog')
      await expect(dialog.getByTestId('chat-name-preview')).toHaveText('chat with chief bot')
      // Default chief has no fixed prompt or Skill. Opening its Chat must
      // leave the ready agent waiting, not submit an empty or invented turn.
      await expect(dialog.getByTestId('chat-input')).toHaveValue('')
      const accepted = page.waitForResponse(response => response.url().endsWith('/api/chats') && response.request().method() === 'POST')
      await dialog.getByTestId('chat-create-submit').click()
      const response = await accepted
      expect(response.status()).toBe(202)
      expect(response.request().postDataJSON()).toMatchObject({ taskId: 'chief-bot', zone: 'default/bot-playground', expectedName: 'chat with chief bot' })
      await expect(dialog.getByTestId('chat-create-success')).toHaveText('创建成功，等待用户输入')
      const started = await (await fetch(`${server!.baseURL}/api/chats/${(await response.json()).id}`)).json()
      expect(started).toMatchObject({ stage: 'succeeded', inputSubmitted: false })
      expect(server!.herdrCalls().filter(call => call.kind === 'agent_prompt')).toEqual([])
      expect(server!.herdrTabs().at(-1)).toMatchObject({ label: 'chat with chief bot', cwd })
      await dialog.getByTestId('chat-create-close').click()
      await post('/api/sessions/events', { sessionId, kind: 'register', data: { cwd, agentType: 'omp', sessionName: 'chat with chief bot', sessionFile: native } })
      server!.setHerdrBehavior({ agents: [{ agent: 'omp', name: `chief-${number}`, workspace_id: 'w-target', tab_id: `chief-tab-${number}`, pane_id: `chief-pane-${number}`, agent_session: { source: 'herdr:omp', agent: 'omp', kind: 'id', value: sessionId } }] })
      const opened = second.waitForResponse(response => response.url().endsWith('/api/chats/resume'))
      await second.getByRole('button', { name: 'chief bot', exact: true }).click()
      const located = await (await opened).json()
      expect(located.outcome).toBe('located')
      expect((await opened).request().postDataJSON()).toEqual({ sessionId })
      await expect(second.getByTestId('chat-creation-dialog')).toHaveCount(0)
      expect(server!.herdrTabs()).toHaveLength(number)
      expect(server!.herdrCalls().filter(call => call.kind === 'agent_focus').at(-1)?.argv).toEqual(['agent', 'focus', `chief-pane-${number}`])
      await post('/api/sessions/events', { sessionId, kind: 'unregister' })
      const definitions = (await server!.getTasks()).tasks
      expect(definitions).toEqual(expectedDefinitions)
      expect(definitions.filter(task => task.id === 'chief-bot')).toHaveLength(1)
      observations.push({ sessionId, located, definitions })
    }
    fs.writeFileSync(info.outputPath('chief-public-readback.json'), JSON.stringify(observations, null, 2))
  } finally { await other.close() }
})
