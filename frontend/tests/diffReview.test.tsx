import { fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import userEvent, { type UserEvent } from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import App from '../src/App'
import type { BrowseEntry } from '../src/browseApi'
import { treeResponse } from './treeResponse'
import type { ChangeDiff, DiffSide } from '../src/diffApi'
import type { GitChange, GitStatus, ZoneSummary } from '../src/gitApi'

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
    { name: 'src', path: 'src', kind: 'directory' },
    { name: 'README.md', path: 'README.md', kind: 'file', size: 6 },
  ],
  src: [
    { name: 'added.txt', path: 'src/added.txt', kind: 'file', size: 4 },
    { name: 'binary.txt', path: 'src/binary.txt', kind: 'file', size: 5 },
    { name: 'modified.txt', path: 'src/modified.txt', kind: 'file', size: 5 },
    { name: 'script.txt', path: 'src/script.txt', kind: 'file', size: 5 },
    { name: 'too-large.txt', path: 'src/too-large.txt', kind: 'file', size: 5 },
  ],
}

const CHANGES: GitChange[] = [
  { path: 'src/added.txt', kind: 'added' },
  { path: 'src/binary.txt', kind: 'modified' },
  { path: 'src/gone.txt', kind: 'deleted' },
  { path: 'src/modified.txt', kind: 'modified' },
  { path: 'src/renamed-new.txt', oldPath: 'src/renamed-old.txt', kind: 'renamed' },
  { path: 'src/script.txt', kind: 'modified' },
  { path: 'src/too-large.txt', kind: 'modified' },
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
  'src/binary.txt': {
    zone: ZONE,
    path: 'src/binary.txt',
    old: TEXT('plain old\n'),
    new: { kind: 'binary', reason: '文件包含 NUL 字节，不是可文本预览的文本。', size: 9 },
    complete: false,
  },
  'src/gone.txt': {
    zone: ZONE,
    path: 'src/gone.txt',
    old: TEXT('gone line\n'),
    new: { kind: 'absent' },
    complete: true,
  },
  'src/modified.txt': {
    zone: ZONE,
    path: 'src/modified.txt',
    old: TEXT('alpha\nbeta\ngamma\n'),
    new: TEXT('alpha\nBETA\ngamma\n'),
    complete: true,
  },
  'src/renamed-new.txt': {
    zone: ZONE,
    path: 'src/renamed-new.txt',
    oldPath: 'src/renamed-old.txt',
    old: TEXT('same line\nold tail\n'),
    new: TEXT('same line\nnew tail\n'),
    complete: true,
  },
  'src/script.txt': {
    zone: ZONE,
    path: 'src/script.txt',
    old: TEXT('safe\n'),
    new: TEXT(
      '<script>window.__pwned = 1</script>\n<img src="x" onerror="window.__pwned = 2">\n',
    ),
    complete: true,
  },
  'src/too-large.txt': {
    zone: ZONE,
    path: 'src/too-large.txt',
    old: {
      kind: 'too-large',
      reason: '旧侧内容大小 2048 字节，超过预览上限 1024 字节；未加载完整内容。',
      size: 2048,
      limit: 1024,
    },
    new: TEXT('small\n'),
    complete: false,
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
    ...overrides,
  }
}

function overview(): { zones: ZoneSummary[] } {
  return {
    zones: [
      {
        path: ZONE,
        repository: true,
        branch: 'main',
        pendingCount: CHANGES.length,
        conflict: false,
      },
    ],
  }
}

function standardHandler(url: URL): Response | undefined {
  if (url.pathname === '/api/zones') {
    return jsonResponse(200, { zones: [{ path: ZONE }] })
  }
  if (url.pathname === '/api/zones/overview') {
    return jsonResponse(200, overview())
  }
  if (url.pathname === '/api/zones/git-status') {
    return jsonResponse(200, gitStatus())
  }
  if (url.pathname === '/api/zones/tree') {
    return jsonResponse(200, treeResponse(ZONE, DIRECTORIES))
  }
  if (url.pathname === '/api/zones/entries') {
    const path = url.searchParams.get('path') ?? ''
    return jsonResponse(200, {
      zone: ZONE,
      path,
      entries: DIRECTORIES[path] ?? [],
    })
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
      ? jsonResponse(404, { code: 'path-not-found', message: '路径不存在' })
      : jsonResponse(200, diff)
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

async function openChanges(user: UserEvent) {
  await openZone(user)
  await user.click(screen.getByTestId('browser-view-changes'))
  await screen.findByTestId('change-list')
}

beforeEach(() => {
  Object.defineProperty(window, 'innerWidth', { configurable: true, value: 1024 })
  delete (window as unknown as Record<string, unknown>).__pwned
})

afterEach(() => {
  vi.unstubAllGlobals()
  delete (window as unknown as Record<string, unknown>).__pwned
})

describe('变更审查 diff：HEAD 到最终本地内容', () => {
  it('宽屏左列表右 diff，默认左右对照并显示删除/新增行', async () => {
    const user = userEvent.setup()
    const calls = mockFetch(standardHandler)

    render(<App />)
    await openChanges(user)

    expect(screen.getByTestId('changes-layout')).toBeVisible()
    expect(screen.getByTestId('diff-placeholder')).toBeVisible()

    await user.click(screen.getByTestId('change:src/modified.txt'))
    const diff = await screen.findByTestId('diff-view')
    expect(diff).toHaveAttribute('data-mode', 'side-by-side')
    expect(screen.getByTestId('diff-basis')).toHaveTextContent(
      '最新本地提交 → 最终本地内容',
    )
    expect(screen.getByTestId('diff-path')).toHaveTextContent('src/modified.txt')

    expect(screen.getAllByTestId('diff-old-removed')[0]).toHaveTextContent('beta')
    expect(screen.getAllByTestId('diff-new-added')[0]).toHaveTextContent('BETA')
    expect(screen.getAllByTestId('diff-old-context')[0]).toHaveTextContent('alpha')
    expect(screen.getAllByTestId('diff-new-context')).toHaveLength(
      screen.getAllByTestId('diff-old-context').length,
    )
    expect(screen.getByTestId('change:src/modified.txt')).toHaveAttribute(
      'aria-current',
      'true',
    )

    const request = calls.find((call) => call.pathname === '/api/zones/diff')
    expect(request?.searchParams.get('zone')).toBe(ZONE)
    expect(request?.searchParams.get('path')).toBe('src/modified.txt')
    expect(request?.searchParams.has('oldPath')).toBe(false)
  })

  it('重命名审查使用 Git 的旧路径读取旧侧', async () => {
    const user = userEvent.setup()
    const calls = mockFetch(standardHandler)

    render(<App />)
    await openChanges(user)
    await user.click(screen.getByTestId('change:src/renamed-new.txt'))

    const diff = await screen.findByTestId('diff-view')
    expect(diff).toHaveAttribute('data-mode', 'side-by-side')
    expect(screen.getByTestId('diff-path')).toHaveTextContent('src/renamed-old.txt')
    expect(screen.getByTestId('diff-path')).toHaveTextContent('src/renamed-new.txt')
    expect(screen.getAllByTestId('diff-old-removed')[0]).toHaveTextContent('old tail')
    expect(screen.getAllByTestId('diff-new-added')[0]).toHaveTextContent('new tail')

    const request = calls.find((call) => call.pathname === '/api/zones/diff')
    expect(request?.searchParams.get('oldPath')).toBe('src/renamed-old.txt')
  })

  it('新增与删除变更分别显示旧侧为空、新侧为空', async () => {
    const user = userEvent.setup()
    mockFetch(standardHandler)

    render(<App />)
    await openChanges(user)

    await user.click(screen.getByTestId('change:src/added.txt'))
    await screen.findByTestId('diff-view')
    expect(screen.getAllByTestId('diff-new-text').map((cell) => cell.textContent)).toEqual([
      'brand',
      'new',
    ])
    expect(screen.queryByTestId('diff-old-removed')).toBeNull()

    await user.click(screen.getByTestId('change:src/gone.txt'))
    await waitFor(() =>
      expect(screen.getAllByTestId('diff-old-text').map((cell) => cell.textContent)).toEqual([
        'gone line',
      ]),
    )
    expect(screen.queryByTestId('diff-new-added')).toBeNull()
  })

  it('diff 内容按纯文本展示，HTML 与脚本不会执行', async () => {
    const user = userEvent.setup()
    mockFetch(standardHandler)

    const { container } = render(<App />)
    await openChanges(user)
    await user.click(screen.getByTestId('change:src/script.txt'))

    const diff = await screen.findByTestId('diff-view')
    expect(diff).toHaveTextContent('<script>window.__pwned = 1</script>')
    expect(diff).toHaveTextContent('<img src="x" onerror="window.__pwned = 2">')
    expect(container.querySelector('script')).toBeNull()
    expect(container.querySelector('img')).toBeNull()
    expect((window as unknown as Record<string, unknown>).__pwned).toBeUndefined()
  })

  it('任一侧超限或二进制时不渲染完整 diff，明确限制且不隐藏变更行', async () => {
    const user = userEvent.setup()
    mockFetch(standardHandler)

    render(<App />)
    await openChanges(user)

    // 所有待提交项仍然在列表中，角标计数与类别着色不变。
    expect(screen.getByTestId('browser-view-changes')).toHaveTextContent('变更（7）')
    for (const change of CHANGES) {
      const row = screen.getByTestId(`change:${change.path}`)
      expect(row).toHaveAttribute('data-change', change.kind)
      expect(row).toHaveClass(
        change.kind === 'added'
          ? 'change-added'
          : change.kind === 'deleted'
            ? 'change-deleted'
            : 'change-modified',
      )
    }

    await user.click(screen.getByTestId('change:src/too-large.txt'))
    const unavailable = await screen.findByTestId('diff-unavailable')
    expect(unavailable).toHaveTextContent('旧侧')
    expect(unavailable).toHaveTextContent('2048')
    expect(unavailable).toHaveTextContent('1024')
    expect(screen.queryByTestId('diff-view')).toBeNull()

    await user.click(screen.getByTestId('change:src/binary.txt'))
    await waitFor(() =>
      expect(screen.getByTestId('diff-unavailable')).toHaveTextContent('NUL'),
    )
    expect(screen.getByTestId('diff-unavailable')).toHaveTextContent('新侧')
    expect(screen.queryByTestId('diff-view')).toBeNull()
    expect(screen.getByTestId('change:src/too-large.txt')).toBeVisible()

    // 浏览视图同样保留这些文件与待提交着色，预览不可用不会缩小审查集合或变更计数。
    await user.click(screen.getByTestId('browser-view-browse'))
    await user.click(await screen.findByTestId('entry:src'))
    expect(await screen.findByTestId('entry:src/too-large.txt')).toHaveAttribute(
      'data-change',
      'modified',
    )
    expect(screen.getByTestId('entry:src/binary.txt')).toHaveAttribute(
      'data-change',
      'modified',
    )
    expect(screen.getByTestId('browser-view-changes')).toHaveTextContent('变更（7）')
  })

  it('切换内部视图与顶层 tab 后保留所选变更、diff 和列表滚动位置', async () => {
    const user = userEvent.setup()
    mockFetch(standardHandler)

    render(<App />)
    await openChanges(user)

    const list = screen.getByTestId('changes-list-scroll')
    list.scrollTop = 180
    fireEvent.scroll(list)

    await user.click(screen.getByTestId('change:src/modified.txt'))
    await screen.findByTestId('diff-view')

    await user.click(screen.getByTestId('browser-view-browse'))
    expect(screen.getByTestId('change:src/modified.txt')).toHaveAttribute(
      'aria-current',
      'true',
    )
    await user.click(screen.getByTestId('browser-view-changes'))
    expect(await screen.findByTestId('diff-view')).toHaveAttribute(
      'data-mode',
      'side-by-side',
    )
    expect(screen.getByTestId('changes-list-scroll').scrollTop).toBe(180)

    await user.click(screen.getByTestId('tab-sessions'))
    await user.click(screen.getByTestId('tab-files'))
    await waitFor(() =>
      expect(screen.getByTestId('diff-view')).toHaveAttribute(
        'data-mode',
        'side-by-side',
      ),
    )
    expect(screen.getByTestId('change:src/modified.txt')).toHaveAttribute(
      'aria-current',
      'true',
    )
    expect(screen.getByTestId('changes-list-scroll').scrollTop).toBe(180)
  })
})

describe('窄屏变更审查：统一 diff 与明确返回', () => {
  beforeEach(() => {
    Object.defineProperty(window, 'innerWidth', { configurable: true, value: 390 })
  })

  it('默认在列表中进入统一 diff，返回后恢复列表滚动位置', async () => {
    const user = userEvent.setup()
    mockFetch(standardHandler)

    render(<App />)
    await openChanges(user)

    expect(screen.getByTestId('changes-mobile')).toBeVisible()
    expect(screen.queryByTestId('diff-view')).toBeNull()

    const list = screen.getByTestId('changes-list-scroll')
    list.scrollTop = 320
    fireEvent.scroll(list)

    await user.click(screen.getByTestId('change:src/modified.txt'))
    const diff = await screen.findByTestId('diff-view')
    expect(diff).toHaveAttribute('data-mode', 'unified')
    expect(screen.getAllByTestId('diff-row-removed')[0]).toHaveTextContent('beta')
    expect(screen.getAllByTestId('diff-row-added')[0]).toHaveTextContent('BETA')
    expect(screen.getByTestId('back-to-changes-list')).toBeVisible()

    await user.click(screen.getByTestId('back-to-changes-list'))
    const restored = await screen.findByTestId('changes-list-scroll')
    expect(restored).toBeVisible()
    expect(restored.scrollTop).toBe(320)
    expect(screen.queryByTestId('diff-view')).toBeNull()
  })

  it('窄屏下超限/二进制仍可见并显示限制说明', async () => {
    const user = userEvent.setup()
    mockFetch(standardHandler)

    render(<App />)
    await openChanges(user)

    await user.click(screen.getByTestId('change:src/too-large.txt'))
    const unavailable = await screen.findByTestId('diff-unavailable')
    expect(unavailable).toHaveTextContent('2048')
    expect(unavailable).toHaveTextContent('1024')
    expect(screen.queryByTestId('diff-view')).toBeNull()

    const back = screen.getByTestId('back-to-changes-list')
    await user.click(back)
    const list = await screen.findByTestId('changes-list-scroll')
    expect(within(list).getByTestId('change:src/too-large.txt')).toBeVisible()
    expect(within(list).getByTestId('change:src/binary.txt')).toBeVisible()
  })
})
