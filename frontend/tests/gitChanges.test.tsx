import { render, screen, waitFor, within } from '@testing-library/react'
import userEvent, { type UserEvent } from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import App from '../src/App'
import type { BrowseEntry } from '../src/browseApi'
import type { GitChange, GitStatus, ZoneSummary } from '../src/gitApi'
import { treeResponse } from './treeResponse'

const ZONE = 'C:\\work\\repository'

function jsonResponse(status: number, body: unknown): Response {
  return {
    ok: status >= 200 && status < 300,
    status,
    json: async () => body,
  } as unknown as Response
}

type Handler = (url: URL) => Response | Promise<Response> | undefined

function mockFetch(handler: Handler): URL[] {
  const calls: URL[] = []
  vi.stubGlobal(
    'fetch',
    vi.fn(async (input: RequestInfo | URL) => {
      const url = new URL(String(input), 'http://localhost')
      calls.push(url)
      return (
        (await handler(url)) ??
        jsonResponse(404, { code: 'unknown-endpoint', message: '未知请求' })
      )
    }),
  )
  return calls
}

const DIRECTORIES: Record<string, BrowseEntry[]> = {
  '': [
    { name: '.gitignore', path: '.gitignore', kind: 'file', size: 10 },
    { name: 'ignored.log', path: 'ignored.log', kind: 'file', size: 7 },
    { name: 'plain.txt', path: 'plain.txt', kind: 'file', size: 5 },
    { name: 'src', path: 'src', kind: 'directory' },
  ],
  src: [
    { name: 'added.txt', path: 'src/added.txt', kind: 'file', size: 4 },
    { name: 'conflicted.txt', path: 'src/conflicted.txt', kind: 'file', size: 5 },
    { name: 'modified.txt', path: 'src/modified.txt', kind: 'file', size: 5 },
    { name: 'nested', path: 'src/nested', kind: 'directory' },
    { name: 'renamed-new.txt', path: 'src/renamed-new.txt', kind: 'file', size: 7 },
  ],
  'src/nested': [{ name: 'deep.txt', path: 'src/nested/deep.txt', kind: 'file', size: 4 }],
}

// Git zone 的整树响应已经省略 ignored.log；前端只展示后端给出的过滤结果。
const VISIBLE_DIRECTORIES: Record<string, BrowseEntry[]> = {
  ...DIRECTORIES,
  '': DIRECTORIES[''].filter((entry) => entry.name !== 'ignored.log'),
}

const CHANGES: GitChange[] = [
  { path: 'src/added.txt', kind: 'added' },
  { path: 'src/conflicted.txt', kind: 'conflicted' },
  { path: 'src/gone.txt', kind: 'deleted' },
  { path: 'src/modified.txt', kind: 'modified' },
  { path: 'src/nested/deep-gone.txt', kind: 'deleted' },
  { path: 'src/renamed-new.txt', oldPath: 'src/renamed-old.txt', kind: 'renamed' },
]

function gitStatus(overrides: Partial<GitStatus> = {}): GitStatus {
  return {
    zone: ZONE,
    repository: true,
    branch: 'main',
    detached: false,
    changes: CHANGES,
    conflict: true,
    pendingCount: CHANGES.length,
    ...overrides,
  }
}

function plainStatus(): GitStatus {
  return {
    zone: ZONE,
    repository: false,
    branch: null,
    detached: false,
    changes: [],
    conflict: false,
    pendingCount: 0,
  }
}

function overview(entries: ZoneSummary[]) {
  return { zones: entries }
}

function listing(path: string) {
  return { zone: ZONE, path, entries: DIRECTORIES[path] ?? [] }
}

function standardHandler(url: URL, status: () => GitStatus): Response | undefined {
  if (url.pathname === '/api/zones') {
    return jsonResponse(200, { zones: [{ path: ZONE }] })
  }
  if (url.pathname === '/api/zones/overview') {
    const current = status()
    return jsonResponse(
      200,
      overview([
        {
          path: ZONE,
          repository: current.repository,
          pendingCount: current.repository ? current.pendingCount : undefined,
          conflict: current.conflict,
        },
      ]),
    )
  }
  if (url.pathname === '/api/zones/git-status') {
    return jsonResponse(200, status())
  }
  if (url.pathname === '/api/zones/tree') {
    const current = status()
    return jsonResponse(
      200,
      treeResponse(
        ZONE,
        current.repository ? VISIBLE_DIRECTORIES : DIRECTORIES,
        current.repository,
      ),
    )
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
  return undefined
}

async function openZone(user: UserEvent) {
  await user.click(screen.getByTestId('tab-files'))
  await screen.findByTestId('zone-list')
  await user.click(screen.getByTestId('open-zone'))
  await screen.findByTestId('zone-browser')
  await screen.findByTestId('browser-branch')
}

beforeEach(() => {
  Object.defineProperty(window, 'innerWidth', { configurable: true, value: 1024 })
})

afterEach(() => {
  vi.unstubAllGlobals()
})

describe('Git zone：分支、内部视图与待提交定位', () => {
  it('显示当前分支与内部子 tab，并按变更类别着色且传播到现存祖先目录', async () => {
    const user = userEvent.setup()
    mockFetch((url) => standardHandler(url, () => gitStatus()))

    render(<App />)
    await openZone(user)

    expect(screen.getByTestId('browser-branch')).toHaveTextContent('main')
    expect(screen.getByTestId('browser-view-browse')).toHaveAttribute('aria-selected', 'true')
    expect(screen.getByTestId('browser-view-changes')).toHaveTextContent('变更（6）')

    // 文件颜色按类别落在浏览树；ignored 文件与 Git 元数据不出现在树中。
    expect(await screen.findByTestId('entry:src')).toHaveAttribute('data-change', 'directory')
    await user.click(screen.getByTestId('entry:src'))
    expect(await screen.findByTestId('entry:src/added.txt')).toHaveAttribute(
      'data-change',
      'added',
    )
    expect(screen.getByTestId('entry:src/modified.txt')).toHaveAttribute(
      'data-change',
      'modified',
    )
    expect(screen.getByTestId('entry:src/renamed-new.txt')).toHaveAttribute(
      'data-change',
      'renamed',
    )
    expect(screen.getByTestId('entry:src/conflicted.txt')).toHaveAttribute(
      'data-change',
      'conflicted',
    )
    expect(screen.getByTestId('entry:src/nested')).toHaveAttribute(
      'data-change',
      'directory',
    )
    expect(screen.getByTestId('entry:src/added.txt')).toHaveClass('change-added')
    expect(screen.getByTestId('entry:src/modified.txt')).toHaveClass('change-modified')
    expect(screen.getByTestId('entry:src/renamed-new.txt')).toHaveClass('change-modified')
    expect(screen.getByTestId('entry:src/conflicted.txt')).toHaveClass('change-conflicted')
    expect(screen.getByTestId('entry:src')).toHaveClass('change-directory')
    expect(screen.getByTestId('entry:src')).not.toHaveClass('change-added')
    expect(screen.getByTestId('entry:src')).not.toHaveClass('change-deleted')

    // 删除项不在浏览树伪造文件；它的现存祖先仍提示变化。
    expect(screen.queryByTestId('entry:src/gone.txt')).toBeNull()
    await user.click(screen.getByTestId('entry:src/nested'))
    expect(await screen.findByTestId('entry:src/nested/deep.txt')).not.toHaveAttribute(
      'data-change',
    )

    expect(screen.queryByTestId('entry:ignored.log')).toBeNull()
    expect(screen.queryByTestId('entry:.git')).toBeNull()
    expect(screen.getByTestId('ignored-hidden-notice')).toHaveTextContent(
      '已按 Git 忽略规则隐藏',
    )

    await user.click(screen.getByTestId('browser-view-changes'))
    const list = await screen.findByTestId('change-list')
    expect(within(list).getByTestId('change:src/added.txt')).toHaveAttribute(
      'data-change',
      'added',
    )
    expect(within(list).getByTestId('change:src/added.txt')).toHaveClass('change-added')
    expect(within(list).getByTestId('change:src/modified.txt')).toHaveAttribute(
      'data-change',
      'modified',
    )
    expect(within(list).getByTestId('change:src/modified.txt')).toHaveClass('change-modified')
    expect(within(list).getByTestId('change:src/renamed-new.txt')).toHaveAttribute(
      'data-change',
      'renamed',
    )
    expect(within(list).getByTestId('change:src/renamed-new.txt')).toHaveClass(
      'change-modified',
    )
    expect(within(list).getByTestId('change:src/renamed-new.txt')).toHaveTextContent(
      'src/renamed-old.txt',
    )
    expect(within(list).getByTestId('change:src/renamed-new.txt')).toHaveTextContent(
      'src/renamed-new.txt',
    )
    expect(within(list).getByTestId('change:src/gone.txt')).toHaveAttribute(
      'data-change',
      'deleted',
    )
    expect(within(list).getByTestId('change:src/gone.txt')).toHaveClass('change-deleted')
    expect(within(list).getByTestId('change:src/conflicted.txt')).toHaveAttribute(
      'data-change',
      'conflicted',
    )
    expect(within(list).getByTestId('change:src/conflicted.txt')).toHaveClass(
      'change-conflicted',
    )
    expect(within(list).getByTestId('change:src/conflicted.txt')).not.toHaveClass(
      'change-modified',
    )
    expect(screen.getByTestId('conflict-warning')).toHaveTextContent('冲突')
    expect(screen.queryByText(/^[AMD]$/)).toBeNull()
    expect(screen.queryByText(/^[AMD] .*/)).toBeNull()
    // Commit 入口已由 05 提供；这段状态带冲突且没有审查版本，因此主按钮必须不可执行。
    expect(screen.getByRole('button', { name: /Commit/ })).toBeDisabled()
    expect(screen.queryByRole('button', { name: /Sync/ })).toBeNull()
  })

  it('主动刷新反映外部切换的分支与新的变更集合', async () => {
    const user = userEvent.setup()
    let current = gitStatus()
    mockFetch((url) => standardHandler(url, () => current))

    render(<App />)
    await openZone(user)
    expect(screen.getByTestId('browser-branch')).toHaveTextContent('main')

    // 模拟外部切分支：下一次主动刷新必须得到新分支，而不是旧缓存。
    current = gitStatus({ branch: 'feature', changes: [], conflict: false, pendingCount: 0 })
    await user.click(screen.getByTestId('browser-refresh'))

    await waitFor(() =>
      expect(screen.getByTestId('browser-branch')).toHaveTextContent('feature'),
    )
    expect(screen.getByTestId('entry:src')).not.toHaveAttribute('data-change')
    await user.click(screen.getByTestId('browser-view-changes'))
    expect(await screen.findByTestId('changes-empty')).toBeVisible()
  })

  it('状态读取失败时显示明确错误，不显示干净或零变更', async () => {
    const user = userEvent.setup()
    mockFetch((url) => {
      if (url.pathname === '/api/zones/git-status') {
        return jsonResponse(500, {
          code: 'git-command-failed',
          message: 'git status 失败：fatal: 无法读取索引。',
        })
      }
      return standardHandler(url, () => gitStatus())
    })

    render(<App />)
    await user.click(screen.getByTestId('tab-files'))
    await screen.findByTestId('zone-list')
    await user.click(screen.getByTestId('open-zone'))

    const error = await screen.findByTestId('git-status-error')
    expect(error).toHaveTextContent('无法读取索引')
    expect(screen.queryByTestId('changes-empty')).toBeNull()
    expect(screen.queryByTestId('browser-branch')).toBeNull()
    expect(screen.queryByTestId('browser-view-changes')).toBeNull()
  })

  it('普通目录只有浏览，没有分支或变更入口', async () => {
    const user = userEvent.setup()
    mockFetch((url) => standardHandler(url, plainStatus))

    render(<App />)
    await user.click(screen.getByTestId('tab-files'))
    await screen.findByTestId('zone-list')
    await user.click(screen.getByTestId('open-zone'))
    await screen.findByTestId('zone-browser')

    expect(screen.queryByTestId('browser-branch')).toBeNull()
    expect(screen.queryByTestId('browser-view-changes')).toBeNull()
    expect(await screen.findByTestId('entry:plain.txt')).not.toHaveAttribute('data-change')
    expect(screen.queryByTestId('changes-view')).toBeNull()
  })
})

describe('zone 待提交角标', () => {
  it('显示后端确认的变更项数，不把目录或行数计入', async () => {
    const user = userEvent.setup()
    mockFetch((url) => {
      if (url.pathname === '/api/zones') {
        return jsonResponse(200, { zones: [{ path: ZONE }] })
      }
      if (url.pathname === '/api/zones/overview') {
        return jsonResponse(
          200,
          overview([
            {
              path: ZONE,
              repository: true,
              pendingCount: 4,
              conflict: false,
            },
          ]),
        )
      }
      return undefined
    })

    render(<App />)
    await user.click(screen.getByTestId('tab-files'))
    await screen.findByTestId('zone-list')

    const badge = await screen.findByTestId('zone-badge')
    expect(badge).toHaveTextContent(/^4$/)
    expect(badge).toHaveAttribute('aria-label', '4 项待提交')
  })

  it('单个 zone 概览失败显示紧凑警告并保留原因', async () => {
    const user = userEvent.setup()
    mockFetch((url) => {
      if (url.pathname === '/api/zones') {
        return jsonResponse(200, { zones: [{ path: ZONE }] })
      }
      if (url.pathname === '/api/zones/overview') {
        return jsonResponse(
          200,
          overview([
            {
              path: ZONE,
              repository: true,
              conflict: false,
              error: '拒绝访问 Git 元数据。',
            },
          ]),
        )
      }
      return undefined
    })

    render(<App />)
    await user.click(screen.getByTestId('tab-files'))
    await screen.findByTestId('zone-list')

    const badge = await screen.findByTestId('zone-badge')
    expect(badge).toHaveTextContent('⚠')
    expect(badge).toHaveAttribute('aria-label', '待提交状态不可用：拒绝访问 Git 元数据。')
    expect(badge).toHaveAttribute('title', '拒绝访问 Git 元数据。')
    expect(badge).not.toHaveTextContent('待提交状态不可用')
    expect(screen.queryByTestId('zone-detail')).toBeNull()
  })

  it('概览读取失败时明确显示未知，不显示 0 项或空角标', async () => {
    const user = userEvent.setup()
    mockFetch((url) => {
      if (url.pathname === '/api/zones') {
        return jsonResponse(200, { zones: [{ path: ZONE }] })
      }
      if (url.pathname === '/api/zones/overview') {
        return jsonResponse(500, {
          code: 'git-command-failed',
          message: '读取 zone 概览失败：拒绝访问。',
        })
      }
      return undefined
    })

    render(<App />)
    await user.click(screen.getByTestId('tab-files'))
    await screen.findByTestId('zone-list')

    const error = await screen.findByTestId('zone-overview-error')
    expect(error).toHaveTextContent('拒绝访问')
    expect(screen.queryByTestId('zone-badge')).toBeNull()
  })
})
