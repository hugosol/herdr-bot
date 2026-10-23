import { expect, test } from '@playwright/test'
import fs from 'node:fs'
import path from 'node:path'
import { startServer, type E2EServer } from './support'

let server: E2EServer | null = null
test.afterEach(async () => { await server?.cleanup(); server = null })

for (const width of [1280, 390]) {
  test(`shared multiline memo explicitly saves, cancels, recovers and survives restart at ${width}px`, async ({ page, browser }) => {
    await page.setViewportSize({ width, height: 900 })
    server = await startServer({ herdr: {} })
    const sessionId = 'memo-native-session'
    const root = path.dirname(server.configPath)
    const native = path.join(root, 'memo-native.jsonl')
    const nativeBytes = JSON.stringify({ type: 'session', id: sessionId, cwd: root }) + '\n'
    fs.writeFileSync(native, nativeBytes)
    const registered = await fetch(`${server.baseURL}/api/sessions/events`, {
      method: 'POST', headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ sessionId, kind: 'register', data: { sessionName: 'Shared memo chat', cwd: root, sessionFile: native, agentType: 'pi' } }),
    })
    expect(registered.status).toBe(200)
    const readMemo = async () => {
      const response = await fetch(`${server!.baseURL}/api/sessions/history`)
      expect(response.status).toBe(200)
      return (await response.json()).sessions[0].memo
    }
    await page.goto(server.baseURL)
    await page.getByRole('button', { name: '查看历史会话' }).click()
    const row = page.getByRole('row').filter({ hasText: 'Shared memo chat' })
    await row.getByRole('button', { name: '新增备忘' }).click()
    const editor = row.getByRole('textbox', { name: 'Shared memo chat的备忘' })
    await editor.fill('cancelled draft\nnot shared')
    expect(await readMemo()).toBeUndefined()
    await row.getByRole('button', { name: '取消', exact: true }).click()
    await expect(editor).toHaveCount(0)
    await row.getByRole('button', { name: '新增备忘' }).click()
    await expect(editor).toHaveValue('')

    const memo = '  exact leading spaces\n第二行 <not HTML>\ntrailing spaces  '
    await editor.fill(memo)
    const staging = path.join(root, 'session-history.json.tmp')
    fs.mkdirSync(staging)
    const { promise: gate, resolve: release } = Promise.withResolvers<void>()
    await page.route('**/api/sessions/history/memo', async (route) => {
      await gate
      await route.continue()
    }, { times: 1 })
    const failure = page.waitForResponse(response => response.url().endsWith('/api/sessions/history/memo') && response.request().method() === 'PUT')
    await row.getByRole('button', { name: '保存', exact: true }).click()
    try {
      await expect(row.getByRole('button', { name: '保存中…' })).toBeDisabled()
      await expect(row.getByRole('button', { name: '取消', exact: true })).toBeDisabled()
      await expect(editor).toBeDisabled()
    } finally { release() }
    expect((await failure).ok()).toBe(false)
    await expect(row.getByRole('alert')).toBeVisible()
    await expect(editor).toHaveValue(memo)
    expect(await readMemo()).toBeUndefined()
    fs.rmdirSync(staging)

    const saved = page.waitForResponse(response => response.url().endsWith('/api/sessions/history/memo') && response.request().method() === 'PUT')
    await row.getByRole('button', { name: '保存', exact: true }).click()
    expect((await saved).ok()).toBe(true)
    await expect(editor).toHaveCount(0)
    await expect(row.getByRole('alert')).toHaveCount(0)
    await expect(row.getByRole('button', { name: '编辑备忘' })).toBeVisible()
    await expect(row.locator('.history-memo__text')).toHaveText(memo)
    expect(await readMemo()).toBe(memo)
    await row.getByRole('button', { name: '编辑备忘' }).click()
    await expect(editor).toHaveValue(memo)
    await editor.fill('unsaved replacement')
    await row.getByRole('button', { name: '取消', exact: true }).click()
    expect(await readMemo()).toBe(memo)
    await page.reload()
    await page.getByRole('button', { name: '查看历史会话' }).click()
    await expect(row.locator('.history-memo__text')).toHaveText(memo)
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true)

    await server.restart()
    const device = await browser.newContext({ viewport: { width: width === 390 ? 1280 : 390, height: 900 } })
    try {
      const other = await device.newPage()
      await other.goto(server.baseURL)
      await other.getByRole('button', { name: '查看历史会话' }).click()
      const shared = other.getByRole('row').filter({ hasText: 'Shared memo chat' })
      await expect(shared.locator('.history-memo__text')).toHaveText(memo)
      await shared.getByRole('button', { name: '编辑备忘' }).click()
      await shared.getByRole('textbox').fill('')
      await shared.getByRole('button', { name: '保存', exact: true }).click()
      await expect(shared.getByRole('textbox')).toHaveCount(0)
      expect(await readMemo()).toBe('')
      await page.goto(server.baseURL)
      await page.getByRole('button', { name: '查看历史会话' }).click()
      await row.getByRole('button', { name: '编辑备忘' }).click()
      await expect(editor).toHaveValue('')
    } finally { await device.close() }
    expect(fs.readFileSync(native, 'utf8')).toBe(nativeBytes)
    expect(server.herdrTabs()).toEqual([])
    expect(server.herdrCalls().some(call => ['agent_start', 'agent_prompt'].includes(String(call.kind)))).toBe(false)
  })
}
