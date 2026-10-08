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

/** 一帧 `snapshot` 的载荷：会话快照 + **同一帧**里的权威关机状态（真实后端两半都带）。 */
function snapshot(sessions: unknown[]) {
  return {
    sessions,
    shutdown: { phase: 'none', monitoredSessionIds: [] as string[], serverTime: Date.now() },
  }
}

beforeEach(() => {
  resetFakeEventSources()
})

afterEach(() => {
  vi.unstubAllGlobals()
})

describe('主界面接线（真实 SSE 数据源）', () => {
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
      source.emit('snapshot', snapshot([wireSession('a', '会话甲'), wireSession('b', '会话乙')]))
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
      source.emit(
        'snapshot',
        snapshot([wireSession('a', '会话甲', { state: 'awaiting-input' }), wireSession('c', '会话丙')]),
      )
    })
    expect(screen.queryByTestId('connection-banner')).not.toBeInTheDocument()
    expect(screen.getAllByTestId('session-name').map((name) => name.textContent)).toEqual(['会话甲', '会话丙'])

    // 卸载页面即断开长连接：不留后台连接。
    unmount()
    expect(source.closed).toBe(true)
  })
})
