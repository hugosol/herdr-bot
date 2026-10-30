import { render, screen, waitFor, within } from '@testing-library/react'
import userEvent, { type UserEvent } from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import App from '../src/app/App'
import type { BrowseEntry } from '../src/zones/browser/browseApi'
import { treeResponse } from './treeResponse'
import type { ChangeDiff, DiffSide } from '../src/zones/git/diffApi'
import type { GitChange, GitStatus, ZoneSummary } from '../src/zones/git/gitApi'

const ZONE = 'C:\\work\\repository'

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

function mockFetch(handler: Handler): { urls: URL[]; commits: string[] } {
  const urls: URL[] = []
  const commits: string[] = []
  vi.stubGlobal(
    'fetch',
    vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = new URL(String(input), 'http://localhost')
      urls.push(url)
      if (url.pathname === '/api/zones/commit') {
        commits.push(String(init?.body ?? ''))
      }
      return (
        (await handler(url, init)) ??
        jsonResponse(404, { code: 'unknown-endpoint', message: '未知请求' })
      )
    }),
  )
  return { urls, commits }
}

const DIRECTORIES: Record<string, BrowseEntry[]> = {
  '': [
    { name: 'src', path: 'src', kind: 'directory' },
    { name: 'README.md', path: 'README.md', kind: 'file', size: 6 },
  ],
  src: [
    { name: 'added.txt', path: 'src/added.txt', kind: 'file', size: 4 },
    { name: 'binary.bin', path: 'src/binary.bin', kind: 'file', size: 5 },
    { name: 'modified.txt', path: 'src/modified.txt', kind: 'file', size: 5 },
  ],
}

const CHANGES: GitChange[] = [
  { path: 'src/added.txt', kind: 'added' },
  { path: 'src/binary.bin', kind: 'modified' },
  { path: 'src/gone.txt', kind: 'deleted' },
  { path: 'src/modified.txt', kind: 'modified' },
]

const TEXT = (content: string): DiffSide => ({
  kind: 'text',
  content,
  size: new TextEncoder().encode(content).length,
})

const DIFFS: Record<string, ChangeDiff> = {
  'src/added.txt': {
    zone: ZONE,
    path: 'src/added.txt',
    old: { kind: 'absent' },
    new: TEXT('brand\nnew\n'),
    complete: true,
  },
  'src/binary.bin': {
    zone: ZONE,
    path: 'src/binary.bin',
    old: TEXT('old\n'),
    new: { kind: 'binary', reason: '文件包含 NUL 字节。', size: 8 },
    complete: false,
  },
  'src/gone.txt': {
    zone: ZONE,
    path: 'src/gone.txt',
    old: TEXT('gone\n'),
    new: { kind: 'absent' },
    complete: true,
  },
  'src/modified.txt': {
    zone: ZONE,
    path: 'src/modified.txt',
    old: TEXT('old\n'),
    new: TEXT('new\n'),
    complete: true,
  },
}

function gitStatus(overrides: Partial<GitStatus> = {}): GitStatus {
  return {
    zone: ZONE,
    repository: true,
    branch: 'main',
    detached: false,
    changes: CHANGES,
    conflict: false,
    pendingCount: CHANGES.length,
    reviewToken: 'token-1',
    blockedReason: null,
    ...overrides,
  }
}

function overview(status: GitStatus): { zones: ZoneSummary[] } {
  return {
    zones: [
      {
        path: ZONE,
        repository: status.repository,
        branch: status.branch,
        pendingCount: status.repository ? status.pendingCount : undefined,
        conflict: status.conflict,
      },
    ],
  }
}

function listing(path: string) {
  return { zone: ZONE, path, entries: DIRECTORIES[path] ?? [] }
}

/**
 * 标准读取面；提交 POST 由测试自己的 handler 接管。
 * `status` 是可变引用，因此“提交后刷新”能观察到新的真实状态。
 */
function standardHandler(
  status: () => GitStatus,
  commit: (body: string) => Response | Promise<Response>,
): Handler {
  return (url, init) => {
    if (url.pathname === '/api/zones') {
      return jsonResponse(200, { zones: [{ path: ZONE }] })
    }
    if (url.pathname === '/api/zones/overview') {
      return jsonResponse(200, overview(status()))
    }
    if (url.pathname === '/api/zones/git-status') {
      return jsonResponse(200, status())
    }
    if (url.pathname === '/api/zones/tree') {
      return jsonResponse(200, treeResponse(ZONE, DIRECTORIES))
    }
    if (url.pathname === '/api/zones/entries') {
      return jsonResponse(200, listing(url.searchParams.get('path') ?? ''))
    }
    if (url.pathname === '/api/zones/file') {
      const path = url.searchParams.get('path') ?? ''
      return jsonResponse(200, {
        zone: ZONE,
        path,
        name: path.split('/').pop() ?? path,
        size: 4,
        preview: { kind: 'text', content: `content of ${path}` },
      })
    }
    if (url.pathname === '/api/zones/diff') {
      const path = url.searchParams.get('path') ?? ''
      const diff = DIFFS[path]
      return diff === undefined
        ? jsonResponse(404, { code: 'path-not-found', message: '没有该变更' })
        : jsonResponse(200, diff)
    }
    if (url.pathname === '/api/zones/commit') {
      return commit(String(init?.body ?? ''))
    }
    return undefined
  }
}

async function openChanges(user: UserEvent) {
  await user.click(screen.getByTestId('tab-files'))
  await screen.findByTestId('zone-list')
  await user.click(screen.getByTestId('open-zone'))
  await screen.findByTestId('zone-browser')
  await screen.findByTestId('browser-branch')
  await user.click(screen.getByTestId('browser-view-changes'))
  await screen.findByTestId('commit-bar')
}

async function confirmCommit(user: UserEvent) {
  await user.click(screen.getByTestId('commit-button'))
  const dialog = await screen.findByTestId('commit-confirm-dialog')
  await user.click(within(dialog).getByTestId('commit-confirm'))
}

beforeEach(() => {
  Object.defineProperty(window, 'innerWidth', { configurable: true, value: 1024 })
})

afterEach(() => {
  vi.unstubAllGlobals()
})

describe('经审查复核的整仓 Commit', () => {
  it('变更列表上方常驻提交信息与主按钮，空信息不能提交', async () => {
    const user = userEvent.setup()
    let status = gitStatus()
    mockFetch(
      standardHandler(
        () => status,
        () => jsonResponse(200, {}),
      ),
    )

    render(<App />)
    await openChanges(user)

    expect(screen.getByLabelText('提交信息')).toHaveValue('')
    const bar = screen.getByTestId('commit-bar')
    const list = screen.getByTestId('changes-list-scroll')
    expect(
      bar.compareDocumentPosition(list) & Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy()
    const button = screen.getByTestId('commit-button')
    expect(button).toHaveTextContent('Commit')
    expect(button).toBeDisabled()
    expect(screen.getByTestId('commit-message-required')).toHaveTextContent(
      '请输入提交信息后即可 Commit',
    )

    await user.type(screen.getByTestId('commit-message'), '   ')
    expect(button).toBeDisabled()
    expect(screen.getByTestId('commit-message-required')).toBeVisible()

    await user.clear(screen.getByTestId('commit-message'))
    await user.type(screen.getByTestId('commit-message'), 'reviewed message')
    expect(button).toBeEnabled()
    expect(screen.queryByTestId('commit-message-required')).toBeNull()

    // 工作树干净时没有可提交变更，按钮禁用并明确说明。
    status = gitStatus({ changes: [], pendingCount: 0 })
    await user.click(screen.getByTestId('browser-refresh'))
    await waitFor(() => expect(screen.getByTestId('commit-unavailable')).toBeVisible())
    expect(screen.getByTestId('commit-button')).toBeDisabled()
  })

  it('顶层 tab 与内部视图往返都保留提交信息', async () => {
    const user = userEvent.setup()
    mockFetch(
      standardHandler(
        () => gitStatus(),
        () => jsonResponse(200, {}),
      ),
    )

    render(<App />)
    await openChanges(user)
    await user.type(screen.getByTestId('commit-message'), 'keep this message')

    await user.click(screen.getByTestId('browser-view-browse'))
    await user.click(screen.getByTestId('browser-view-changes'))
    expect(screen.getByTestId('commit-message')).toHaveValue('keep this message')

    await user.click(screen.getByTestId('tab-sessions'))
    await user.click(screen.getByTestId('tab-files'))
    expect(screen.getByTestId('commit-message')).toHaveValue('keep this message')
  })

  it('确认框只有一句消息与确认/取消，取消不发送任何写请求', async () => {
    const user = userEvent.setup()
    let status = gitStatus()
    const requests = mockFetch(
      standardHandler(
        () => status,
        () => jsonResponse(200, {}),
      ),
    )

    render(<App />)
    await openChanges(user)
    await user.type(screen.getByTestId('commit-message'), 'unique-message-marker')
    await user.click(screen.getByTestId('commit-button'))

    const dialog = await screen.findByTestId('commit-confirm-dialog')
    const paragraphs = within(dialog).getAllByRole('paragraph')
    expect(paragraphs).toHaveLength(1)
    const text = dialog.textContent ?? ''
    expect(text).toContain('确认提交当前审查版本中的全部待提交变更？')
    expect(text).not.toContain(ZONE)
    expect(text).not.toContain('main')
    expect(text).not.toContain('unique-message-marker')
    expect(text).not.toContain('src/added.txt')
    expect(text).not.toContain('4')

    await user.click(within(dialog).getByTestId('commit-cancel'))
    expect(screen.queryByTestId('commit-confirm-dialog')).not.toBeInTheDocument()
    expect(requests.commits).toHaveLength(0)
    expect(screen.getByTestId('commit-message')).toHaveValue('unique-message-marker')
    status = gitStatus()
  })

  it('确认后只提交一次当前审查版本，成功后刷新实际状态', async () => {
    const user = userEvent.setup()
    let status = gitStatus()
    const requests = mockFetch(
      standardHandler(
        () => status,
        () => {
          status = gitStatus({ changes: [], pendingCount: 0, reviewToken: 'token-2' })
          return jsonResponse(200, {
            zone: ZONE,
            outcome: 'committed',
            commit: 'abcdef1234567890',
            branch: 'main',
            message: 'reviewed message',
          })
        },
      ),
    )

    render(<App />)
    await openChanges(user)
    await user.type(screen.getByTestId('commit-message'), 'reviewed message')
    await confirmCommit(user)

    const feedback = await screen.findByTestId('commit-feedback')
    expect(feedback).toHaveAttribute('data-tone', 'success')
    expect(feedback).toHaveTextContent('abcdef12')
    expect(feedback).toHaveTextContent('reviewed message')

    expect(requests.commits).toHaveLength(1)
    expect(JSON.parse(requests.commits[0])).toEqual({
      zone: ZONE,
      message: 'reviewed message',
      reviewToken: 'token-1',
    })
    await waitFor(() => expect(screen.getByTestId('commit-unavailable')).toBeVisible())
    expect(screen.getByTestId('commit-message')).toHaveValue('reviewed message')
  })

  it('审查失效时保留信息、刷新内容，并且只有再次确认才重新提交', async () => {
    const user = userEvent.setup()
    let status = gitStatus()
    const tokens: string[] = []
    const requests = mockFetch(
      standardHandler(
        () => status,
        (body) => {
          tokens.push(JSON.parse(body).reviewToken)
          if (tokens.length === 1) {
            status = gitStatus({ reviewToken: 'token-2' })
            return jsonResponse(409, {
              code: 'review-invalid',
              message: '审查版本已失效：内容已变化；本次没有提交，请重新审查。',
            })
          }
          status = gitStatus({ changes: [], pendingCount: 0, reviewToken: 'token-3' })
          return jsonResponse(200, {
            zone: ZONE,
            outcome: 'committed',
            commit: '1234567890abcdef',
            branch: 'main',
            message: 'reviewed again',
          })
        },
      ),
    )

    render(<App />)
    await openChanges(user)
    await user.type(screen.getByTestId('commit-message'), 'reviewed again')
    await confirmCommit(user)

    const feedback = await screen.findByTestId('commit-feedback')
    expect(feedback).toHaveAttribute('data-tone', 'error')
    expect(feedback).toHaveTextContent('审查版本已失效')
    expect(feedback).toHaveTextContent('提交信息已保留')
    expect(screen.getByTestId('commit-message')).toHaveValue('reviewed again')
    expect(requests.commits).toHaveLength(1)

    // 刷新后出现新的审查版本；必须再次点击 Commit 并确认。
    await waitFor(() => expect(screen.getByTestId('commit-button')).toBeEnabled())
    expect(requests.commits).toHaveLength(1)
    await confirmCommit(user)
    await waitFor(() => expect(tokens).toEqual(['token-1', 'token-2']))
    await waitFor(() =>
      expect(screen.getByTestId('commit-feedback')).toHaveAttribute(
        'data-tone',
        'success',
      ),
    )
    expect(requests.commits).toHaveLength(2)
  })

  it('响应中断显示未确认，不把写入当成功也不自动重试，并按实际状态刷新', async () => {
    const user = userEvent.setup()
    let status = gitStatus()
    let statusReads = 0
    const requests = mockFetch((url, init) => {
      if (url.pathname === '/api/zones/commit') {
        return Promise.reject(new TypeError('连接中断'))
      }
      if (url.pathname === '/api/zones/git-status') {
        statusReads += 1
        // 实际仓库可能已经提交：刷新后显示干净状态。
        if (statusReads > 1) {
          status = gitStatus({ changes: [], pendingCount: 0, reviewToken: 'token-2' })
        }
        return jsonResponse(200, status)
      }
      return standardHandler(
        () => status,
        () => jsonResponse(200, {}),
      )(url, init)
    })

    render(<App />)
    await openChanges(user)
    await user.type(screen.getByTestId('commit-message'), 'maybe committed')
    await confirmCommit(user)

    const feedback = await screen.findByTestId('commit-feedback')
    expect(feedback).toHaveAttribute('data-tone', 'unconfirmed')
    expect(feedback).toHaveTextContent('未确认')
    expect(feedback).toHaveTextContent('不会自动重试')
    expect(requests.commits).toHaveLength(1)
    await waitFor(() => expect(statusReads).toBeGreaterThan(1))
    expect(screen.getByTestId('commit-message')).toHaveValue('maybe committed')
  })

  it('执行中切换 tab 不取消已接受执行，返回后可见结果', async () => {
    const user = userEvent.setup()
    let status = gitStatus()
    let resolveCommit: ((response: Response) => void) | null = null
    const requests = mockFetch(
      standardHandler(
        () => status,
        () =>
          new Promise<Response>((resolve) => {
            resolveCommit = resolve
          }),
      ),
    )

    render(<App />)
    await openChanges(user)
    await user.type(screen.getByTestId('commit-message'), 'slow commit')
    await confirmCommit(user)
    expect(requests.commits).toHaveLength(1)

    await user.click(screen.getByTestId('tab-sessions'))
    await user.click(screen.getByTestId('tab-files'))
    expect(requests.commits).toHaveLength(1)
    expect(screen.queryByTestId('commit-feedback')).not.toBeInTheDocument()

    status = gitStatus({ changes: [], pendingCount: 0, reviewToken: 'token-2' })
    resolveCommit!(
      jsonResponse(200, {
        zone: ZONE,
        outcome: 'committed',
        commit: 'fedcba9876543210',
        branch: 'main',
        message: 'slow commit',
      }),
    )

    const feedback = await screen.findByTestId('commit-feedback')
    expect(feedback).toHaveAttribute('data-tone', 'success')
    expect(feedback).toHaveTextContent('fedcba98')
  })

  it('整页重载不重放已经发起的 Commit', async () => {
    const user = userEvent.setup()
    let pendingCommit: ((response: Response) => void) | null = null
    const requests = mockFetch(
      standardHandler(
        () => gitStatus(),
        () =>
          new Promise<Response>((resolve) => {
            pendingCommit = resolve
          }),
      ),
    )

    const first = render(<App />)
    await openChanges(user)
    await user.type(screen.getByTestId('commit-message'), 'reload must not replay')
    await confirmCommit(user)
    expect(requests.commits).toHaveLength(1)

    first.unmount()
    render(<App />)
    // 整页打开默认回到会话列表；没有自动提交请求。
    expect(screen.getByTestId('tab-sessions')).toHaveAttribute('aria-selected', 'true')
    expect(requests.commits).toHaveLength(1)
    pendingCommit!(
      jsonResponse(200, {
        zone: ZONE,
        outcome: 'committed',
        commit: '0011223344556677',
        branch: 'main',
        message: 'reload must not replay',
      }),
    )
    expect(requests.commits).toHaveLength(1)
  })

  it('Git 诊断按纯文本显示，不执行其中的 HTML 或 script', async () => {
    const user = userEvent.setup()
    let status = gitStatus()
    mockFetch(
      standardHandler(
        () => status,
        () =>
          jsonResponse(500, {
            code: 'git-command-failed',
            message:
              'git commit 失败：hook rejected <script>window.__pwned = 1</script><img src=x onerror="window.__pwned = 2">',
          }),
      ),
    )

    render(<App />)
    await openChanges(user)
    await user.type(screen.getByTestId('commit-message'), 'hook rejects')
    await confirmCommit(user)

    const feedback = await screen.findByTestId('commit-feedback')
    expect(feedback).toHaveAttribute('data-tone', 'error')
    expect(feedback).toHaveTextContent('<script>window.__pwned = 1</script>')
    expect('__pwned' in window).toBe(false)
    expect(feedback.querySelector('script')).toBeNull()
    expect(feedback.querySelector('img')).toBeNull()
  })

  it('本功能超时与已有输出按纯文本展示', async () => {
    const user = userEvent.setup()
    let status = gitStatus()
    mockFetch(
      standardHandler(
        () => status,
        () =>
          jsonResponse(504, {
            code: 'git-timeout',
            message:
              'git commit 超过本功能配置的 1 秒执行上限，已停止本次执行。取消前已有的输出：hook started',
          }),
      ),
    )

    render(<App />)
    await openChanges(user)
    await user.type(screen.getByTestId('commit-message'), 'will time out')
    await confirmCommit(user)

    const feedback = await screen.findByTestId('commit-feedback')
    expect(feedback).toHaveAttribute('data-tone', 'error')
    expect(feedback).toHaveTextContent('本功能配置的 1 秒执行上限')
    expect(feedback).toHaveTextContent('hook started')
  })

  it('普通目录没有 Commit，阻塞状态显示原因并禁用主按钮', async () => {
    const user = userEvent.setup()
    let status = gitStatus()
    mockFetch(
      standardHandler(
        () => status,
        () => jsonResponse(200, {}),
      ),
    )

    const view = render(<App />)
    await user.click(screen.getByTestId('tab-files'))
    await screen.findByTestId('zone-list')

    status = gitStatus({
      repository: false,
      branch: null,
      changes: [],
      pendingCount: 0,
      reviewToken: undefined,
    })
    await user.click(screen.getByTestId('open-zone'))
    await screen.findByTestId('zone-browser')
    await waitFor(() =>
      expect(screen.queryByTestId('browser-view-changes')).not.toBeInTheDocument(),
    )
    expect(screen.queryByTestId('commit-bar')).not.toBeInTheDocument()
    expect(screen.queryByTestId('commit-button')).not.toBeInTheDocument()

    // detached HEAD 与未完成流程：两条原因都必须明确显示，Commit 禁用。
    status = gitStatus({
      detached: true,
      blockedReason: '当前处于 detached HEAD：普通 Commit 不可用。',
    })
    view.unmount()
    mockFetch(
      standardHandler(
        () => status,
        () => jsonResponse(200, {}),
      ),
    )
    render(<App />)
    await openChanges(user)
    expect(screen.getByTestId('git-blocked-reason-summary')).toHaveTextContent(
      'detached HEAD',
    )
    expect(screen.getByTestId('git-blocked-reason')).toHaveTextContent('detached HEAD')
    expect(screen.getByTestId('commit-button')).toBeDisabled()

    status = gitStatus({
      blockedReason: '存在未完成的 merge 流程：请在外部工具完成或撤销后再试。',
    })
    await user.click(screen.getByTestId('browser-refresh'))
    await waitFor(() =>
      expect(screen.getByTestId('git-blocked-reason')).toHaveTextContent(
        '未完成的 merge',
      ),
    )
    expect(screen.getByTestId('commit-button')).toBeDisabled()
  })

  it('无首次提交的仓库以新增审查，并允许完成首个提交', async () => {
    const user = userEvent.setup()
    let status = gitStatus({
      branch: 'main',
      changes: [
        { path: 'staged.txt', kind: 'added' },
        { path: 'untracked.txt', kind: 'added' },
      ],
      pendingCount: 2,
      reviewToken: 'unborn-token',
    })
    const requests = mockFetch(
      standardHandler(
        () => status,
        () => {
          status = gitStatus({
            changes: [],
            pendingCount: 0,
            reviewToken: 'after-first-commit',
          })
          return jsonResponse(200, {
            zone: ZONE,
            outcome: 'committed',
            commit: '1111222233334444',
            branch: 'main',
            message: 'first commit',
          })
        },
      ),
    )

    render(<App />)
    await openChanges(user)
    expect(screen.getByTestId('change:staged.txt')).toHaveClass('change-added')
    expect(screen.getByTestId('change:untracked.txt')).toHaveClass('change-added')
    expect(screen.getByTestId('commit-button')).toBeDisabled()

    await user.type(screen.getByTestId('commit-message'), 'first commit')
    expect(screen.getByTestId('commit-button')).toBeEnabled()
    await confirmCommit(user)
    const feedback = await screen.findByTestId('commit-feedback')
    expect(feedback).toHaveAttribute('data-tone', 'success')
    expect(requests.commits).toHaveLength(1)
    expect(JSON.parse(requests.commits[0]).reviewToken).toBe('unborn-token')
  })

  it('冲突文件与普通失败区分：冲突显示明确警告并阻止 Commit', async () => {
    const user = userEvent.setup()
    const status = gitStatus({
      conflict: true,
      changes: [{ path: 'src/modified.txt', kind: 'conflicted' }, ...CHANGES.slice(0, 3)],
      pendingCount: CHANGES.length,
      blockedReason: '存在未解决的冲突文件：请在外部工具解决冲突后再试。',
    })
    mockFetch(
      standardHandler(
        () => status,
        () => jsonResponse(200, {}),
      ),
    )

    render(<App />)
    await openChanges(user)
    expect(screen.getByTestId('conflict-warning')).toBeVisible()
    expect(screen.getByTestId('git-blocked-reason')).toHaveTextContent('冲突')
    expect(screen.getByTestId('commit-button')).toBeDisabled()
  })
})
