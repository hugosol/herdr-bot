import { render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { act } from 'react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import App from '../src/App'
import { createSseSessionFeed } from '../src/sseSessionFeed'
import { FakeSessionEventSource, resetFakeEventSources } from './fakeSessionEventSource'

/** wire 形状的会话视图。 */
function wireSession(sessionId: string, sessionName: string, overrides: Record<string, unknown> = {}) {
  return {
    sessionId,
    sessionName,
    cwd: `E:\\Developer\\myhub\\${sessionId}`,
    model: 'claude-sonnet-4-5',
    online: true,
    state: 'working',
    ...overrides,
  }
}

beforeEach(() => {
  resetFakeEventSources()
})

afterEach(() => {
  vi.unstubAllGlobals()
})

describe('主界面接线（真实 SSE 数据源）', () => {
  it('打开即实况：帧驱动会话列表收敛，断线保留快照并标过期，重连后自动对齐', async () => {
    // 组件不得直接依赖全局 EventSource：真用它就当场抛错。真实接线走注入的工厂，默认工厂只在
    // `main.tsx` 的运行时里生效。
    vi.stubGlobal(
      'EventSource',
      class {
        constructor() {
          throw new Error('组件不得直接使用全局 EventSource')
        }
      },
    )

    const feed = createSseSessionFeed({
      eventSourceFactory: (url) => new FakeSessionEventSource(url),
    })
    const user = userEvent.setup()
    const { unmount } = render(<App sessionFeed={feed} />)
    await user.click(screen.getByTestId('tab-sessions'))

    const source = FakeSessionEventSource.instances.at(-1)!
    expect(source.url).toBe('/api/sessions/events')
    // 还没有帧时是引导空态。
    expect(screen.getByTestId('sessions-empty')).toBeInTheDocument()

    act(() => {
      source.emit('snapshot', { sessions: [wireSession('a', '会话甲'), wireSession('b', '会话乙')] })
    })
    expect(screen.queryByTestId('sessions-empty')).not.toBeInTheDocument()
    expect(screen.getAllByTestId('session-card')).toHaveLength(2)
    expect(screen.queryByTestId('connection-banner')).not.toBeInTheDocument()

    // 增量把「会话甲」推到等待用户输入：等待组置顶，页面从未被刷新；tab 不因此出现任何提示。
    act(() => {
      source.emit('upsert', { session: wireSession('a', '会话甲', { state: 'awaiting-input' }) })
    })
    expect(screen.getByTestId('waiting-group')).toBeInTheDocument()
    expect(screen.getByTestId('tab-sessions').children).toHaveLength(0)

    // 断线：保留最后一帧（不出现空列表闪烁），并在会话列表内标注可能过期。
    act(() => source.fail())
    expect(screen.getByTestId('connection-banner')).toBeInTheDocument()
    expect(screen.getAllByTestId('session-card')).toHaveLength(2)
    expect(
      within(screen.getByTestId('waiting-group')).getByTestId('session-name'),
    ).toHaveTextContent('会话甲')

    // 浏览器自行重连：首帧快照即收敛——注销的「会话乙」消失、新会话出现，降级标记收回。
    act(() => {
      source.open()
      source.emit('snapshot', {
        sessions: [wireSession('a', '会话甲', { state: 'awaiting-input' }), wireSession('c', '会话丙')],
      })
    })
    expect(screen.queryByTestId('connection-banner')).not.toBeInTheDocument()
    expect(screen.getAllByTestId('session-name').map((name) => name.textContent)).toEqual(['会话甲', '会话丙'])

    // 卸载页面即断开长连接：不留后台连接。
    unmount()
    expect(source.closed).toBe(true)
  })
})
