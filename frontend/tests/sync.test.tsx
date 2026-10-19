import { render, screen, waitFor, within } from '@testing-library/react'
import userEvent, { type UserEvent } from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import App from '../src/App'
import type { BrowseEntry } from '../src/browseApi'
import { treeResponse } from './treeResponse'
import type {
  GitChange,
  GitStatus,
  SyncAccepted,
  UpstreamRef,
  WorkspaceSummary,
} from '../src/gitApi'

const WORKSPACE = 'C:\\work\\repository'

const UPSTREAM: UpstreamRef = {
  remote: 'upstream',
  branch: 'feature',
  short: 'upstream/feature',
}

function jsonResponse(status: number, body: unknown): Response {
  return {
    ok: status >= 200 && status < 300,
    status,
    json: async () => body,
  } as unknown as Response
}

type Handler = (
  url: URL,
  init: RequestInit | undefined,
) => Response | Promise<Response> | undefined

function mockFetch(handler: Handler): {
  urls: URL[]
  commits: string[]
  checks: string[]
  syncs: string[]
} {
  const urls: URL[] = []
  const commits: string[] = []
  const checks: string[] = []
  const syncs: string[] = []
  vi.stubGlobal(
    'fetch',
    vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = new URL(String(input), 'http://localhost')
      urls.push(url)
      if (url.pathname === '/api/workspaces/commit') {
        commits.push(String(init?.body ?? ''))
      }
      if (url.pathname === '/api/workspaces/check-remote') {
        checks.push(String(init?.body ?? ''))
      }
      if (url.pathname === '/api/workspaces/sync') {
        syncs.push(String(init?.body ?? ''))
      }
      return (
        (await handler(url, init)) ??
        jsonResponse(404, { code: 'unknown-endpoint', message: '未知请求' })
      )
    }),
  )
  return { urls, commits, checks, syncs }
}

const DIRECTORIES: Record<string, BrowseEntry[]> = {
  '': [{ name: 'tracked.txt', path: 'tracked.txt', kind: 'file', size: 5 }],
}

const CHANGES: GitChange[] = [
  { path: 'tracked.txt', kind: 'modified' },
  { path: 'added.txt', kind: 'added' },
]

function gitStatus(overrides: Partial<GitStatus> = {}): GitStatus {
  return {
    workspace: WORKSPACE,
    repository: true,
    branch: 'main',
    detached: false,
    changes: [],
    conflict: false,
    pendingCount: 0,
    reviewToken: 'token-1',
    blockedReason: null,
    sync: {
      upstream: UPSTREAM,
      ahead: 0,
      behind: 0,
      remoteCheckedAt: null,
    },
    ...overrides,
  }
}

function syncFacts(
  ahead: number | null,
  behind: number | null,
  remoteCheckedAt: number | null,
  upstream: UpstreamRef | null = UPSTREAM,
): GitStatus['sync'] {
  return { upstream, ahead, behind, remoteCheckedAt }
}

function accepted(overrides: Partial<SyncAccepted> = {}): SyncAccepted {
  return {
    workspace: WORKSPACE,
    outcome: 'synced',
    branch: 'main',
    upstream: UPSTREAM,
    ahead: 0,
    behind: 0,
    remoteCheckedAt: 1720000005000,
    pulled: 1,
    pushed: 0,
    steps: [
      { step: 'fetch', outcome: 'completed' },
      { step: 'pull', outcome: 'completed' },
      { step: 'push', outcome: 'skipped' },
    ],
    ...overrides,
  }
}

function overview(status: GitStatus): { workspaces: WorkspaceSummary[] } {
  return {
    workspaces: [
      {
        path: WORKSPACE,
        repository: status.repository,
        branch: status.branch,
        pendingCount: status.repository ? status.pendingCount : undefined,
        conflict: status.conflict,
      },
    ],
  }
}

function standardHandler(
  status: () => GitStatus,
  overrides: {
    check?: (body: string) => Response | Promise<Response>
    sync?: (body: string) => Response | Promise<Response>
    commit?: (body: string) => Response | Promise<Response>
  } = {},
): Handler {
  return (url, init) => {
    if (url.pathname === '/api/workspaces') {
      return jsonResponse(200, { workspaces: [{ path: WORKSPACE }] })
    }
    if (url.pathname === '/api/workspaces/overview') {
      return jsonResponse(200, overview(status()))
    }
    if (url.pathname === '/api/workspaces/git-status') {
      return jsonResponse(200, status())
    }
    if (url.pathname === '/api/workspaces/tree') {
      return jsonResponse(200, treeResponse(WORKSPACE, DIRECTORIES))
    }
    if (url.pathname === '/api/workspaces/entries') {
      const path = url.searchParams.get('path') ?? ''
      return jsonResponse(200, {
        workspace: WORKSPACE,
        path,
        entries: DIRECTORIES[path] ?? [],
      })
    }
    if (url.pathname === '/api/workspaces/file') {
      const path = url.searchParams.get('path') ?? ''
      return jsonResponse(200, {
        workspace: WORKSPACE,
        path,
        name: path,
        size: 4,
        preview: { kind: 'text', content: `content of ${path}` },
      })
    }
    if (url.pathname === '/api/workspaces/diff') {
      return jsonResponse(404, { code: 'path-not-found', message: '没有该变更' })
    }
    if (url.pathname === '/api/workspaces/check-remote') {
      return overrides.check?.(String(init?.body ?? '')) ?? jsonResponse(200, status())
    }
    if (url.pathname === '/api/workspaces/sync') {
      return overrides.sync?.(String(init?.body ?? '')) ?? jsonResponse(200, accepted())
    }
    if (url.pathname === '/api/workspaces/commit') {
      return overrides.commit?.(String(init?.body ?? '')) ?? jsonResponse(200, {})
    }
    return undefined
  }
}

async function openChanges(user: UserEvent) {
  await user.click(screen.getByTestId('tab-files'))
  await screen.findByTestId('workspace-list')
  await user.click(screen.getByTestId('open-workspace'))
  await screen.findByTestId('workspace-browser')
  await screen.findByTestId('browser-branch')
  await user.click(screen.getByTestId('browser-view-changes'))
  await screen.findByTestId('commit-bar')
}

async function openReadySync(user: UserEvent) {
  await openChanges(user)
  await waitFor(() =>
    expect(screen.getByTestId('sync-button')).toBeEnabled(),
  )
}

async function confirmSync(user: UserEvent) {
  await user.click(screen.getByTestId('sync-button'))
  const dialog = await screen.findByTestId('sync-confirm-dialog')
  await user.click(within(dialog).getByTestId('sync-confirm'))
}

beforeEach(() => {
  Object.defineProperty(window, 'innerWidth', { configurable: true, value: 1024 })
})

afterEach(() => {
  vi.unstubAllGlobals()
})

describe('按现有 upstream 安全 Sync', () => {
  it('主按钮逐态对应真实 Git 事实：Commit、Sync、已同步、未检查、无 upstream 与阻塞', async () => {
    const user = userEvent.setup()
    let status = gitStatus({
      changes: CHANGES,
      pendingCount: CHANGES.length,
    })
    mockFetch(standardHandler(() => status))

    render(<App />)
    await openChanges(user)

    // 有普通待提交变更：Commit 是主操作；Sync 不出现。
    expect(screen.getByTestId('commit-button')).toHaveTextContent('Commit')
    expect(screen.queryByTestId('sync-button')).not.toBeInTheDocument()
    expect(screen.getByTestId('sync-state')).toHaveTextContent(
      'Sync 只对干净工作区可用',
    )

    // 干净但远端尚未检查：Sync 禁用并给出检查入口与原因。
    status = gitStatus({ sync: syncFacts(0, 0, null) })
    await user.click(screen.getByTestId('browser-refresh'))
    await waitFor(() =>
      expect(screen.getByTestId('sync-button')).toHaveAttribute(
        'data-state',
        'disabled',
      ),
    )
    expect(screen.getByTestId('sync-state')).toHaveTextContent('远端尚未检查')
    expect(screen.getByTestId('check-remote-button')).toBeEnabled()
    expect(screen.getByTestId('remote-checked-at')).toHaveTextContent('尚未检查')

    // 已知已同步：主按钮禁用为“已同步”，仍可主动检查远端。
    status = gitStatus({ sync: syncFacts(0, 0, 1720000001000) })
    await user.click(screen.getByTestId('browser-refresh'))
    await waitFor(() =>
      expect(screen.getByTestId('sync-button')).toHaveTextContent('已同步'),
    )
    expect(screen.getByTestId('sync-button')).toBeDisabled()
    expect(screen.getByTestId('check-remote-button')).toBeEnabled()
    expect(screen.getByTestId('remote-checked-at')).toHaveAttribute(
      'data-checked-at',
      '1720000001000',
    )

    // 已知 ahead/behind 非零：Sync 可用，并显示真实数量。
    status = gitStatus({ sync: syncFacts(2, 1, 1720000002000) })
    await user.click(screen.getByTestId('browser-refresh'))
    await waitFor(() => expect(screen.getByTestId('sync-button')).toBeEnabled())
    expect(screen.getByTestId('sync-state')).toHaveTextContent('待拉取 1 个提交')
    expect(screen.getByTestId('sync-state')).toHaveTextContent('待推送 2 个提交')

    // 无 upstream：显示限制，不提供 Publish Branch，也不能检查远端。
    status = gitStatus({ sync: syncFacts(null, null, null, null) })
    await user.click(screen.getByTestId('browser-refresh'))
    await waitFor(() =>
      expect(screen.getByTestId('sync-state')).toHaveTextContent('没有 upstream'),
    )
    expect(screen.getByTestId('sync-button')).toBeDisabled()
    expect(screen.getByTestId('check-remote-button')).toBeDisabled()
    expect(screen.getByTestId('sync-upstream')).toHaveTextContent('未配置')

    // detached HEAD / 冲突：与普通 Git 事实一致地阻止 Sync 并说明原因。
    status = gitStatus({
      detached: true,
      branch: null,
      sync: syncFacts(null, null, null, null),
      blockedReason: '当前处于 detached HEAD：普通 Commit 不可用。',
    })
    await user.click(screen.getByTestId('browser-refresh'))
    await waitFor(() =>
      expect(screen.getByTestId('sync-state')).toHaveTextContent('detached HEAD'),
    )
    expect(screen.getByTestId('sync-button')).toBeDisabled()

    status = gitStatus({
      conflict: true,
      sync: syncFacts(0, 0, 1720000003000),
      blockedReason: '存在未解决的冲突文件：请在外部工具解决冲突后再试。',
    })
    await user.click(screen.getByTestId('browser-refresh'))
    await waitFor(() =>
      expect(screen.getByTestId('sync-state')).toHaveTextContent('冲突'),
    )
    expect(screen.getByTestId('sync-button')).toBeDisabled()
  })

  it('主动检查远端只发一次请求，并更新最近检查时间与实际 ahead/behind', async () => {
    const user = userEvent.setup()
    let status = gitStatus({ sync: syncFacts(0, 0, null) })
    const requests = mockFetch(
      standardHandler(
        () => status,
        {
          check: () => {
            status = gitStatus({ sync: syncFacts(1, 2, 1720000010000) })
            return jsonResponse(200, status)
          },
        },
      ),
    )

    render(<App />)
    await openChanges(user)
    expect(screen.getByTestId('sync-button')).toBeDisabled()

    await user.click(screen.getByTestId('check-remote-button'))
    const feedback = await screen.findByTestId('sync-feedback')
    expect(feedback).toHaveAttribute('data-tone', 'success')
    expect(feedback).toHaveTextContent('远端已检查')
    expect(requests.checks).toHaveLength(1)
    expect(JSON.parse(requests.checks[0])).toEqual({ workspace: WORKSPACE })

    await waitFor(() => expect(screen.getByTestId('sync-button')).toBeEnabled())
    expect(screen.getByTestId('remote-checked-at')).toHaveAttribute(
      'data-checked-at',
      '1720000010000',
    )
    expect(screen.getByTestId('sync-state')).toHaveTextContent('待拉取 2 个提交')
    expect(screen.getByTestId('sync-state')).toHaveTextContent('待推送 1 个提交')
    expect(screen.getByTestId('sync-upstream')).toHaveTextContent('upstream/feature')

    // 远端独立前进后再检查：页面使用新的事实，而不是旧数量。
    status = gitStatus({ sync: syncFacts(1, 2, 1720000010000) })
    await user.click(screen.getByTestId('browser-refresh'))
    await waitFor(() => expect(screen.getByTestId('sync-button')).toBeEnabled())
    await user.click(screen.getByTestId('check-remote-button'))
    await waitFor(() =>
      expect(requests.checks).toHaveLength(2),
    )
  })

  it('Sync 只需一句确认；取消不发送任何写请求', async () => {
    const user = userEvent.setup()
    const status = gitStatus({ sync: syncFacts(0, 1, 1720000004000) })
    const requests = mockFetch(standardHandler(() => status))

    render(<App />)
    await openReadySync(user)
    await user.click(screen.getByTestId('sync-button'))

    const dialog = await screen.findByTestId('sync-confirm-dialog')
    const paragraphs = within(dialog).getAllByRole('paragraph')
    expect(paragraphs).toHaveLength(1)
    const text = dialog.textContent ?? ''
    expect(text).toContain('确认按当前分支的既有 upstream')
    expect(text).not.toContain(WORKSPACE)
    expect(text).not.toContain('main')
    expect(text).not.toContain('upstream/feature')
    expect(text).not.toContain('待拉取')
    expect(text).not.toContain('1 个提交')

    await user.click(within(dialog).getByTestId('sync-cancel'))
    expect(screen.queryByTestId('sync-confirm-dialog')).not.toBeInTheDocument()
    expect(requests.syncs).toHaveLength(0)
    expect(screen.getByTestId('sync-button')).toBeEnabled()
  })

  it('确认后只同步一次，成功后显示真实结果并刷新为已同步', async () => {
    const user = userEvent.setup()
    let status = gitStatus({ sync: syncFacts(0, 1, 1720000004000) })
    const requests = mockFetch(
      standardHandler(
        () => status,
        {
          sync: () => {
            status = gitStatus({
              branch: 'feature',
              sync: syncFacts(0, 0, 1720000005000),
            })
            return jsonResponse(200, accepted())
          },
        },
      ),
    )

    render(<App />)
    await openReadySync(user)
    await confirmSync(user)

    const feedback = await screen.findByTestId('sync-feedback')
    expect(feedback).toHaveAttribute('data-tone', 'success')
    expect(feedback).toHaveTextContent('已同步（upstream/feature）')
    expect(feedback).toHaveTextContent('拉取 1 个提交')
    expect(feedback).toHaveTextContent('推送 0 个提交')
    expect(requests.syncs).toHaveLength(1)
    expect(JSON.parse(requests.syncs[0])).toEqual({ workspace: WORKSPACE })

    await waitFor(() =>
      expect(screen.getByTestId('sync-button')).toHaveTextContent('已同步'),
    )
    expect(screen.getByTestId('sync-button')).toBeDisabled()
    expect(screen.getByTestId('remote-checked-at')).toHaveAttribute(
      'data-checked-at',
      '1720000005000',
    )
  })

  it('Sync 执行中切 tab 不取消已接受执行，返回后可见实际结果', async () => {
    const user = userEvent.setup()
    let status = gitStatus({ sync: syncFacts(0, 1, 1720000004000) })
    let resolveSync: ((response: Response) => void) | null = null
    const requests = mockFetch(
      standardHandler(
        () => status,
        {
          sync: () =>
            new Promise<Response>((resolve) => {
              resolveSync = resolve
            }),
        },
      ),
    )

    render(<App />)
    await openReadySync(user)
    await confirmSync(user)
    expect(requests.syncs).toHaveLength(1)

    await user.click(screen.getByTestId('tab-sessions'))
    await user.click(screen.getByTestId('tab-files'))
    expect(requests.syncs).toHaveLength(1)
    expect(screen.queryByTestId('sync-feedback')).not.toBeInTheDocument()

    status = gitStatus({ sync: syncFacts(0, 0, 1720000006000) })
    resolveSync!(jsonResponse(200, accepted({ remoteCheckedAt: 1720000006000 })))

    const feedback = await screen.findByTestId('sync-feedback')
    expect(feedback).toHaveAttribute('data-tone', 'success')
    expect(feedback).toHaveTextContent('已同步')
    await waitFor(() =>
      expect(screen.getByTestId('sync-button')).toHaveTextContent('已同步'),
    )
  })

  it('整页重载不重放已经发起的 Sync', async () => {
    const user = userEvent.setup()
    let pendingSync: ((response: Response) => void) | null = null
    const status = gitStatus({ sync: syncFacts(0, 1, 1720000004000) })
    const requests = mockFetch(
      standardHandler(
        () => status,
        {
          sync: () =>
            new Promise<Response>((resolve) => {
              pendingSync = resolve
            }),
        },
      ),
    )

    const first = render(<App />)
    await openReadySync(user)
    await confirmSync(user)
    expect(requests.syncs).toHaveLength(1)

    first.unmount()
    render(<App />)
    expect(screen.getByTestId('tab-sessions')).toHaveAttribute('aria-selected', 'true')
    expect(requests.syncs).toHaveLength(1)
    pendingSync!(jsonResponse(200, accepted()))
    expect(requests.syncs).toHaveLength(1)
  })

  it('响应中断显示未确认、按实际状态刷新且不自动重试', async () => {
    const user = userEvent.setup()
    let status = gitStatus({ sync: syncFacts(1, 0, 1720000004000) })
    let statusReads = 0
    const requests = mockFetch((url, init) => {
      if (url.pathname === '/api/workspaces/sync') {
        return Promise.reject(new TypeError('连接中断'))
      }
      if (url.pathname === '/api/workspaces/git-status') {
        statusReads += 1
        return jsonResponse(200, status)
      }
      return standardHandler(() => status)(url, init)
    })

    render(<App />)
    await openReadySync(user)
    await confirmSync(user)

    const feedback = await screen.findByTestId('sync-feedback')
    expect(feedback).toHaveAttribute('data-tone', 'unconfirmed')
    expect(feedback).toHaveTextContent('未确认')
    expect(feedback).toHaveTextContent('不会自动重试')
    expect(requests.syncs).toHaveLength(1)
    await waitFor(() => expect(statusReads).toBeGreaterThan(1))
    // 实际状态仍是待推送，Sync 仍是主操作；页面没有伪造成功。
    expect(screen.getByTestId('sync-button')).toBeEnabled()
  })

  it('Git 诊断按纯文本显示，超时与已有输出不被改写', async () => {
    const user = userEvent.setup()
    const status = gitStatus({ sync: syncFacts(1, 0, 1720000004000) })
    mockFetch(
      standardHandler(
        () => status,
        {
          sync: () =>
            jsonResponse(504, {
              code: 'git-timeout',
              message:
                '同步在 push 步骤失败：git push 超过本功能配置的 1 秒执行上限，已停止本次执行。取消前已有的输出：push hook started <script>window.__pwned = 1</script><img src=x onerror="window.__pwned = 2"> 已完成的步骤（fetch → pull）不会被自动撤销。',
            }),
        },
      ),
    )

    render(<App />)
    await openReadySync(user)
    await confirmSync(user)

    const feedback = await screen.findByTestId('sync-feedback')
    expect(feedback).toHaveAttribute('data-tone', 'error')
    expect(feedback).toHaveTextContent('本功能配置的 1 秒执行上限')
    expect(feedback).toHaveTextContent('push hook started')
    expect(feedback).toHaveTextContent('<script>window.__pwned = 1</script>')
    expect('__pwned' in window).toBe(false)
    expect(feedback.querySelector('script')).toBeNull()
    expect(feedback.querySelector('img')).toBeNull()
  })

  it('外部切分支后页面重新显示当前分支与 upstream', async () => {
    const user = userEvent.setup()
    let status = gitStatus({ sync: syncFacts(0, 0, 1720000004000) })
    mockFetch(standardHandler(() => status))

    render(<App />)
    await openChanges(user)
    expect(screen.getByTestId('browser-branch')).toHaveTextContent('main')
    expect(screen.getByTestId('sync-upstream')).toHaveTextContent('upstream/feature')

    status = gitStatus({
      branch: 'release',
      sync: syncFacts(null, null, null, {
        remote: 'upstream',
        branch: 'release',
        short: 'upstream/release',
      }),
    })
    await user.click(screen.getByTestId('browser-refresh'))
    await waitFor(() =>
      expect(screen.getByTestId('browser-branch')).toHaveTextContent('release'),
    )
    expect(screen.getByTestId('sync-upstream')).toHaveTextContent(
      'upstream/release',
    )
    // 新 upstream 尚未检查：旧分支的检查时间不能冒充当前分支已检查。
    expect(screen.getByTestId('sync-state')).toHaveTextContent('远端尚未检查')
    expect(screen.getByTestId('sync-button')).toBeDisabled()
  })

  it('普通目录与 Git 状态读取失败都不显示可执行的 Sync', async () => {
    const user = userEvent.setup()
    const plain = gitStatus({
      repository: false,
      branch: null,
      changes: [],
      pendingCount: 0,
      reviewToken: undefined,
      sync: undefined,
    })
    mockFetch(standardHandler(() => plain))

    const view = render(<App />)
    await user.click(screen.getByTestId('tab-files'))
    await screen.findByTestId('workspace-list')
    await user.click(screen.getByTestId('open-workspace'))
    await screen.findByTestId('workspace-browser')
    await waitFor(() =>
      expect(screen.queryByTestId('commit-bar')).not.toBeInTheDocument(),
    )
    expect(screen.queryByTestId('sync-bar')).not.toBeInTheDocument()
    expect(screen.queryByTestId('sync-button')).not.toBeInTheDocument()
    expect(screen.queryByTestId('check-remote-button')).not.toBeInTheDocument()

    // 状态读取失败：显示明确错误，不把旧状态当作可同步事实。
    view.unmount()
    mockFetch((url, init) => {
      if (url.pathname === '/api/workspaces/git-status') {
        return jsonResponse(500, {
          code: 'workspace-unavailable',
          message: '登记目录已不可访问。',
        })
      }
      return standardHandler(() => gitStatus())(url, init)
    })
    render(<App />)
    await user.click(screen.getByTestId('tab-files'))
    await screen.findByTestId('workspace-list')
    await user.click(screen.getByTestId('open-workspace'))
    await screen.findByTestId('workspace-browser')
    await waitFor(() =>
      expect(screen.getByTestId('git-status-error')).toHaveTextContent(
        '登记目录已不可访问',
      ),
    )
    expect(screen.queryByTestId('commit-bar')).not.toBeInTheDocument()
    expect(screen.queryByTestId('sync-button')).not.toBeInTheDocument()
    expect(screen.queryByTestId('check-remote-button')).not.toBeInTheDocument()
  })
})
