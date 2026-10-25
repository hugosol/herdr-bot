import { availableParallelism } from 'node:os'
import { defineConfig } from '@playwright/test'

/**
 * 真实浏览器验收：页面、公开 HTTP 接口与进程重启都来自交付产物。
 *
 * 测试自己启动隔离的 herdr-bot 进程，因此不使用 Playwright 的 webServer 配置。
 * 运行前需先构建前端（生成 backend/static）与后端二进制。
 */
export default defineConfig({
  testDir: './browser-tests',
  // 真实网卡验收仅由 playwright.manual.config.ts 显式收集。
  testIgnore: '**/*.manual.spec.ts',
  timeout: 90_000,
  expect: { timeout: 10_000 },
  fullyParallel: false,
  // Files are sequential by default; OMP/Pi history specs opt into isolated parallel cases.
  workers: Math.min(availableParallelism(), 16),
  reporter: [['list']],
  use: {
    channel: 'chrome',
    headless: true,
    viewport: { width: 1280, height: 900 },
  },
})
