import react from '@vitejs/plugin-react'
import { defineConfig } from 'vitest/config'

// 前端交互测试基座：jsdom + React Testing Library + jest-dom 匹配器。
// 与 `vite.config.ts` 分开，构建配置不掺入测试环境；两者都只用 React 插件。
export default defineConfig({
  plugins: [react()],
  test: {
    environment: 'jsdom',
    globals: true,
    setupFiles: ['./tests/setup.ts'],
    include: ['tests/**/*.test.{ts,tsx}'],
    css: false,
  },
})
