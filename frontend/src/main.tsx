import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'

import App from './App'
import { createSseSessionFeed } from './sseSessionFeed'
import { createWorkspaceWatchFeed } from './workspaceWatch'
import './styles.css'

const root = document.getElementById('root')
if (!root) {
  throw new Error('缺少 #root 挂载点')
}

// 页面唯一的数据源：与同源（开发时经 Vite 代理）的 `/api/sessions/events` 建立只读 SSE 长连接。
// 归一化在 `createSseSessionFeed` 里，组件只管消费注入的契约。
const sessionFeed = createSseSessionFeed()
// 文件列表的自动更新也走同源只读长连接；监听只观察本地状态，不触发远端 fetch。
const workspaceWatchFeed = createWorkspaceWatchFeed()

createRoot(root).render(
  <StrictMode>
    <App sessionFeed={sessionFeed} workspaceWatchFeed={workspaceWatchFeed} />
  </StrictMode>,
)
