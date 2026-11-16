import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { act } from 'react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import App from '../../src/app/App'
import { createSseSessionFeed } from '../../src/sessions/sseSessionFeed'
import { FakeSessionEventSource, resetFakeEventSources } from '../support/fakeSessionEventSource'

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

describe('会话观察流中的自动关机', () => {
  it('停留在会话列表时，自动关机提醒立即可见且可取消，无需切换 tab', async () => {
    // 契约：自动安排使用全局提醒。可能的违规是提醒位于隐藏 tab 内：DOM 存在却不可见。
    // 保留 App、真实 SSE 映射及关机命令客户端，只替换外部事件和 HTTP 传输。
    const serverTime = Date.now()
    const selected = { phase: 'none', monitoredSessionIds: ['a'], serverTime }
    const cancelled = { phase: 'cancelled', monitoredSessionIds: [], arrangementId: 'automatic-a', serverTime }
    const fetchDouble = vi.fn(async (input: RequestInfo | URL) => {
      const url = String(input)
      if (url === '/api/shutdown/monitor') {
        return new Response(JSON.stringify({ outcome: 'selected', shutdown: selected }))
      }
      if (url === '/api/shutdown/cancel') {
        return new Response(JSON.stringify({ outcome: 'cancelled', shutdown: cancelled }))
      }
      throw new Error('自动触发不应由浏览器提交开始命令：' + url)
    })
    vi.stubGlobal('fetch', fetchDouble)
    const feed = createSseSessionFeed({ eventSourceFactory: (url) => new FakeSessionEventSource(url) })
    render(<App sessionFeed={feed} />)
    const user = userEvent.setup()
    await user.click(screen.getByTestId('tab-sessions'))
    const source = FakeSessionEventSource.instances.at(-1)!
    act(() => source.emit('snapshot', {
      sessions: [wireSession('a', '会话甲')],
      shutdown: { phase: 'none', monitoredSessionIds: [], serverTime },
    }))
    await user.click(screen.getByTestId('monitor-button'))
    act(() => {
      source.emit('upsert', { session: wireSession('a', '会话甲', { state: 'awaiting-input' }) })
      source.emit('shutdown', { shutdown: {
        phase: 'counting-down', monitoredSessionIds: [], arrangementId: 'automatic-a',
        deadline: serverTime + 30_000, serverTime,
      } })
    })

    expect(screen.getByTestId('tab-sessions')).toHaveAttribute('aria-selected', 'true')
    expect(screen.getByTestId('shutdown-countdown')).toBeVisible()
    expect(screen.getByRole('dialog', { name: '关机' })).toBeVisible()
    expect(screen.getByRole('button', { name: '取消关机' })).toBeVisible()
    await user.click(screen.getByRole('button', { name: '取消关机' }))
    // 命令被服务端确认：只反馈这次取消的归宿，安排仍按观察流显示。
    expect(screen.getByTestId('shutdown-accepted')).toHaveTextContent('已确认这次取消')
    expect(screen.getByTestId('shutdown-countdown')).toBeVisible()
    // 观察流给出「取消已生效」：这才是显示「取消成功」的事实来源。
    act(() => {
      source.emit('shutdown', { shutdown: cancelled })
    })
    expect(screen.getByTestId('shutdown-result')).toBeVisible()
    expect(screen.getByTestId('shutdown-result')).toHaveTextContent('取消成功')
    await user.click(screen.getByTestId('shutdown-result-back'))
    expect(screen.queryByRole('dialog', { name: '关机' })).not.toBeInTheDocument()
    expect(screen.getByTestId('panel-sessions')).toBeVisible()
    expect(fetchDouble.mock.calls.map(([url]) => url)).toEqual([
      '/api/shutdown/monitor', '/api/shutdown/cancel',
    ])
  })

})
