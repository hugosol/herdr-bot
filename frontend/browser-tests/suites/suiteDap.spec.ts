import fs from 'node:fs'
import path from 'node:path'
import { expect, test } from '@playwright/test'
import { startServer, type E2EServer } from '../support/support'

let server: E2EServer | null = null
test.afterEach(async () => { await server?.cleanup(); server = null })

test('OMP selects and repairs DAP references while Pi saves without DAP', async ({ page }, info) => {
  server = await startServer({ separateCwd: true })
  const authority = path.join(path.dirname(server.configPath), 'meta-data')
  const dapFile = path.join(authority, 'dap.json')
  const command = path.join(authority, 'CodeLLDB Windows x64', 'adapter', 'codelldb.exe')
  const description = '仅限 Windows x64，需手动解压完整 CodeLLDB 安装包。'
  const catalog = JSON.stringify({ adapters: { 'code-lldb-windows': { name: 'CodeLLDB', description, config: {
    command, languages: ['c', 'cpp', 'rust', 'zig'], args: ['--liblldb', 'native.dll'], connectMode: 'stdio',
  } } } })
  fs.mkdirSync(authority, { recursive: true })
  fs.writeFileSync(dapFile, catalog)

  await page.setViewportSize({ width: 1440, height: 900 })
  await page.goto(`${server.baseURL}?layout=expanded`)
  await page.getByTestId('tab-suite').click()
  await page.getByRole('button', { name: '创建 Suite', exact: true }).click()
  const dap = page.getByRole('group', { name: '集中 DAP 选择', exact: true })
  const option = dap.getByRole('checkbox', { name: 'CodeLLDB', exact: true })
  const details = dap.getByText('详情', { exact: true })
  const save = page.getByRole('button', { name: '保存 Suite', exact: true })
  const reload = page.getByRole('button', { name: '重新读取 Suite', exact: true })
  await expect(option).toBeVisible()
  await expect(dap.getByText(description, { exact: true })).toBeHidden()
  await expect(dap.getByText(command, { exact: true })).toBeHidden()
  await expect(dap.getByText('c、cpp、rust、zig', { exact: true })).toBeHidden()
  await details.click()
  await expect(dap.getByText(description, { exact: true })).toBeVisible()
  await expect(dap.getByText(command, { exact: true })).toBeVisible()
  await expect(dap.getByText('c、cpp、rust、zig', { exact: true })).toBeVisible()
  await expect(dap.getByText(/--liblldb/)).toBeVisible()
  await expect(dap.getByText(/不表示已安装或通过调试验证/)).toBeVisible()
  await option.check()
  for (const [name, width] of [['desktop', 1440], ['tablet', 768], ['mobile', 390]] as const) {
    await page.setViewportSize({ width, height: 900 })
    expect(await dap.evaluate(node => node.scrollWidth <= node.clientWidth)).toBe(true)
    const screenshot = info.outputPath(`dap-expanded-${name}.png`)
    await dap.screenshot({ path: screenshot })
    await info.attach(`dap-expanded-${name}`, { path: screenshot, contentType: 'image/png' })
  }

  await page.getByLabel('Suite ID', { exact: true }).fill('debug-omp')
  await page.getByLabel('Suite 名称', { exact: true }).fill('调试工具')
  const post = page.waitForResponse(response => response.url().endsWith('/api/suites') && response.request().method() === 'POST')
  await save.click()
  const saved = await post
  expect(saved.ok()).toBe(true)
  expect(saved.request().postDataJSON()).toMatchObject({ harness: 'omp', dap: ['code-lldb-windows'] })
  expect(fs.readFileSync(dapFile, 'utf8')).toBe(catalog)
  await page.reload()
  await page.getByTestId('tab-suite').click()
  await page.getByRole('button', { name: '调试工具 · OMP · debug-omp', exact: true }).click()
  await expect(option).toBeChecked()

  fs.writeFileSync(dapFile, '{"adapters":{}}')
  await reload.click()
  const stale = dap.getByRole('checkbox', { name: 'code-lldb-windows（失效：集中 DAP 定义不存在）', exact: true })
  await expect(stale).toBeChecked()
  await expect(save).toBeDisabled()
  const staleCatalog = await (await page.request.get(`${server.baseURL}/api/suites`)).json()
  expect(staleCatalog).toMatchObject({ dap: [], suites: [{ id: 'debug-omp', dap: ['code-lldb-windows'] }] })
  await info.attach('dap-missing-reference', { body: await dap.screenshot(), contentType: 'image/png' })
  await stale.click()
  await expect(stale).toHaveCount(0)
  await expect(save).toBeEnabled()
  await save.click()
  await expect(page.getByRole('status')).toHaveText('Suite 已保存到权威文件。')
  const current = await (await page.request.get(`${server.baseURL}/api/suites`)).json()
  expect(current.suites).toMatchObject([{ id: 'debug-omp', dap: [] }])

  fs.writeFileSync(dapFile, catalog)
  await reload.click()
  await expect(option).toBeVisible()
  await page.getByRole('button', { name: '创建 Suite', exact: true }).click()
  await option.check()
  await page.getByLabel('适用宿主', { exact: true }).selectOption('pi')
  await expect(dap).toHaveCount(0)
  await page.getByLabel('Suite ID', { exact: true }).fill('debug-pi')
  await page.getByLabel('Suite 名称', { exact: true }).fill('Pi 工具')
  const piPost = page.waitForResponse(response => response.url().endsWith('/api/suites') && response.request().method() === 'POST')
  await save.click()
  const piSaved = await piPost
  expect(piSaved.ok()).toBe(true)
  expect(piSaved.request().postDataJSON()).toMatchObject({ harness: 'pi' })
  expect(piSaved.request().postDataJSON()).not.toHaveProperty('dap')
})
