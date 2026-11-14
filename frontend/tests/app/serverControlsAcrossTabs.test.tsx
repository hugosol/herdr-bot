import { act, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import App from '../../src/app/App'
import type {
  ConnectionStatus,
  SessionFeed,
  SessionFeedSnapshot,
  SessionView,
} from '../../src/sessions/sessionFeed'
import type { ShutdownView } from '../../src/server/shutdown/shutdownView'

const ZONE = 'C:\\work\\project'
const SERVER_TIME = 1_700_000_000_000

function jsonResponse(status: number, body: unknown): Response {
  return {
    ok: status >= 200 && status < 300,
    status,
    json: async () => body,
  } as unknown as Response
}

/** 服务端时钟的固定取值：用例不依赖真实时间，也不断言实现里的计算公式。 */
function countingDown(seconds: number, receivedAt: number): ShutdownView {
  return {
    phase: 'counting-down',
    arrangementId: 'shutdown-1',
    deadline: SERVER_TIME + seconds * 1000,
    serverTime: SERVER_TIME,
    monitoredSessionIds: [],
    receivedAt,
  }
}

/** 只驱动观察流的假数据源：会话与关机来自同一份快照。 */
function sessionFeed(sessions: SessionView[], shutdown: ShutdownView) {
  let snapshot: SessionFeedSnapshot = { sessions, connection: 'live', shutdown }
  const listeners = new Set<() => void>()
  const feed: SessionFeed = {
    subscribe: (onChange) => {
      listeners.add(onChange)
      return () => listeners.delete(onChange)
    },
    getSnapshot: () => snapshot,
  }
  const push = async (next: SessionFeedSnapshot, connection: ConnectionStatus = 'live') => {
    snapshot = { ...next, connection }
    await act(async () => listeners.forEach((notify) => notify()))
  }
  return { feed, push }
}

/** 记录 fetch 调用并按用例给定的答复返回；未覆盖的地址一律视为错误。 */
function mockFetch(
  reply: (url: URL, method: string) => Promise<Response> | Response | undefined,
) {
  const calls: Array<{ url: URL; method: string }> = []
  vi.stubGlobal(
    'fetch',
    vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = new URL(String(input), 'http://localhost')
      const method = init?.method ?? 'GET'
      calls.push({ url, method })
      return (
        (await reply(url, method)) ??
        jsonResponse(404, { code: 'unknown-endpoint', message: '未知请求' })
      )
    }),
  )
  return calls
}

beforeEach(() => {
  Object.defineProperty(window, 'innerWidth', { configurable: true, value: 1024 })
})

afterEach(() => {
  vi.unstubAllGlobals()
})

describe('跨流程回归：文件功能不改变既有行为', () => {
  it('全局关机提醒在三个顶层 tab 都可见，切 tab 不取消安排', async () => {
    const sessions: SessionView[] = [
      {
        sessionId: 'session-one-id',
        sessionName: 'session-one',
        cwd: 'C:\\work',
        online: true,
        state: 'working',
      },
    ]
    const { feed } = sessionFeed(sessions, countingDown(30, Date.now()))
    const calls = mockFetch((url, method) => {
      if (url.pathname === '/api/zones' && method === 'GET') {
        return jsonResponse(200, { zones: [] })
      }
      if (url.pathname === '/api/zones/overview' && method === 'GET') {
        return jsonResponse(200, { zones: [] })
      }
      return jsonResponse(404, { code: 'unknown-endpoint', message: '未知请求' })
    })
    const user = userEvent.setup()
    render(<App sessionFeed={feed} />)

    // 默认会话列表：会话读取与全局关机提醒同时可见。
    expect(screen.getByText('session-one')).toBeVisible()
    expect(screen.getByTestId('shutdown-countdown')).toBeVisible()
    expect(screen.getByTestId('shutdown-takeover')).toBeVisible()

    await user.click(screen.getByTestId('tab-files'))
    await screen.findByTestId('zone-empty')
    expect(screen.getByTestId('shutdown-countdown')).toBeVisible()
    expect(screen.getByTestId('shutdown-takeover')).toBeVisible()

    await user.click(screen.getByTestId('server-button'))
    expect(screen.getByTestId('shutdown-countdown')).toBeVisible()
    expect(screen.getByTestId('shutdown-takeover')).toBeVisible()

    await user.click(screen.getByTestId('server-back'))
    await user.click(screen.getByTestId('tab-sessions'))
    expect(screen.getByText('session-one')).toBeVisible()
    expect(screen.getByTestId('shutdown-countdown')).toBeVisible()

    // 切 tab 与读取文件列表都没有发送关机命令：提醒只是展示同一份后端事实。
    expect(
      calls.some((call) => call.url.pathname.startsWith('/api/shutdown')),
    ).toBe(false)
  })

  it('文件功能存在时固定操作仍按原有方式执行', async () => {
    const { feed } = sessionFeed([], {
      phase: 'none',
      serverTime: SERVER_TIME,
      monitoredSessionIds: [],
      receivedAt: Date.now(),
    })
    const calls = mockFetch((url, method) => {
      if (url.pathname === '/api/zones' && method === 'GET') {
        return jsonResponse(200, { zones: [{ id: 'test/zone', path: ZONE }] })
      }
      if (url.pathname === '/api/zones/overview' && method === 'GET') {
        return jsonResponse(200, {
          zones: [{ path: ZONE, repository: false, conflict: false }],
        })
      }
      if (url.pathname === '/api/operations/start' && method === 'POST') {
        return jsonResponse(200, {
          operation: 'start',
          ended: true,
          started: true,
          exitCode: 0,
          outcome: 'completed',
        })
      }
      return jsonResponse(404, { code: 'unknown-endpoint', message: '未知请求' })
    })
    const user = userEvent.setup()
    render(<App sessionFeed={feed} />)

    await user.click(screen.getByTestId('tab-files'))
    await screen.findByTestId('zone-list')
    await user.click(screen.getByTestId('server-button'))
    await user.click(screen.getByTestId('start-button'))
    await waitFor(() =>
      expect(screen.getByTestId('operation-result')).toHaveTextContent(
        '启动脚本调用已结束：脚本执行完成（退出码 0）。',
      ),
    )
    expect(calls.some((call) => call.url.pathname === '/api/operations/start')).toBe(true)
  })

})
