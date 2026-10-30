import fs from 'node:fs'
import { execFileSync } from 'node:child_process'
import path from 'node:path'
import { expect, test } from '@playwright/test'
import { startServer, type E2EServer } from './support'
import { zoneRowFor } from './ui'

let server: E2EServer | null = null
test.afterEach(async () => { await server?.cleanup(); server = null })

// Approved seams: S1 delivered page and S2 the exact HTTP interface it uses.
// P7 source: the approved default is meta-data/workflow-maps, created without samples.
// A plausible violation is a hard-coded empty UI or scanning a different directory:
// observe the first empty read, then add a literal file and reload through both seams.
test('P7 Map default directory is created empty and preserves user files', async ({ page }) => {
  server = await startServer()
  const maps = path.join(server.cwd, 'meta-data', 'workflow-maps')
  expect(fs.existsSync(maps)).toBe(false)
  await page.goto(`${server.baseURL}?layout=expanded`)
  await page.getByTestId('tab-scheduler').click()
  await expect(page.getByTestId('scheduler-empty')).toBeVisible()
  expect(fs.readdirSync(maps)).toEqual([])
  const firstRead = await page.request.get(`${server.baseURL}/api/scheduler/maps`)
  expect(firstRead.status()).toBe(200)
  expect(await firstRead.json()).toEqual({ files: [] })
  const bytes = '{"user":"keep this exact content"}\n'
  fs.writeFileSync(path.join(maps, 'personal.json'), bytes)
  await page.getByRole('button', { name: '刷新全部' }).click()
  await expect(page.getByTestId('scheduler-map-list').getByRole('listitem')).toHaveText(['personal.json'])
  const response = await page.request.get(`${server.baseURL}/api/scheduler/maps`)
  expect(response.status()).toBe(200)
  expect(await response.json()).toEqual({ files: ['personal.json'] })
  expect(fs.readFileSync(path.join(maps, 'personal.json'), 'utf8')).toBe(bytes)
})

// S1/S2: real file timestamps distinguish newest-first order from filename or
// creation order, including filename ties and an explicit refresh after retiming.
test('Maps list newest modifications first with filename ties and refresh after timestamp changes', async ({ page }) => {
  server = await startServer()
  await page.goto(`${server.baseURL}?layout=expanded`)
  await page.getByTestId('tab-scheduler').click()
  await expect(page.getByTestId('scheduler-empty')).toBeVisible()
  const maps = path.join(server.cwd, 'meta-data', 'workflow-maps')
  const older = new Date('2026-01-01T00:00:00Z')
  const newer = new Date('2026-01-02T00:00:00Z')
  const newest = new Date('2026-01-03T00:00:00Z')
  const originals = new Map([
    ['zeta.json', '{"title":"zeta","description":"","nodes":{}}\n'],
    ['alpha.json', '{"title":"alpha","description":"","nodes":{}}\n'],
    ['beta.JSON', '{"title":"beta","description":"","nodes":{}}\n'],
  ])
  for (const [name, bytes] of originals) {
    const file = path.join(maps, name)
    fs.writeFileSync(file, bytes)
    const modified = name === 'alpha.json' ? older : newer
    fs.utimesSync(file, modified, modified)
  }
  await page.getByRole('button', { name: '刷新全部', exact: true }).click()
  await expect(page.getByTestId('scheduler-map-list').getByRole('listitem')).toHaveText(['beta', 'zeta', 'alpha'])
  const initial = await page.request.get(`${server.baseURL}/api/scheduler/maps`)
  expect(initial.status()).toBe(200)
  expect(await initial.json()).toEqual({ files: ['beta.JSON', 'zeta.json', 'alpha.json'], titles: { 'alpha.json': 'alpha', 'beta.JSON': 'beta', 'zeta.json': 'zeta' } })

  fs.utimesSync(path.join(maps, 'alpha.json'), newest, newest)
  await page.getByRole('button', { name: '刷新全部', exact: true }).click()
  await expect(page.getByTestId('scheduler-map-list').getByRole('listitem')).toHaveText(['alpha', 'beta', 'zeta'])
  const refreshed = await page.request.get(`${server.baseURL}/api/scheduler/maps`)
  expect(refreshed.status()).toBe(200)
  expect(await refreshed.json()).toEqual({ files: ['alpha.json', 'beta.JSON', 'zeta.json'], titles: { 'alpha.json': 'alpha', 'beta.JSON': 'beta', 'zeta.json': 'zeta' } })
  for (const [name, bytes] of originals) expect(fs.readFileSync(path.join(maps, name), 'utf8')).toBe(bytes)
})

test('Map titles display and filter independently of filenames with duplicate selection fallback and refresh', async ({ page }) => {
  server = await startServer()
  const maps = path.join(server.cwd, 'meta-data', 'workflow-maps')
  fs.mkdirSync(maps, { recursive: true })
  const modified = new Date('2026-01-01T00:00:00Z')
  const originals = new Map([
    ['first.json', JSON.stringify({ title: 'Shared itinerary', description: 'First file identity', nodes: {} })],
    ['second.json', JSON.stringify({ title: 'Shared itinerary', description: 'Second file identity', nodes: {} })],
    ['empty.json', JSON.stringify({ title: '', description: '', nodes: {} })],
    ['whitespace.json', JSON.stringify({ title: ' \t\n ', description: '', nodes: {} })],
    ['invalid.json', '{"title":"Must not display invalid title","description":"","nodes":[] }'],
  ])
  for (const [name, bytes] of originals) {
    const file = path.join(maps, name)
    fs.writeFileSync(file, bytes)
    fs.utimesSync(file, modified, modified)
  }
  await page.goto(`${server.baseURL}?layout=expanded`)
  await page.getByTestId('tab-scheduler').click()
  const list = page.getByTestId('scheduler-map-list')
  await expect(list.getByRole('button')).toHaveText(['empty.json', 'Shared itinerary', 'invalid.json', 'Shared itinerary', 'whitespace.json'])
  const response = await page.request.get(`${server.baseURL}/api/scheduler/maps`)
  expect(response.status()).toBe(200)
  expect(await response.json()).toEqual({
    files: ['empty.json', 'first.json', 'invalid.json', 'second.json', 'whitespace.json'],
    titles: { 'first.json': 'Shared itinerary', 'second.json': 'Shared itinerary' },
  })
  const filter = page.getByLabel('筛选 Map', { exact: true })
  await filter.fill('SHARED ITINERARY')
  await expect(list.getByRole('button')).toHaveText(['Shared itinerary', 'Shared itinerary'])
  await filter.fill('SECOND.JSON')
  await expect(list.getByRole('button')).toHaveText(['Shared itinerary'])
  await filter.clear()
  const first = list.locator('[data-map-file="first.json"]')
  const second = list.locator('[data-map-file="second.json"]')
  const preview = page.getByRole('region', { name: 'Map 预览', exact: true })
  await first.click()
  await expect(preview).toContainText('First file identity')
  await expect(first).toHaveAttribute('aria-pressed', 'true')
  await expect(second).toHaveAttribute('aria-pressed', 'false')
  await second.click()
  await expect(preview).toContainText('Second file identity')
  await expect(second).toHaveAttribute('aria-pressed', 'true')
  await expect(first).toHaveAttribute('aria-pressed', 'false')
  await list.getByRole('button', { name: 'invalid.json', exact: true }).click()
  await expect(page.getByRole('alert')).toContainText('invalid.json')
  await second.click()
  await expect(preview).toContainText('Second file identity')

  const updated = JSON.stringify({ title: 'Updated itinerary', description: 'Second file identity', nodes: {} })
  fs.writeFileSync(path.join(maps, 'second.json'), updated)
  fs.utimesSync(path.join(maps, 'second.json'), modified, modified)
  await page.getByRole('button', { name: '刷新全部', exact: true }).click()
  await expect(list.getByRole('button')).toHaveText(['empty.json', 'Shared itinerary', 'invalid.json', 'Updated itinerary', 'whitespace.json'])
  await expect(second).toHaveAttribute('aria-pressed', 'true')
  await expect(preview.getByRole('heading', { name: 'Updated itinerary', exact: true })).toBeVisible()
  await expect(preview).toContainText('Second file identity')
  const refreshed = await page.request.get(`${server.baseURL}/api/scheduler/maps`)
  expect(await refreshed.json()).toEqual({
    files: ['empty.json', 'first.json', 'invalid.json', 'second.json', 'whitespace.json'],
    titles: { 'first.json': 'Shared itinerary', 'second.json': 'Updated itinerary' },
  })
  for (const [name, bytes] of originals) {
    expect(fs.readFileSync(path.join(maps, name), 'utf8')).toBe(name === 'second.json' ? updated : bytes)
  }
})

// P5/P6/P9: literal whitelist, with distinct cwd/config, bots, zone and docs
// decoys. Resolving relative to the config, recursing, or using zone-local files
// would each produce a different list from ['alpha.json', 'beta.JSON'].
test('P5 P6 P9 two browsers share only configured top-level Maps regardless of zone', async ({ browser }) => {
  server = await startServer({ separateCwd: true })
  const root = path.dirname(server.configPath)
  const maps = path.join(server.cwd, 'selected-maps')
  fs.mkdirSync(path.join(maps, 'nested'), { recursive: true })
  const originals = new Map([
    ['alpha.json', '{"literal":"alpha"}\n'], ['beta.JSON', '{"literal":"beta"}\n'],
    ['notes.txt', 'not a Map'], ['nested/hidden.json', '{}'],
  ])
  const mapTime = new Date('2026-01-01T00:00:00Z')
  for (const [name, bytes] of originals) {
    const file = path.join(maps, name)
    fs.writeFileSync(file, bytes)
    fs.utimesSync(file, mapTime, mapTime)
  }
  for (const directory of [path.join(root, 'selected-maps'), path.join(server.cwd, 'shared-bots'), path.join(server.cwd, 'docs', 'example')]) {
    fs.mkdirSync(directory, { recursive: true })
    fs.writeFileSync(path.join(directory, 'excluded.json'), '{}')
  }
  fs.appendFileSync(server.configPath, "\n[scheduler]\nmap_directory = 'selected-maps'\nbot_directory = 'shared-bots'\n")
  await server.restart()
  const first = await browser.newContext()
  const second = await browser.newContext()
  try {
    const a = await first.newPage()
    const b = await second.newPage()
    for (const page of [a, b]) {
      await page.goto(`${server.baseURL}?layout=expanded`)
      await page.getByTestId('tab-scheduler').click()
      await expect(page.getByTestId('scheduler-map-list').getByRole('listitem')).toHaveText(['alpha.json', 'beta.JSON'])
      const response = await page.request.get(`${server.baseURL}/api/scheduler/maps`)
      expect(await response.json()).toEqual({ files: ['alpha.json', 'beta.JSON'] })
    }
    for (const name of ['zone-one', 'zone-two']) {
      const zone = path.join(server.cwd, name)
      fs.mkdirSync(zone)
      fs.writeFileSync(path.join(zone, 'zone-only.json'), '{}')
      const registered = await a.request.post(`${server.baseURL}/api/zones`, { data: { path: zone } })
      expect(registered.status()).toBe(200)
      await a.getByTestId('tab-files').click()
      await zoneRowFor(a, (await registered.json()).zones.find((item: { path: string }) => item.path.toLowerCase().endsWith(name))!.path).getByTestId('open-zone').click()
      await a.getByTestId('tab-scheduler').click()
      await expect(a.getByTestId('scheduler-map-list').getByRole('listitem')).toHaveText(['alpha.json', 'beta.JSON'])
      await a.getByTestId('tab-files').click()
      await a.getByRole('button', { name: '返回 zone 列表' }).click()
      await a.getByTestId('tab-scheduler').click()
    }
    for (const [name, bytes] of originals) expect(fs.readFileSync(path.join(maps, name), 'utf8')).toBe(bytes)
  } finally { await first.close(); await second.close() }
})

// P7/P8: a missing multi-level parent must be prepared, whereas a normal file
// occupying the configured path is a real creation error, never an empty list.
test('P7 P8 missing parents are created and an obstructing file produces a visible creation error', async ({ page }) => {
  server = await startServer({ separateCwd: true })
  const maps = path.join(server.cwd, 'missing', 'parents', 'maps')
  fs.appendFileSync(server.configPath, "\n[scheduler]\nmap_directory = 'missing/parents/maps'\nbot_directory = 'missing/parents/bots'\n")
  await server.restart()
  // Independent catalog scopes may initialize their shared missing parents at
  // the same time. Neither request may turn the other's mkdir into a 500.
  const initialCatalogs = await Promise.all(Array.from({ length: 8 }, (_, index) =>
    page.request.get(`${server.baseURL}/api/scheduler/${index % 2 === 0 ? 'maps' : 'bots'}`)))
  for (const response of initialCatalogs) expect(response.status()).toBe(200)
  await page.goto(`${server.baseURL}?layout=expanded`)
  await page.getByTestId('tab-scheduler').click()
  await expect(page.getByTestId('scheduler-empty')).toBeVisible()
  expect(fs.readdirSync(maps)).toEqual([])
  fs.rmdirSync(maps)
  fs.writeFileSync(maps, 'DO NOT OVERWRITE THIS FILE')
  await page.getByRole('button', { name: '刷新全部' }).click()
  await expect(page.getByTestId('scheduler-error')).toContainText('创建失败')
  await expect(page.getByTestId('scheduler-empty')).toHaveCount(0)
  const failed = await page.request.get(`${server.baseURL}/api/scheduler/maps`)
  expect(failed.status()).toBe(500)
  expect(await failed.json()).toMatchObject({ code: 'map-directory-create-failed' })
  expect(fs.readFileSync(maps, 'utf8')).toBe('DO NOT OVERWRITE THIS FILE')
  fs.unlinkSync(maps)
  fs.mkdirSync(maps)
  fs.writeFileSync(path.join(maps, 'readme.txt'), 'not JSON')
  await page.getByRole('button', { name: '刷新全部' }).click()
  await expect(page.getByTestId('scheduler-empty')).toBeVisible()
  const empty = await page.request.get(`${server.baseURL}/api/scheduler/maps`)
  expect(empty.status()).toBe(200)
  expect(await empty.json()).toEqual({ files: [] })
})

// P8: deny the actual Windows list-directory permission, leaving the production
// reader in place. An implementation swallowing read_dir errors would show empty
// or stale files; both are explicitly absent during the failed read.
test('P8 actual unreadable directory reports a read error and can recover', async ({ page }) => {
  server = await startServer()
  const maps = path.join(server.cwd, 'meta-data', 'workflow-maps')
  await page.goto(`${server.baseURL}?layout=expanded`)
  await page.getByTestId('tab-scheduler').click()
  await expect(page.getByTestId('scheduler-empty')).toBeVisible()
  fs.writeFileSync(path.join(maps, 'before-denial.json'), '{}')
  await page.getByRole('button', { name: '刷新全部' }).click()
  await expect(page.getByTestId('scheduler-map-list')).toHaveText('before-denial.json')
  const user = execFileSync('whoami.exe', [], { encoding: 'utf8' }).trim()
  try {
    execFileSync('icacls.exe', [maps, '/deny', `${user}:(RD)`])
    expect(() => fs.readdirSync(maps)).toThrow()
    await page.getByRole('button', { name: '刷新全部' }).click()
    await expect(page.getByTestId('scheduler-error')).toContainText('读取失败')
    await expect(page.getByTestId('scheduler-empty')).toHaveCount(0)
    await expect(page.getByTestId('scheduler-map-list')).toHaveCount(0)
    const failed = await page.request.get(`${server.baseURL}/api/scheduler/maps`)
    expect(failed.status()).toBe(500)
    expect(await failed.json()).toMatchObject({ code: 'map-directory-read-failed' })
  } finally {
    execFileSync('icacls.exe', [maps, '/remove:d', user])
  }
  await page.getByRole('button', { name: '刷新全部' }).click()
  await expect(page.getByTestId('scheduler-map-list')).toHaveText('before-denial.json')
})

// Required production boundary, complementary to ticket 08's full security suite:
// requests cannot select a path, and real junctions at entry/root/ancestor cannot
// expose the independently seeded outside filename or content.
test('Map listing rejects path injection and real directory junctions without writes or execution', async ({ page }) => {
  server = await startServer()
  const maps = path.join(server.cwd, 'meta-data', 'workflow-maps')
  const outside = path.join(server.cwd, 'outside')
  fs.mkdirSync(outside)
  fs.writeFileSync(path.join(outside, 'outside-secret.json'), 'PRIVATE OUTSIDE BYTES')
  await page.goto(`${server.baseURL}?layout=expanded`)
  await page.getByTestId('tab-scheduler').click()
  await expect(page.getByTestId('scheduler-empty')).toBeVisible()
  fs.writeFileSync(path.join(maps, 'inside.json'), 'KEEP INSIDE BYTES')
  fs.symlinkSync(outside, path.join(maps, 'linked.json'), 'junction')
  await page.getByRole('button', { name: '刷新全部' }).click()
  await expect(page.getByTestId('scheduler-map-list').getByRole('listitem')).toHaveText(['inside.json'])
  for (const query of [`path=${encodeURIComponent(outside)}`, 'zone=anything', 'path=..%2Foutside', 'path=C%3A%5CWindows']) {
    const rejected = await page.request.get(`${server.baseURL}/api/scheduler/maps?${query}`)
    expect(rejected.status()).toBe(400)
    expect(await rejected.json()).toMatchObject({ code: 'invalid-request' })
  }
  const body = await page.request.get(`${server.baseURL}/api/scheduler/maps`, { data: { path: outside } })
  expect(body.status()).toBe(400)
  expect((await page.request.post(`${server.baseURL}/api/scheduler/maps`, { data: { path: outside } })).status()).toBe(405)
  fs.renameSync(maps, `${maps}-saved`)
  try {
    fs.symlinkSync(outside, maps, 'junction')
    await page.getByRole('button', { name: '刷新全部' }).click()
    await expect(page.getByTestId('scheduler-error')).toContainText('不能经过')
    const rejected = await page.request.get(`${server.baseURL}/api/scheduler/maps`)
    expect(rejected.status()).toBe(500)
    const rejection = await rejected.json()
    expect(rejection).toMatchObject({ code: 'map-directory-link-rejected' })
    expect(JSON.stringify(rejection)).not.toContain('outside-secret')
    expect(JSON.stringify(rejection)).not.toContain('PRIVATE OUTSIDE BYTES')
    await expect(page.getByTestId('scheduler-map-list')).toHaveCount(0)
  } finally {
    if (fs.existsSync(maps)) fs.unlinkSync(maps)
    fs.renameSync(`${maps}-saved`, maps)
  }
  fs.appendFileSync(server.configPath, "\n[scheduler]\nmap_directory = 'linked-parent/maps'\n")
  fs.symlinkSync(outside, path.join(server.cwd, 'linked-parent'), 'junction')
  await server.restart()
  await page.reload()
  await page.getByTestId('tab-scheduler').click()
  await expect(page.getByTestId('scheduler-error')).toContainText('不能经过')
  const ancestor = await page.request.get(`${server.baseURL}/api/scheduler/maps`)
  expect(ancestor.status()).toBe(500)
  expect(await ancestor.json()).toMatchObject({ code: 'map-directory-link-rejected' })
  expect(fs.existsSync(path.join(outside, 'maps'))).toBe(false)
  expect(fs.readFileSync(path.join(outside, 'outside-secret.json'), 'utf8')).toBe('PRIVATE OUTSIDE BYTES')
  expect(fs.readFileSync(path.join(maps, 'inside.json'), 'utf8')).toBe('KEEP INSIDE BYTES')
  expect(fs.existsSync(path.join(server.cwd, 'operations'))).toBe(false)
  expect(fs.existsSync(path.join(server.cwd, 'extension-executed'))).toBe(false)
})
