import { expect, test, type Page } from '@playwright/test'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { startServer, type E2EServer } from '../support/support'
import { zoneRowFor } from '../support/ui'

// Parallel cases run in separate worker processes; each keeps its own server and cleanup hooks.
test.describe.configure({ mode: 'parallel' })

let server: E2EServer | null = null
let root: string | null = null
test.afterEach(async () => {
  try { await server?.cleanup() }
  finally {
    server = null
    if (root) fs.rmSync(root, { recursive: true, force: true })
    root = null
  }
})
async function post(endpoint: string, body: unknown) {
  const response = await fetch(`${server!.baseURL}${endpoint}`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify(body) })
  expect(response.ok, await response.clone().text()).toBe(true)
  return response.json()
}
async function preparePendingQuery() {
  root = fs.mkdtempSync(path.join(os.tmpdir(), 'pi-zone-history-'))
  const gate = path.join(root, 'never-release')
  server = await startServer({ herdr: { behavior: { native_session_gate: { wait_for_file: gate, timeout_ms: 60_000 } } } })
  const zones = await post('/api/zones', { id: 'test/zone', path: root })
  await post('/api/tasks', { name: 'Pi history browser', chatName: 'Pi Browser Chat', agent: 'pi', model: 'default', thinking: 'default', skill: null, prompt: '' })
  return zones.zones[0].id as string
}
async function createInPage(page: Page, zone: string) {
  await page.goto(server!.baseURL)
  await page.getByTestId('tab-files').click()
  await zoneRowFor(page, zone).getByTestId('zone-create-chat').click()
  const dialog = page.getByTestId('chat-creation-dialog')
  await dialog.locator('#chat-workspace-target').selectOption('default')
  await dialog.getByTestId('chat-input').fill('/settings')
  await dialog.getByTestId('chat-create-submit').click()
  return dialog
}

// The caller never receives a cleanup handle when initialization rejects. The
// fixture must still remove only the temporary tree allocated by that attempt.
test('isolated browser server removes its temporary data when initialization rejects', async () => {
  const marker = `startup-cleanup-${process.pid}-${Date.now()}`
  const ownedDirectories = () => fs.readdirSync(os.tmpdir(), { withFileTypes: true })
    .filter(entry => entry.isDirectory() && entry.name.startsWith('herdr-bot browser '))
    .map(entry => path.join(os.tmpdir(), entry.name))
    .filter(directory => {
      try { return fs.readFileSync(path.join(directory, 'herdr-bot.toml'), 'utf8').includes(marker) }
      catch (error) {
        if ((error as NodeJS.ErrnoException).code === 'ENOENT') return false
        throw error
      }
    })
  try {
    // A quote makes this attempt's real TOML invalid; it does not alter any
    // executable or shared fixture, and the backend must exit before listening.
    await expect(startServer({ herdr: { workspaceName: `${marker}'` } })).rejects.toThrow('提前退出')
    expect(ownedDirectories()).toEqual([])
  } finally {
    for (const directory of ownedDirectories()) fs.rmSync(directory, { recursive: true, force: true })
  }
})

// A pending external query owns a Windows executable handle. Stopping only
// the backend parent leaves that child alive and makes cleanup fail with EPERM.
test('isolated browser server cleanup also stops its pending Herdr query child', async ({ page }) => {
  const zone = await preparePendingQuery()
  const dialog = await createInPage(page, zone)
  const owned = server!
  // The native report makes the backend verify the launch with agent get.
  // Leave that child blocked: creation cannot succeed until verification.
  await expect.poll(() => owned.herdrCalls().some(c => c.kind === 'agent_get' && typeof c.pid === 'number')).toBe(true)
  expect(owned.herdrCalls().filter(c => c.kind === 'agent_prompt')).toHaveLength(0)
  await expect(dialog.getByTestId('chat-create-success')).toHaveCount(0)
  // Keep afterEach ownership if cleanup fails; never kill a recycled record PID.
  await owned.cleanup()
  server = null
  expect(fs.existsSync(path.dirname(owned.configPath))).toBe(false)
})

