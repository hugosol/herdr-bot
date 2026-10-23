import { defineConfig } from '@playwright/test'
import config from './playwright.config'

// 只有显式选择此配置才授权真实网卡；Playwright worker 与后端子进程继承。
process.env.HERDR_BOT_MANUAL_NETWORK = '1'

export default defineConfig({
  ...config,
  testMatch: '**/lanAccess.manual.spec.ts',
  testIgnore: [],
})
