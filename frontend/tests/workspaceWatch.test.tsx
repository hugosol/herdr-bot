import { act, render, screen, waitFor } from '@testing-library/react'
import userEvent, { type UserEvent } from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import App from '../src/App'
import type { BrowseEntry } from '../src/browseApi'
import { createWorkspaceWatchFeed } from '../src/workspaceWatch'
import { FakeWorkspaceEventSource, resetFakeWorkspaceEventSources } from './fakeWorkspaceEventSource'

const WORKSPACE = 'C:\\work\\project'

function jsonResponse(status: number, body: unknown): Response {
  return {
    ok: status >= 200 && status < 300,
    status,
    json: async () => body,
  } as unknown as Response
}

const README: BrowseEntry = { name: 'README', path: 'README', kind: 'file', size: 16 }

type Fixture = {
  entries: BrowseEntry[]
  pendingCount: number
  changes: Array<{ path: string; kind: string }>
}

function standardHandler(fixture: Fixture) {
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
            pendingCount: fixture.pendingCount,
          },
        ],
      })
    }
    if (url.pathname === '/api/workspaces/git-status') {
      return jsonResponse(200, {
        workspace: WORKSPACE,
        repository: true,
        branch: 'main',
        detached: false,
        changes: fixture.changes,
        conflict: false,
        pendingCount: fixture.changes.length,
      })
    }
    if (url.pathname === '/api/workspaces/entries') {
      return jsonResponse(200, {
        workspace: WORKSPACE,
        path: url.searchParams.get('path') ?? '',
        entries: fixture.entries,
      })
    }
    if (url.pathname === '/api/workspaces/file') {
      const path = url.searchParams.get('path') ?? ''
      return jsonResponse(200, {
        workspace: WORKSPACE,
        path,
        name: path,
        size: 16,
        preview: { kind: 'text', content: 'extensionless text' },
      })
    }
    return undefined
  }
}

function mockFetch(handler: (url: URL) => Response | undefined): URL[] {
  const calls: URL[] = []
  vi.stubGlobal(
    'fetch',
    vi.fn(async (input: RequestInfo | URL) => {
      const url = new URL(String(input), 'http://localhost')
      calls.push(url)
      return (
        handler(url) ??
        jsonResponse(404, { code: 'unknown-endpoint', message: '未知请求' })
      )
    }),
  )
  return calls
}

function watchFeed() {
  const feed = createWorkspaceWatchFeed({
    eventSourceFactory: (url) => new FakeWorkspaceEventSource(url),
  })
  return feed
}

function liveSnapshot(payload: Record<string, unknown> = {}) {
  return {
    intervalSeconds: 10,
    workspaces: [{ workspace: WORKSPACE, health: 'live', generation: 0 }],
    ...payload,
  }
}

/** 订阅者收到帧会产生 React 状态更新；用例统一在 act 内送帧。 */
function emit(source: FakeWorkspaceEventSource, type: string, payload: unknown) {
  act(() => {
    source.emit(type, payload)
  })
}

async function openWorkspace(user: UserEvent) {
  await user.click(screen.getByTestId('tab-files'))
  await waitFor(() => expect(FakeWorkspaceEventSource.instances.length).toBeGreaterThan(0))
  const source = FakeWorkspaceEventSource.instances.at(-1)!
  emit(source, 'snapshot', liveSnapshot())
  await screen.findByTestId('workspace-list')
  await user.click(screen.getByTestId('open-workspace'))
  await screen.findByTestId('workspace-browser')
  await screen.findByTestId('entry:README')
  return source
}

beforeEach(() => {
  resetFakeWorkspaceEventSources()
  Object.defineProperty(window, 'innerWidth', { configurable: true, value: 1024 })
})

afterEach(() => {
  vi.unstubAllGlobals()
})

describe('workspace 自动刷新与监听健康', () => {
  it('真实文件事件通知到达后自动重新读取目录，无需点击刷新', async () => {
    const user = userEvent.setup()
    const fixture: Fixture = {
      entries: [README],
      pendingCount: 0,
      changes: [],
    }
    const calls = mockFetch(standardHandler(fixture))
    const feed = watchFeed()
    render(<App workspaceWatchFeed={feed} />)
    const source = await openWorkspace(user)

    expect(screen.queryByTestId('entry:new.txt')).toBeNull()
    fixture.entries = [
      README,
      { name: 'new.txt', path: 'new.txt', kind: 'file', size: 7 },
    ]
    emit(source, 'refresh', { workspace: WORKSPACE, at: 100, generation: 1 })

    expect(await screen.findByTestId('entry:new.txt')).toBeVisible()
    const entryReads = calls.filter((url) => url.pathname === '/api/workspaces/entries')
    expect(entryReads.length).toBeGreaterThan(1)
  })

  it('workspace 列表的待提交角标随合并后的刷新通知更新', async () => {
    const user = userEvent.setup()
    const fixture: Fixture = {
      entries: [README],
      pendingCount: 0,
      changes: [],
    }
    mockFetch(standardHandler(fixture))
    const feed = watchFeed()
    render(<App workspaceWatchFeed={feed} />)

    await user.click(screen.getByTestId('tab-files'))
    await waitFor(() => expect(FakeWorkspaceEventSource.instances.length).toBeGreaterThan(0))
    const source = FakeWorkspaceEventSource.instances.at(-1)!
    emit(source, 'snapshot', liveSnapshot())
    await screen.findByTestId('workspace-list')
    expect(screen.queryByTestId('workspace-badge')).toBeNull()

    fixture.pendingCount = 1
    emit(source, 'refresh', { workspace: WORKSPACE, at: 100, generation: 1 })
    await waitFor(() =>
      expect(screen.getByTestId('workspace-badge')).toHaveTextContent('1 项待提交'),
    )
  })

  it('监听失效明确显示错误与过期提示，不自动宣称恢复；手动刷新仍取得当前事实', async () => {
    const user = userEvent.setup()
    const fixture: Fixture = {
      entries: [README],
      pendingCount: 0,
      changes: [],
    }
    mockFetch(standardHandler(fixture))
    const feed = watchFeed()
    render(<App workspaceWatchFeed={feed} />)
    const source = await openWorkspace(user)

    emit(source, 'health', {
      workspace: WORKSPACE,
      health: 'error',
      message: 'workspace 根目录已被替换，自动更新已停止。',
      generation: 0,
    })
    const banner = await screen.findByTestId('watch-error')
    expect(banner).toHaveTextContent('自动更新已停止')
    expect(banner).toHaveTextContent('可能已过期')
    expect(banner).toHaveTextContent('刷新')

    // 旧监听已经失效：即使再收到一个迟到事件，也不能把错误状态洗成正常。
    emit(source, 'refresh', { workspace: WORKSPACE, at: 200, generation: 1 })
    expect(screen.getByTestId('watch-error')).toBeVisible()

    fixture.entries = [
      README,
      { name: 'current.txt', path: 'current.txt', kind: 'file', size: 8 },
    ]
    await user.click(screen.getByTestId('browser-refresh'))
    expect(await screen.findByTestId('entry:current.txt')).toBeVisible()
    expect(screen.getByTestId('watch-error')).toBeVisible()
  })

  it('连接中断显示可能过期，只有完整 snapshot 才恢复自动更新状态', async () => {
    const user = userEvent.setup()
    const fixture: Fixture = {
      entries: [README],
      pendingCount: 0,
      changes: [],
    }
    mockFetch(standardHandler(fixture))
    const feed = watchFeed()
    render(<App workspaceWatchFeed={feed} />)
    const source = await openWorkspace(user)

    emit(source, 'error', {})
    expect(await screen.findByTestId('watch-stale')).toHaveTextContent('可能已过期')

    // 增量帧不是恢复证据。
    emit(source, 'refresh', { workspace: WORKSPACE, at: 300, generation: 1 })
    expect(screen.getByTestId('watch-stale')).toBeVisible()

    emit(source, 'snapshot', liveSnapshot())
    await waitFor(() => expect(screen.queryByTestId('watch-stale')).toBeNull())
  })

  it('普通目录的自动刷新同样生效，不依赖 Git 状态', async () => {
    const user = userEvent.setup()
    const fixture: Fixture = {
      entries: [README],
      pendingCount: 0,
      changes: [],
    }
    mockFetch((url) => {
      if (url.pathname === '/api/workspaces/git-status') {
        return jsonResponse(200, {
          workspace: WORKSPACE,
          repository: false,
          branch: null,
          detached: false,
          changes: [],
          conflict: false,
          pendingCount: 0,
        })
      }
      return standardHandler(fixture)(url)
    })
    const feed = watchFeed()
    render(<App workspaceWatchFeed={feed} />)
    const source = await openWorkspace(user)

    fixture.entries = [
      README,
      { name: 'plain.txt', path: 'plain.txt', kind: 'file', size: 6 },
    ]
    emit(source, 'refresh', { workspace: WORKSPACE, at: 400, generation: 1 })
    expect(await screen.findByTestId('entry:plain.txt')).toBeVisible()
  })
})
