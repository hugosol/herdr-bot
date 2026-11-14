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
async function prepare(behavior: Record<string, unknown>) {
  root = fs.mkdtempSync(path.join(os.tmpdir(), 'pi-zone-history-'))
  server = await startServer({ herdr: { behavior } })
  const zones = await post('/api/zones', { id: 'test/zone', path: root })
  const saved = await post('/api/tasks', { name: 'Pi history browser', chatName: 'Pi Browser Chat', agent: 'pi', model: 'default', thinking: 'default', skill: null, prompt: '' })
  return { zone: zones.zones[0].id as string, task: saved.tasks.find((t: {name:string}) => t.name === 'Pi history browser').id as string }
}
async function createInPage(page: Page, zone: string) {
  await page.goto(server!.baseURL)
  await page.getByTestId('tab-files').click()
  await zoneRowFor(page, zone).getByTestId('zone-create-chat').click()
  const dialog = page.getByTestId('chat-creation-dialog')
  await dialog.locator('#chat-workspace-target').selectOption('default')
  await dialog.getByTestId('chat-input').fill('/settings')
  await dialog.getByTestId('chat-create-submit').click()
  await expect(dialog.getByTestId('chat-create-success')).toHaveText('创建成功，初始指示已提交')
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
  const { zone } = await prepare({})
  server!.setHerdrBehavior({ native_session_gate: { wait_for_file: path.join(root!, 'never-release'), timeout_ms: 60_000 } })
  await createInPage(page, zone)
  await expect.poll(() => server!.herdrCalls().some(c => c.kind === 'agent_get')).toBe(true)
  const owned = server!
  // Keep afterEach ownership if cleanup fails; never kill a recycled record PID.
  await owned.cleanup()
  server = null
  expect(fs.existsSync(path.dirname(owned.configPath))).toBe(false)
})

