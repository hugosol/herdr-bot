import fs from 'node:fs'
import path from 'node:path'
import { expect, test } from '@playwright/test'
import { startServer, type E2EServer } from '../support/support'

let server: E2EServer | null = null
test.afterEach(async () => { await server?.cleanup(); server = null })

// S1/P3: real HTTP + existing record-only scripts exercise the real operation
// lock. A fabricated success, lost result, or queued busy request disagrees
// with the prewritten exit codes and external invocation record below.
test('P3: relocated server preserves success, failure, pending and authoritative busy feedback', async ({ page, context }, info) => {
  server = await startServer({ safeShutdown: true })
  await server.stop()
  const originalConfig = fs.readFileSync(server.configPath, 'utf8')
  const failureScript = path.resolve('../fixtures/operations/failure.bat')
  fs.writeFileSync(server.configPath, originalConfig.replace(/(\[operations.stop\]\nscript = ')[^']+'/, `$1${failureScript}'`))
  await server.restart()
  const recordRoot = path.join(path.dirname(server.configPath), 'operations')
  const records = () => fs.existsSync(path.join(recordRoot, 'calls.tsv'))
    ? fs.readFileSync(path.join(recordRoot, 'calls.tsv'), 'utf8').trim().split('\n').map(line => line.trim().split('\t').slice(3, 5))
    : []
  await page.goto(`${server.baseURL}?layout=expanded`)
  await page.getByTestId('server-button').click()
  await expect(page.getByTestId('server-page')).toContainText('不表示后台程序已经启动、已经就绪或已经彻底停止')
  await page.getByTestId('start-button').click()
  await expect(page.getByTestId('operation-result')).toHaveAttribute('data-outcome', 'completed')
  await expect(page.getByTestId('operation-result')).toHaveText('启动脚本调用已结束：脚本执行完成（退出码 0）。')
  expect(records()).toEqual([['success', 'invoke']])
  await page.getByTestId('server-back').click()
  await page.getByTestId('tab-scheduler').click()
  await page.getByTestId('server-button').click()
  await expect(page.getByTestId('operation-result')).toHaveAttribute('data-outcome', 'completed')
  await page.getByTestId('stop-button').click()
  await expect(page.getByTestId('operation-result')).toHaveAttribute('data-outcome', 'failed')
  await expect(page.getByTestId('operation-result')).toHaveText('停止脚本调用失败：脚本以退出码 3 结束。')
  expect(records()).toEqual([['success', 'invoke'], ['failure', 'invoke']])
  await server.stop()
  const heldScript = path.resolve('../fixtures/operations/long-running.bat')
  fs.writeFileSync(server.configPath, originalConfig.replace(/(\[operations.start\]\nscript = ')[^']+'/, `$1${heldScript}'`))
  await server.restart()
  await page.goto(`${server.baseURL}?layout=expanded`)
  await page.getByTestId('server-button').click()
  const ended = page.waitForResponse(response => response.url().endsWith('/api/operations/start'))
  await page.getByTestId('start-button').click()
  const other = await context.newPage()
  try {
    await expect.poll(records).toEqual([['success', 'invoke'], ['failure', 'invoke'], ['long-running', 'invoke']])
    await expect(page.getByTestId('pending-hint')).toContainText('本次操作尚未结束')
    await expect(page.getByTestId('start-button')).toBeDisabled()
    await expect(page.getByTestId('stop-button')).toBeDisabled()
    await other.goto(server.baseURL)
    await other.getByTestId('server-button').click()
    await other.getByTestId('stop-button').click()
    await expect(other.getByTestId('operation-result')).toHaveAttribute('data-outcome', 'busy')
    await expect(other.getByTestId('operation-result')).toContainText('本页面不会自动重试，也不会在这次调用结束后补做')
    await page.screenshot({ path: info.outputPath('server-pending.png'), fullPage: true })
    await other.screenshot({ path: info.outputPath('server-busy.png'), fullPage: true })
  } finally {
    fs.mkdirSync(path.join(recordRoot, 'stop'), { recursive: true })
    fs.writeFileSync(path.join(recordRoot, 'stop', 'long-running.stop'), 'release')
    await ended
  }
  await expect(page.getByTestId('operation-result')).toHaveAttribute('data-outcome', 'completed')
  await expect(other.getByTestId('operation-result')).toHaveAttribute('data-outcome', 'busy')
  expect(records()).toEqual([['success', 'invoke'], ['failure', 'invoke'], ['long-running', 'invoke'], ['long-running', 'stop-observed']])
  await expect(page.getByTestId('shutdown-button')).toBeVisible()
  await page.getByTestId('shutdown-button').click()
  await expect(page.getByTestId('shutdown-confirm')).toBeVisible()
  await page.getByTestId('shutdown-back').click()
  await expect(page.getByTestId('server-page')).toBeVisible()
})

