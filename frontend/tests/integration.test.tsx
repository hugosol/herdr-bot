import { act, render, screen, waitFor, within } from '@testing-library/react'
import userEvent, { type UserEvent } from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import App from '../src/App'
import type { BrowseEntry } from '../src/browseApi'
import { treeResponse } from './treeResponse'
import type {
  ConnectionStatus,
  SessionFeed,
  SessionFeedSnapshot,
  SessionView,
} from '../src/sessionFeed'
import type { ShutdownView } from '../src/shutdownView'

const WORKSPACE = 'C:\\work\\project'
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

/** 一个可变的后端事实：用例在页面打开期间改变它，模拟节流窗口内的外部变化。 */
type BackendState = {
  entries: BrowseEntry[]
  branch: string
  changes: Array<{ path: string; kind: string }>
}

function workspaceHandler(state: BackendState) {
  return (url: URL): Response | undefined => {
    if (url.pathname === '/api/workspaces') {
      return jsonResponse(200, { workspaces: [{ path: WORKSPACE }] })
    }
    if (url.pathname === '/api/workspaces/overview') {
      return jsonResponse(200, {
        workspaces: [
          {
            path: WORKSPACE,
            repository: true,
            conflict: false,
            pendingCount: state.changes.length,
          },
        ],
      })
    }
    if (url.pathname === '/api/workspaces/git-status') {
      return jsonResponse(200, {
        workspace: WORKSPACE,
        repository: true,
        branch: state.branch,
        detached: false,
        changes: state.changes,
        conflict: false,
        pendingCount: state.changes.length,
        reviewToken: `token:${state.branch}:${state.changes.length}`,
      })
    }
    if (url.pathname === '/api/workspaces/tree') {
      return jsonResponse(200, treeResponse(WORKSPACE, { '': state.entries }))
    }
    if (url.pathname === '/api/workspaces/entries') {
      return jsonResponse(200, {
        workspace: WORKSPACE,
        path: url.searchParams.get('path') ?? '',
        entries: state.entries,
      })
    }
    if (url.pathname === '/api/workspaces/file') {
      const path = url.searchParams.get('path') ?? ''
      return jsonResponse(200, {
        workspace: WORKSPACE,
        path,
        name: path,
        size: 4,
        preview: { kind: 'text', content: 'text' },
      })
    }
    return undefined
  }
}

async function openWorkspace(user: UserEvent) {
  await user.click(screen.getByTestId('tab-files'))
  await screen.findByTestId('workspace-list')
  await user.click(screen.getByTestId('open-workspace'))
  await screen.findByTestId('workspace-browser')
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
      if (url.pathname === '/api/workspaces' && method === 'GET') {
        return jsonResponse(200, { workspaces: [] })
      }
      if (url.pathname === '/api/workspaces/overview' && method === 'GET') {
        return jsonResponse(200, { workspaces: [] })
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
    await screen.findByTestId('workspace-empty')
    expect(screen.getByTestId('shutdown-countdown')).toBeVisible()
    expect(screen.getByTestId('shutdown-takeover')).toBeVisible()

    await user.click(screen.getByTestId('tab-control'))
    expect(screen.getByTestId('shutdown-countdown')).toBeVisible()
    expect(screen.getByTestId('shutdown-takeover')).toBeVisible()

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
      if (url.pathname === '/api/workspaces' && method === 'GET') {
        return jsonResponse(200, { workspaces: [{ path: WORKSPACE }] })
      }
      if (url.pathname === '/api/workspaces/overview' && method === 'GET') {
        return jsonResponse(200, {
          workspaces: [{ path: WORKSPACE, repository: false, conflict: false }],
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
    await screen.findByTestId('workspace-list')
    await user.click(screen.getByTestId('tab-control'))
    await user.click(screen.getByTestId('start-button'))
    await waitFor(() =>
      expect(screen.getByTestId('operation-result')).toHaveTextContent(
        '启动脚本调用已结束：脚本执行完成（退出码 0）。',
      ),
    )
    expect(calls.some((call) => call.url.pathname === '/api/operations/start')).toBe(true)
  })

  it('节流窗口内回到页面只更新分支，手动刷新才读取新目录树', async () => {
    const state: BackendState = {
      entries: [{ name: 'README', path: 'README', kind: 'file', size: 16 }],
      branch: 'main',
      changes: [],
    }
    mockFetch(workspaceHandler(state))
    const user = userEvent.setup()
    render(<App />)

    // 进入 workspace：读取一次完整目录树与当前分支。
    await openWorkspace(user)
    await screen.findByTestId('entry:README')
    expect(screen.getByTestId('browser-branch')).toHaveTextContent('分支：main')

    // 外部改变文件与分支后从会话列表返回：只更新 Git 状态，目录树保持进入时的快照。
    state.entries = [{ name: 'NEXT', path: 'NEXT', kind: 'file', size: 4 }]
    state.branch = 'feature'
    await user.click(screen.getByTestId('tab-sessions'))
    await user.click(screen.getByTestId('tab-files'))
    await waitFor(() =>
      expect(screen.getByTestId('browser-branch')).toHaveTextContent('分支：feature'),
    )
    expect(screen.getByTestId('entry:README')).toBeVisible()
    expect(screen.queryByTestId('entry:NEXT')).toBeNull()

    // 只有手动刷新会重新读取目录树，并同时取得新的 Git 状态。
    state.entries = [{ name: 'THIRD', path: 'THIRD', kind: 'file', size: 5 }]
    state.branch = 'feature-2'
    await user.click(screen.getByTestId('browser-refresh'))
    await screen.findByTestId('entry:THIRD')
    expect(screen.queryByTestId('entry:README')).toBeNull()
    expect(screen.queryByTestId('entry:NEXT')).toBeNull()
    expect(screen.getByTestId('browser-branch')).toHaveTextContent('分支：feature-2')
  })

  it('文件列表与浏览期间读取失败不会被伪装成空目录或干净仓库', async () => {
    mockFetch((url) => {
      if (url.pathname === '/api/workspaces') {
        return jsonResponse(200, { workspaces: [{ path: WORKSPACE }] })
      }
      if (url.pathname === '/api/workspaces/overview') {
        return jsonResponse(200, {
          workspaces: [{ path: WORKSPACE, repository: true, conflict: false, pendingCount: 1 }],
        })
      }
      if (url.pathname === '/api/workspaces/git-status') {
        return jsonResponse(500, { code: 'read-failed', message: 'Git 元数据不可读。' })
      }
      if (url.pathname === '/api/workspaces/tree') {
        return jsonResponse(500, { code: 'read-failed', message: '目录不可读。' })
      }
      return jsonResponse(404, { code: 'unknown-endpoint', message: '未知请求' })
    })
    const user = userEvent.setup()
    render(<App />)

    await user.click(screen.getByTestId('tab-files'))
    await screen.findByTestId('workspace-list')
    await user.click(screen.getByTestId('open-workspace'))
    const browser = await screen.findByTestId('workspace-browser')
    expect(await within(browser).findByTestId('directory-error')).toHaveTextContent('目录不可读。')
    expect(within(browser).getByTestId('git-status-error')).toBeVisible()
    expect(within(browser).queryByTestId('changes-empty')).toBeNull()
  })
})
