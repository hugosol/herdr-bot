import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { test, expect } from '@playwright/test'
import { startServer, type E2EServer } from '../support/support'

test.use({ launchOptions: process.env.RUST_SCRIPT_CHROME ? { executablePath: process.env.RUST_SCRIPT_CHROME } : {} })

// Approved P23 page partition: delivered page + public complete definitions.
// Oracle: shipped creator keeps its identity/guides, dedicated saver has three
// empty strings; create and revise Maps must reference different save abilities.
// A page showing a stale Task target or a globally migrated replace-text fails.
test('P23 migrated creator and dedicated saver match complete public definitions and create/revise references', async ({ page, browser }, info) => {
  const root = path.resolve('..')
  const home = fs.mkdtempSync(path.join(os.tmpdir(), 'rust-migration-page-'))
  const keys = ['HOME', 'USERPROFILE', 'APPDATA', 'LOCALAPPDATA', 'XDG_CONFIG_HOME', 'XDG_DATA_HOME', 'XDG_STATE_HOME', 'OMP_CODING_AGENT_DIR', 'PI_CODING_AGENT_DIR']
  const previous = Object.fromEntries(keys.map(key => [key, process.env[key]]))
  let server: E2EServer | undefined
  const record: Record<string, unknown> = { browser: browser.version(), passed: false, processesExited: false }
  try {
    for (const key of keys) { process.env[key] = path.join(home, key); fs.mkdirSync(process.env[key]!) }
    server = await startServer({ separateCwd: true, herdr: {} })
    for (const tree of ['bots', 'workflow-maps']) fs.cpSync(path.join(root, 'meta-data', tree), path.join(server.cwd, 'meta-data', tree), { recursive: true })
    const expected: Record<string, unknown> = {}
    for (const [file, id, capability] of [
      ['bot-1793348060748020300-25960-2.json', 'create-blank-bot', 'create_blank_bot'],
      ['save-created-bot-guides.json', 'save-created-bot-guides', 'save_bot_guides'],
    ]) {
      const shipped = JSON.parse(fs.readFileSync(path.join(root, 'meta-data/bots', file), 'utf8'))
      expect(shipped.id).toBe(id)
      expect(shipped.rust_script).toBe(capability)
      expect(shipped.task_id).toBeUndefined()
      expect(shipped.map_file).toBeUndefined()
      if (id === 'save-created-bot-guides') for (const field of ['when', 'plan_guide', 'node_guide']) expect(shipped[field]).toBe('')
      const response = await page.request.post(`${server.baseURL}/api/scheduler/bots/edit`, { data: { id } })
      expect(response.status()).toBe(200)
      const actual = await response.json()
      expect(JSON.parse(actual.revision.original)).toEqual(shipped)
      const { id: _id, ...content } = shipped
      expect(actual.content).toEqual(content)
      expected[id] = actual
      await page.goto(`${server.baseURL}?layout=expanded`)
      await page.getByTestId('tab-bot').click()
      await page.getByRole('button', { name: `${shipped.name} · ${id}`, exact: true }).click()
      const preview = page.getByRole('region', { name: 'Bot 预览', exact: true })
      await expect(preview).toContainText(capability)
      for (const field of ['when', 'plan_guide', 'node_guide']) {
        await preview.getByRole('button', { name: field, exact: true }).click()
        await expect(preview.getByRole('region', { name: field, exact: true })).toHaveText(shipped[field])
      }
      await page.screenshot({ path: info.outputPath(`${id}.png`), fullPage: true })
      await page.getByRole('button', { name: 'edit', exact: true }).click()
      // Group is a separate editor control, not part of the content textarea.
      const { group: _group, ...editable } = actual.content
      expect(JSON.parse(await page.getByLabel('Bot 内容 JSON', { exact: true }).inputValue())).toEqual(editable)
      await expect(page.getByRole('combobox', { name: '固定 Rust 能力', exact: true })).toHaveValue(capability)
    }
    const maps: Record<string, unknown> = {}
    for (const file of ['create-bot-wrapper.json', 'enhance-existing-bot--bc-301b25d9f8d6.json']) {
      const response = await page.request.get(`${server.baseURL}/api/scheduler/maps/${file}`)
      expect(response.status()).toBe(200)
      const actual = await response.json()
      const shippedMap = JSON.parse(fs.readFileSync(path.join(root, 'meta-data/workflow-maps', file), 'utf8'))
      // Public Map serialization omits the optional empty input requirements.
      if (shippedMap.input_requirements === '') delete shippedMap.input_requirements
      expect(actual).toEqual(shippedMap)
      const references = Object.values(actual.nodes).map(node => {
        if (!node || typeof node !== 'object' || !('bot_id' in node) || typeof node.bot_id !== 'string') throw new Error('Missing actual Map Bot reference')
        return node.bot_id
      })
      if (file === 'create-bot-wrapper.json') {
        expect(actual.nodes.create_blank.bot_id).toBe('create-blank-bot')
        expect(actual.nodes.save_bot.bot_id).toBe('save-created-bot-guides')
      } else {
        expect(references).toContain('replace-text')
        expect(references).not.toContain('save-created-bot-guides')
      }
      maps[file] = actual
      await page.goto(`${server.baseURL}?layout=expanded`)
      await page.getByTestId('tab-scheduler').click()
      await page.getByRole('button', { name: actual.title, exact: true }).click()
      await expect(page.getByRole('region', { name: 'Map 预览', exact: true })).toBeVisible()
      await page.screenshot({ path: info.outputPath(`${file}.png`), fullPage: true })
    }
    expect(server.herdrCalls()).toEqual([])
    expect(server.herdrTabs()).toEqual([])
    Object.assign(record, { expected, maps, passed: true, calls: server.herdrCalls(), tabs: server.herdrTabs() })
  } finally {
    record.diagnostics = server?.diagnostics()
    try { await server?.cleanup(); record.processesExited = true; fs.rmSync(home, { recursive: true, force: true }) }
    finally {
      fs.writeFileSync(info.outputPath('P23-evidence.json'), JSON.stringify(record, null, 2))
      const destination = process.env.RUST_SCRIPT_ACCEPTANCE_DIR
      if (destination) { fs.mkdirSync(destination, { recursive: true }); fs.cpSync(info.outputDir, path.join(destination, 'P23'), { recursive: true }) }
      for (const key of keys) { if (previous[key] === undefined) delete process.env[key]; else process.env[key] = previous[key] }
    }
  }
})
