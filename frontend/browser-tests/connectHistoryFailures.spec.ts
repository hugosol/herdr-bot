import { expect, test } from '@playwright/test'
import fs from 'node:fs'
import path from 'node:path'
import { startServer } from './support'

// S1/P21: a fresh read after a previously successful view must show failure,
// never an empty collection or old rows presented as freshly read history.
test('history read failure is visible and cannot impersonate a fresh successful read', async ({ page }) => {
  const server = await startServer()
  const historyFile = path.join(path.dirname(server.storageFile), 'session-history.json')
  try {
    const registered = await fetch(`${server.baseURL}/api/sessions/events`, {
      method: 'POST', headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ sessionId: 'browser-preserved-native-id', kind: 'register', data: { cwd: 'C:/literal/browser', sessionName: 'Preserved conversation', agentType: 'pi' } }),
    })
    expect(registered.status).toBe(200)
    await page.goto(server.baseURL)
    await page.getByRole('button', { name: '查看历史会话' }).click()
    await expect(page.getByRole('cell', { name: 'Preserved conversation', exact: true })).toBeVisible()
    const original = fs.readFileSync(historyFile)
    await page.getByRole('button', { name: '查看当前会话' }).click()
    fs.writeFileSync(historyFile, 'invalid history content\n')
    const failedRead = page.waitForResponse(response => response.url().includes('/api/sessions/history'))
    await page.getByRole('button', { name: '查看历史会话' }).click()
    expect((await failedRead).status()).toBe(500)
    await expect(page.getByRole('alert')).toContainText('读取历史会话失败')
    await expect(page.getByText('尚无历史会话', { exact: false })).toHaveCount(0)
    await expect(page.getByRole('cell', { name: 'Preserved conversation', exact: true })).toHaveCount(0)
    await page.screenshot({ path: test.info().outputPath('history-read-failed.png'), fullPage: true })
    fs.writeFileSync(historyFile, original)
    await page.getByRole('button', { name: '查看当前会话' }).click()
    await page.getByRole('button', { name: '查看历史会话' }).click()
    await expect(page.getByRole('cell', { name: 'Preserved conversation', exact: true })).toBeVisible()
    await expect(page.getByRole('alert')).toHaveCount(0)
  } finally {
    await server.cleanup()
  }
})
