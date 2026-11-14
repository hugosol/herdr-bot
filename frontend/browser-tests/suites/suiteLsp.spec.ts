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
  fs.mkdirSync(authority, { recursive: true })
  fs.writeFileSync(path.join(authority, 'suites.json'), JSON.stringify({ suites: [{
    id: 'missing-lsp', name: '缺程序的声明', agents: ['omp'],
    lsp: { 'rust-analyzer': { command: path.join(authority, 'never-installed-language-server.exe'),
      args: [], fileTypes: ['.rs'], rootMarkers: ['Cargo.toml'] } },
  }] }))
  await page.goto(`${server.baseURL}?layout=expanded`)
  await page.getByTestId('tab-suite').click()
  await page.getByRole('button', { name: '缺程序的声明 · missing-lsp', exact: true }).click()
  await expect(page.getByLabel('LSP 声明（JSON）', { exact: true })).toHaveValue(/never-installed-language-server\.exe/)
  await expect(page.getByText('LSP 声明不表示已启用或正在运行。OMP 按项目标记和可执行程序原生发现；缺少程序时不会启动，不会自动安装或修改 PATH。', { exact: true })).toBeVisible()
  await expect(page.getByText('保留 OMP 默认、用户及项目配置的原生优先级；Suite 不是语言服务白名单。', { exact: true })).toBeVisible()
})
