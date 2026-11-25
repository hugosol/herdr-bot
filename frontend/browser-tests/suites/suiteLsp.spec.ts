import fs from 'node:fs'
import path from 'node:path'
import { expect, test } from '@playwright/test'
import { startServer, type E2EServer } from '../support/support'

let server: E2EServer | null = null
test.afterEach(async () => { await server?.cleanup(); server = null })

// S1/P38: a saved declaration for an absent executable must not be presented
// as a running service. Independent source: the ticket's declared != running
// rule. Native absence/process evidence is exercised by the real OMP runner.
test('P38 distinguishes an LSP declaration from a running native language service', async ({ page }) => {
  server = await startServer({ separateCwd: true })
  const authority = path.join(path.dirname(server.configPath), 'meta-data')
  fs.mkdirSync(path.join(authority, 'lsp'), { recursive: true })
  fs.writeFileSync(path.join(authority, 'lsp', 'rust-analyzer.json'), JSON.stringify({
    name: 'rust-analyzer', description: 'Rust 声明，不表示已安装或正在运行。',
    config: { command: path.join(authority, 'never-installed-language-server.exe'),
      args: [], fileTypes: ['.rs'], rootMarkers: ['Cargo.toml'] },
  }))
  fs.writeFileSync(path.join(authority, 'suites.json'), JSON.stringify({ suites: [{
    id: 'missing-lsp-omp', name: '缺程序的声明 · OMP', harness: 'omp', skills: [], extensions: [], mcp: [],
    lsp: ['rust-analyzer'],
  }] }))
  await page.goto(`${server.baseURL}?layout=expanded`)
  await page.getByTestId('tab-suite').click()
  await page.getByRole('button', { name: '缺程序的声明 · OMP · missing-lsp-omp', exact: true }).click()
  const overview = page.getByRole('region', { name: 'Suite 概览', exact: true })
  const description = overview.getByText('Rust 声明，不表示已安装或正在运行。', { exact: true })
  await expect(overview.getByRole('checkbox')).toHaveCount(0)
  await expect(overview.getByText('rust-analyzer', { exact: true }).first()).toBeVisible()
  await expect(description).toBeHidden()
  await overview.getByLabel('rust-analyzer 详情', { exact: true }).click()
  await expect(description).toBeVisible()
  await overview.getByLabel('LSP 说明', { exact: true }).click()
  await expect(overview.getByText(/保留默认、用户和项目配置的原生优先级.*不是语言服务白名单.*不会自动安装/)).toBeVisible()
})
