// Real delivered form -> production HTTP -> real native Chat. No API launch fallback.
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { chromium } from '../frontend/node_modules/@playwright/test/index.mjs'

const request = JSON.parse(fs.readFileSync(process.argv[2], 'utf8'))
const evidence = { request, http: [], cleanupErrors: [] }
let browser
let failed = false
try {
  evidence.browserProfile = fs.mkdtempSync(path.join(os.tmpdir(), 'nesting-files-chrome-'))
  browser = await chromium.launchPersistentContext(evidence.browserProfile, {
    executablePath: request.chrome, headless: true, timeout: 30000,
    viewport: { width: 1440, height: 1000 },
  })
  const page = await browser.newPage()
  page.on('response', async response => {
    if (!response.url().includes('/api/')) return
    const item = { url: response.url(), status: response.status(), method: response.request().method(), body: response.request().postData() }
    evidence.http.push(item)
    try { item.response = await response.text() } catch (error) { item.error = String(error) }
  })
  await page.goto(request.base + '?layout=expanded')
  await page.getByRole('tab', { name: 'scheduler', exact: true }).click()
  await page.getByRole('button', { name: request.mapTitle, exact: true }).click()
  await page.getByRole('button', { name: '+ 发起 Run', exact: true }).click()
  await page.getByRole('combobox', { name: '管理者 Task', exact: true }).selectOption(request.manager)
  await page.getByRole('combobox', { name: '管理者 zone', exact: true }).selectOption(request.zone)
  await page.getByRole('textbox', { name: '本次要求', exact: true }).fill(request.business)
  for (const file of request.files) {
    await page.getByRole('textbox', { name: `文件输入 ${file.id}（必需）`, exact: true }).fill(file.path)
  }
  evidence.confirmed = { business: await page.getByRole('textbox', { name: '本次要求', exact: true }).inputValue(), files: [] }
  for (const file of request.files) {
    evidence.confirmed.files.push({ id: file.id, path: await page.getByRole('textbox', { name: `文件输入 ${file.id}（必需）`, exact: true }).inputValue() })
  }
  await page.screenshot({ path: request.evidence + '.before.png', fullPage: true })
  await page.getByRole('button', { name: '校验启动配置', exact: true }).click()
  await page.getByRole('form', { name: '发起 Run', exact: true }).getByRole('status').filter({ hasText: '配置校验通过' }).waitFor()
  const started = page.waitForResponse(response => response.request().method() === 'POST' && response.url().endsWith('/runs'), { timeout: 90000 })
  await page.getByRole('button', { name: '启动 Run', exact: true }).click()
  const response = await started
  evidence.launched = { status: response.status(), body: await response.json() }
  if (response.status() !== 201 || evidence.launched.body.startupError || !evidence.launched.body.run?.manager) {
    throw new Error('Delivered page failed actual Chat startup: ' + JSON.stringify(evidence.launched))
  }
  await page.getByRole('heading', { name: 'Run 详情', exact: true }).waitFor({ timeout: 90000 })
  evidence.runId = page.url().split('#workflow/')[1]
  if (!evidence.runId) throw new Error('Delivered page did not expose actual Run identity')
  evidence.page = await page.locator('body').innerText()
  await page.screenshot({ path: request.evidence + '.after.png', fullPage: true })
} catch (error) {
  failed = true
  evidence.error = error.stack || String(error)
} finally {
  if (browser) {
    try { await browser.close() } catch (error) { failed = true; evidence.cleanupErrors.push(String(error)) }
  }
  fs.writeFileSync(request.evidence, JSON.stringify(evidence, null, 2))
}
console.log(JSON.stringify(evidence))
if (failed) process.exitCode = 1
