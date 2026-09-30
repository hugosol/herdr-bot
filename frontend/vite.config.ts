import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

export default defineConfig({
  plugins: [react()],
  build: {
    // 交付运行时由 Axum 服务提供打包页面：构建产物直接落在后端静态目录，
    // 本机与局域网设备都从同一个服务取页面和操作接口。
    outDir: '../backend/static',
    emptyOutDir: true,
  },
  server: {
    // 仅开发便利：前端开发服务器把操作接口转发给本机后端。
    proxy: {
      '/api': 'http://127.0.0.1:8787',
    },
  },
})
