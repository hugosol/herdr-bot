import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import userEvent, { type UserEvent } from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import App from '../src/app/App'
import type { BrowseEntry, DirectoryListing, FileView } from '../src/zones/browser/browseApi'
import { treeResponse } from './treeResponse'

const ZONE = 'C:\\work\\project'

function jsonResponse(status: number, body: unknown): Response {
  return {
    ok: status >= 200 && status < 300,
    status,
    json: async () => body,
  } as unknown as Response
}

type Handler = (url: URL) => Response | Promise<Response> | undefined

/** 记录浏览请求并按用例回答；未覆盖的地址一律视为错误。 */
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

const ROOT_ENTRIES: BrowseEntry[] = [
  { name: '.gitignore', path: '.gitignore', kind: 'file', size: 10 },
  { name: '.hidden.txt', path: '.hidden.txt', kind: 'file', size: 6 },
  { name: 'ignored.log', path: 'ignored.log', kind: 'file', size: 7 },
  { name: 'README', path: 'README', kind: 'file', size: 16 },
  { name: 'binary.txt', path: 'binary.txt', kind: 'file', size: 4 },
  { name: 'huge.txt', path: 'huge.txt', kind: 'file', size: 1_048_577 },
  { name: 'page.html', path: 'page.html', kind: 'file', size: 34 },
  { name: 'src', path: 'src', kind: 'directory' },
  {
    name: 'link-out',
    path: 'link-out',
    kind: 'link',
    linkTarget: 'C:\\outside\\target',
    linkReason: '链接指向 zone 之外；本服务不跟随，未读取目标内容。',
  },
]

const SRC_ENTRIES: BrowseEntry[] = [
  { name: 'nested', path: 'src/nested', kind: 'directory' },
  { name: 'main.ts', path: 'src/main.ts', kind: 'file', size: 15 },
]

const MANY_ENTRIES: BrowseEntry[] = Array.from({ length: 60 }, (_, index) => {
  const name = `${String(index + 1).padStart(3, '0')}.txt`
  return { name, path: `many/${name}`, kind: 'file', size: 5 } satisfies BrowseEntry
})

const DIRECTORIES: Record<string, BrowseEntry[]> = {
  '': ROOT_ENTRIES,
  src: SRC_ENTRIES,
  many: MANY_ENTRIES,
}

const FILES: Record<string, FileView> = {
  README: {
    zone: ZONE,
    path: 'README',
    name: 'README',
    size: 16,
    preview: { kind: 'text', content: 'extensionless text' },
  },
  '.hidden.txt': {
    zone: ZONE,
    path: '.hidden.txt',
    name: '.hidden.txt',
    size: 6,
    preview: { kind: 'text', content: 'hidden' },
  },
  '.gitignore': {
    zone: ZONE,
    path: '.gitignore',
    name: '.gitignore',
    size: 10,
    preview: { kind: 'text', content: 'ignored.log\n' },
  },
  'ignored.log': {
    zone: ZONE,
    path: 'ignored.log',
    name: 'ignored.log',
    size: 7,
    preview: { kind: 'text', content: 'ignored' },
  },
  'src/main.ts': {
    zone: ZONE,
    path: 'src/main.ts',
    name: 'main.ts',
    size: 15,
    preview: { kind: 'text', content: 'console.log(1)\n' },
  },
  'page.html': {
    zone: ZONE,
    path: 'page.html',
    name: 'page.html',
    size: 34,
    preview: { kind: 'text', content: '<script>window.__pwned=1</script>' },
  },
  'binary.txt': {
    zone: ZONE,
    path: 'binary.txt',
    name: 'binary.txt',
    size: 4,
    preview: { kind: 'binary', reason: '文件包含 NUL 字节，不是可文本预览的文本。' },
  },
  'huge.txt': {
    zone: ZONE,
    path: 'huge.txt',
    name: 'huge.txt',
    size: 1_048_577,
    preview: {
      kind: 'too-large',
      reason: '文件大小 1048577 字节，超过预览上限 1048576 字节；未加载完整内容。',
      size: 1_048_577,
      limit: 1_048_576,
    },
  },
}

function listing(path: string): DirectoryListing {
  return { zone: ZONE, path, entries: DIRECTORIES[path] ?? [] }
}

/** 标准 zone 响应：列表、目录与文件都来自固定夹具，便于断言页面行为。 */
function standardHandler(url: URL): Response | undefined {
  if (url.pathname === '/api/zones') {
    return jsonResponse(200, { zones: [{ path: ZONE }] })
  }
  if (url.pathname === '/api/zones/overview') {
    return jsonResponse(200, {
      zones: [{ path: ZONE, repository: false, conflict: false }],
    })
  }
  if (url.pathname === '/api/zones/git-status') {
    return jsonResponse(200, {
      zone: ZONE,
      repository: false,
      branch: null,
      detached: false,
      changes: [],
      conflict: false,
      pendingCount: 0,
    })
  }
  if (url.pathname === '/api/zones/tree') {
    return jsonResponse(200, treeResponse(ZONE, DIRECTORIES))
  }
  if (url.pathname === '/api/zones/entries') {
    return jsonResponse(200, listing(url.searchParams.get('path') ?? ''))
  }
  if (url.pathname === '/api/zones/file') {
    const path = url.searchParams.get('path') ?? ''
    const file = FILES[path]
    return file ? jsonResponse(200, file) : undefined
  }
  return undefined
}

async function openZone(user: UserEvent) {
  await user.click(screen.getByTestId('tab-files'))
  await screen.findByTestId('zone-list')
  await user.click(screen.getByTestId('open-zone'))
  await screen.findByTestId('zone-browser')
  await screen.findByTestId('entry:README')
}

beforeEach(() => {
  Object.defineProperty(window, 'innerWidth', { configurable: true, value: 1024 })
})

afterEach(() => {
  vi.unstubAllGlobals()
})

describe('zone 只读文件浏览', () => {
  it('进入 zone 时一次读取整树，展开目录不再发起读取', async () => {
    const user = userEvent.setup()
    const calls = mockFetch(standardHandler)

    render(<App />)
    await openZone(user)

    await waitFor(() => {
      expect(screen.queryByTestId('tree-loading')).toBeNull()
    })
    const treeReads = calls.filter((url) => url.pathname === '/api/zones/tree')
    expect(treeReads).toHaveLength(1)

    await user.click(screen.getByTestId('entry:src'))
    expect(await screen.findByTestId('entry:src/main.ts')).toBeVisible()
    expect(
      calls.filter((url) => url.pathname === '/api/zones/tree'),
    ).toHaveLength(1)
  })

  it('整树读取期间的刷新合并为补跑，不取消当前读取', async () => {
    const user = userEvent.setup()
    let settleRoot: (response: Response) => void = () => {}
    const deferredRoot = new Promise<Response>((resolve) => {
      settleRoot = resolve
    })
    let treeCalls = 0
    const calls = mockFetch((url) => {
      if (url.pathname === '/api/zones/tree') {
        treeCalls += 1
        if (treeCalls === 1) return deferredRoot
        return jsonResponse(200, treeResponse(ZONE, DIRECTORIES))
      }
      return standardHandler(url)
    })

    render(<App />)
    await user.click(screen.getByTestId('tab-files'))
    await screen.findByTestId('zone-list')
    await user.click(screen.getByTestId('open-zone'))
    await screen.findByTestId('zone-browser')
    expect(screen.getByTestId('tree-loading')).toBeVisible()
    expect(treeCalls).toBe(1)

    // 当前整树读取尚未完成时点刷新：应合并为尾随刷新，而不是并行再开一轮。
    await user.click(screen.getByTestId('browser-refresh'))
    expect(treeCalls).toBe(1)
    expect(calls.filter((url) => url.pathname === '/api/zones/tree')).toHaveLength(1)

    settleRoot(jsonResponse(200, treeResponse(ZONE, DIRECTORIES)))
    await waitFor(() => expect(screen.queryByTestId('tree-loading')).toBeNull())
    expect(await screen.findByTestId('entry:README')).toBeVisible()
    expect(treeCalls).toBe(2)
  })

  it('Git 整树省略 ignored 路径，显示隐藏提示并使用未忽略空态', async () => {
    const user = userEvent.setup()
    const filteredTree = treeResponse(
      ZONE,
      {
        '': [
          { name: '.gitignore', path: '.gitignore', kind: 'file', size: 10 },
          { name: 'empty', path: 'empty', kind: 'directory' },
          { name: 'kept', path: 'kept', kind: 'directory' },
        ],
        empty: [],
        kept: [{ name: 'tracked.txt', path: 'kept/tracked.txt', kind: 'file', size: 7 }],
      },
      true,
    )
    mockFetch((url) => {
      if (url.pathname === '/api/zones') {
        return jsonResponse(200, { zones: [{ path: ZONE }] })
      }
      if (url.pathname === '/api/zones/overview') {
        return jsonResponse(200, {
          zones: [{ path: ZONE, repository: true, conflict: false, pendingCount: 0 }],
        })
      }
      if (url.pathname === '/api/zones/git-status') {
        return jsonResponse(200, {
          zone: ZONE,
          repository: true,
          branch: 'main',
          detached: false,
          changes: [],
          conflict: false,
          pendingCount: 0,
        })
      }
      if (url.pathname === '/api/zones/tree') {
        return jsonResponse(200, filteredTree)
      }
      return undefined
    })

    render(<App />)
    await user.click(screen.getByTestId('tab-files'))
    await screen.findByTestId('zone-list')
    await user.click(screen.getByTestId('open-zone'))
    await screen.findByTestId('zone-browser')

    expect(await screen.findByTestId('ignored-hidden-notice')).toHaveTextContent(
      '已按 Git 忽略规则隐藏',
    )
    expect(screen.queryByTestId('entry:ignored.log')).toBeNull()
    expect(screen.queryByTestId('entry:.git')).toBeNull()
    expect(screen.getByTestId('entry:.gitignore')).toBeVisible()

    await user.click(screen.getByTestId('entry:empty'))
    expect(await screen.findByTestId('directory-empty:empty')).toHaveTextContent(
      '此目录没有未忽略的内容。',
    )

    await user.click(screen.getByTestId('entry:kept'))
    expect(await screen.findByTestId('entry:kept/tracked.txt')).toBeVisible()
  })

  it('手动刷新后隐藏当前文件时清除选择与内容', async () => {
    const user = userEvent.setup()
    let rootEntries: BrowseEntry[] = [{ name: 'README', path: 'README', kind: 'file', size: 16 }]
    mockFetch((url) => {
      if (url.pathname === '/api/zones') {
        return jsonResponse(200, { zones: [{ path: ZONE }] })
      }
      if (url.pathname === '/api/zones/overview') {
        return jsonResponse(200, {
          zones: [{ path: ZONE, repository: true, conflict: false, pendingCount: 0 }],
        })
      }
      if (url.pathname === '/api/zones/git-status') {
        return jsonResponse(200, {
          zone: ZONE,
          repository: true,
          branch: 'main',
          detached: false,
          changes: [],
          conflict: false,
          pendingCount: 0,
        })
      }
      if (url.pathname === '/api/zones/tree') {
        return jsonResponse(200, treeResponse(ZONE, { '': rootEntries }, true))
      }
      if (url.pathname === '/api/zones/file') {
        const path = url.searchParams.get('path') ?? ''
        return path === 'README' ? jsonResponse(200, FILES.README) : undefined
      }
      return undefined
    })

    render(<App />)
    await openZone(user)
    await user.click(screen.getByTestId('entry:README'))
    expect(await screen.findByTestId('file-text')).toHaveTextContent('extensionless text')

    rootEntries = [{ name: 'NEXT', path: 'NEXT', kind: 'file', size: 4 }]
    await user.click(screen.getByTestId('browser-refresh'))
    expect(await screen.findByTestId('entry:NEXT')).toBeVisible()
    expect(screen.queryByTestId('entry:README')).toBeNull()
    expect(screen.queryByTestId('file-view')).toBeNull()
    expect(screen.queryByTestId('file-text')).toBeNull()
  })

  it('从其他顶层 tab 返回时保留目录树快照，不自动重读', async () => {
    const user = userEvent.setup()
    const calls = mockFetch(standardHandler)

    render(<App />)
    await openZone(user)
    await waitFor(() => expect(screen.queryByTestId('tree-loading')).toBeNull())
    const readsBefore = calls.filter(
      (url) => url.pathname === '/api/zones/tree',
    ).length
    const statusBefore = calls.filter(
      (url) => url.pathname === '/api/zones/git-status',
    ).length

    await user.click(screen.getByTestId('tab-sessions'))
    await user.click(screen.getByTestId('tab-files'))

    await waitFor(() =>
      expect(
        calls.filter((url) => url.pathname === '/api/zones/git-status').length,
      ).toBeGreaterThan(statusBefore),
    )
    expect(screen.getByTestId('entry:README')).toBeVisible()
    expect(
      calls.filter((url) => url.pathname === '/api/zones/tree'),
    ).toHaveLength(readsBefore)
  })

  it('列出隐藏文件与忽略规则文件，并按内容读取无后缀文本', async () => {
    const user = userEvent.setup()
    const calls = mockFetch(standardHandler)

    render(<App />)
    await openZone(user)

    for (const name of ['.gitignore', '.hidden.txt', 'ignored.log', 'README']) {
      expect(screen.getByTestId(`entry:${name}`)).toBeVisible()
    }

    // 普通目录没有可用的 Git 变更视图或 Commit/Sync 控件。
    expect(screen.queryByText(/变更视图/)).toBeNull()
    expect(screen.queryByRole('button', { name: /Commit/ })).toBeNull()
    expect(screen.queryByRole('button', { name: /Sync/ })).toBeNull()

    await user.click(screen.getByTestId('entry:README'))
    expect(await screen.findByTestId('file-text')).toHaveTextContent('extensionless text')
    expect(screen.getByTestId('file-path')).toHaveTextContent('README')

    await user.click(screen.getByTestId('entry:.hidden.txt'))
    expect(await screen.findByTestId('file-text')).toHaveTextContent('hidden')

    await user.click(screen.getByTestId('entry:ignored.log'))
    expect(await screen.findByTestId('file-text')).toHaveTextContent('ignored')

    await user.click(screen.getByTestId('entry:.gitignore'))
    expect(await screen.findByTestId('file-text')).toHaveTextContent('ignored.log')

    const fileRequest = calls.find(
      (url) =>
        url.pathname === '/api/zones/file' && url.searchParams.get('path') === 'README',
    )
    expect(fileRequest?.searchParams.get('zone')).toBe(ZONE)
  })

  it('二进制与超限文件给出明确原因，不显示完整内容', async () => {
    const user = userEvent.setup()
    mockFetch(standardHandler)

    render(<App />)
    await openZone(user)

    await user.click(screen.getByTestId('entry:binary.txt'))
    const binary = await screen.findByTestId('file-unavailable')
    expect(binary).toHaveTextContent('NUL 字节')
    expect(screen.queryByTestId('file-text')).toBeNull()

    await user.click(screen.getByTestId('entry:huge.txt'))
    const huge = await screen.findByTestId('file-unavailable')
    expect(huge).toHaveTextContent('1048577')
    expect(huge).toHaveTextContent('1048576')
    expect(huge).toHaveTextContent('未加载完整内容')
    expect(screen.queryByTestId('file-text')).toBeNull()
  })

  it('目录或所选文件读取失败时明确报错，不伪装成空目录或空内容', async () => {
    const user = userEvent.setup()
    let failDirectories = false
    mockFetch((url) => {
      if (url.pathname === '/api/zones') {
        return jsonResponse(200, { zones: [{ path: ZONE }] })
      }
      if (url.pathname === '/api/zones/tree') {
        return failDirectories
          ? jsonResponse(404, {
              code: 'zone-unavailable',
              message: 'zone 目录已不存在或无法访问：拒绝访问。',
            })
          : jsonResponse(200, treeResponse(ZONE, DIRECTORIES))
      }
      if (url.pathname === '/api/zones/file') {
        return jsonResponse(404, {
          code: 'path-not-found',
          message: '路径在 zone 中不存在或已被删除。',
        })
      }
      return undefined
    })

    render(<App />)
    await openZone(user)

    await user.click(screen.getByTestId('entry:README'))
    expect(await screen.findByTestId('file-error')).toHaveTextContent('不存在或已被删除')
    expect(screen.queryByTestId('file-text')).toBeNull()

    // 外部删除后手动刷新：明确错误，且不显示空目录提示。
    failDirectories = true
    await user.click(screen.getByTestId('browser-refresh'))
    expect(await screen.findByTestId('directory-error')).toHaveTextContent(
      'zone 目录已不存在或无法访问',
    )
    expect(screen.queryByTestId('directory-empty')).toBeNull()
    expect(screen.queryByTestId('entry:README')).toBeNull()
  })

  it('跨顶层 tab 往返保留 zone、展开状态、所选文件与浏览滚动', async () => {
    const user = userEvent.setup()
    mockFetch(standardHandler)

    render(<App />)
    await openZone(user)

    await user.click(screen.getByTestId('entry:src'))
    expect(await screen.findByTestId('entry:src/main.ts')).toBeVisible()
    await user.click(screen.getByTestId('entry:src/main.ts'))
    expect(await screen.findByTestId('file-text')).toHaveTextContent('console.log(1)')

    const tree = screen.getByTestId('browser-tree')
    tree.scrollTop = 123
    fireEvent.scroll(tree)

    await user.click(screen.getByTestId('tab-sessions'))
    expect(screen.getByTestId('panel-files')).not.toBeVisible()

    await user.click(screen.getByTestId('tab-files'))
    expect(await screen.findByTestId('browser-zone')).toHaveTextContent(ZONE)
    expect(await screen.findByTestId('file-text')).toHaveTextContent('console.log(1)')
    expect(screen.getByTestId('entry:src/main.ts')).toBeVisible()
    expect(screen.getByTestId('browser-tree').scrollTop).toBe(123)
  })

  it('窄屏逐层进入、从文件内容返回并恢复列表滚动位置', async () => {
    Object.defineProperty(window, 'innerWidth', { configurable: true, value: 390 })
    const user = userEvent.setup()
    mockFetch((url) => {
      const standard = standardHandler(url)
      if (standard) return standard
      if (url.pathname === '/api/zones/entries') {
        return jsonResponse(200, listing(url.searchParams.get('path') ?? ''))
      }
      return undefined
    })

    render(<App />)
    await user.click(screen.getByTestId('tab-files'))
    await screen.findByTestId('zone-list')
    await user.click(screen.getByTestId('open-zone'))
    expect(await screen.findByTestId('browser-mobile')).toBeVisible()
    expect(screen.queryByTestId('browser-layout')).toBeNull()

    await user.click(await screen.findByTestId('entry:src'))
    expect(await screen.findByTestId('entry:src/main.ts')).toBeVisible()

    await user.click(screen.getByTestId('mobile-parent'))
    await user.click(await screen.findByTestId('entry:README'))
    expect(await screen.findByTestId('file-text')).toHaveTextContent('extensionless text')

    await user.click(screen.getByTestId('back-to-file-list'))
    expect(await screen.findByTestId('browser-mobile-list')).toBeVisible()
    expect(screen.getByTestId('entry:README')).toBeVisible()
  })

  it('窄屏从文件内容返回后恢复进入前的列表滚动位置', async () => {
    Object.defineProperty(window, 'innerWidth', { configurable: true, value: 390 })
    const user = userEvent.setup()
    const rootEntries = ROOT_ENTRIES.concat([
      { name: 'many', path: 'many', kind: 'directory' },
    ])
    mockFetch((url) => {
      if (url.pathname === '/api/zones') {
        return jsonResponse(200, { zones: [{ path: ZONE }] })
      }
      if (url.pathname === '/api/zones/tree') {
        return jsonResponse(
          200,
          treeResponse(ZONE, { ...DIRECTORIES, '': rootEntries }),
        )
      }
      if (url.pathname === '/api/zones/entries') {
        const path = url.searchParams.get('path') ?? ''
        const entries = path === '' ? rootEntries : (DIRECTORIES[path] ?? [])
        return jsonResponse(200, { zone: ZONE, path, entries })
      }
      if (url.pathname === '/api/zones/file') {
        const path = url.searchParams.get('path') ?? ''
        const file = FILES[path]
        if (file) return jsonResponse(200, file)
        // many/* 的文件内容用固定文本，便于断言进入的是哪一个文件。
        if (path.startsWith('many/')) {
          const name = path.slice('many/'.length)
          return jsonResponse(200, {
            zone: ZONE,
            path,
            name,
            size: 5,
            preview: { kind: 'text', content: name },
          })
        }
      }
      return undefined
    })

    render(<App />)
    await user.click(screen.getByTestId('tab-files'))
    await screen.findByTestId('zone-list')
    await user.click(screen.getByTestId('open-zone'))
    await screen.findByTestId('entry:many')

    const list = screen.getByTestId('browser-mobile-list')
    list.scrollTop = 250
    fireEvent.scroll(list)

    await user.click(screen.getByTestId('entry:many'))
    expect(await screen.findByTestId('entry:many/050.txt')).toBeVisible()
    const manyList = screen.getByTestId('browser-mobile-list')
    manyList.scrollTop = 320
    fireEvent.scroll(manyList)

    await user.click(screen.getByTestId('entry:many/050.txt'))
    expect(await screen.findByTestId('file-text')).toHaveTextContent('050')
    await user.click(screen.getByTestId('back-to-file-list'))

    expect(await screen.findByTestId('browser-mobile-list')).toBeVisible()
    expect(screen.getByTestId('browser-mobile-list').scrollTop).toBe(320)
  })

  it('HTML 文件可进入整页阅读模式，脚本被隔离且退出恢复原文件', async () => {
    const user = userEvent.setup()
    mockFetch(standardHandler)

    render(<App />)
    await openZone(user)

    await user.click(screen.getByTestId('entry:README'))
    await screen.findByTestId('file-text')
    expect(screen.queryByTestId('read-html')).toBeNull()

    await user.click(screen.getByTestId('entry:page.html'))
    const text = await screen.findByTestId('file-text')
    expect(text.textContent).toBe('<script>window.__pwned=1</script>')

    await user.click(screen.getByTestId('read-html'))
    const reader = screen.getByTestId('html-reader')
    expect(reader).toHaveAttribute('role', 'dialog')
    expect(reader).toHaveAttribute('aria-modal', 'true')
    expect(screen.getByTestId('exit-html-reader')).toBeVisible()
    expect(screen.getByTestId('html-reader-path')).toHaveTextContent('page.html')

    const frame = screen.getByTestId('html-reader-frame')
    expect(frame).toHaveAttribute('sandbox', 'allow-same-origin')
    expect(frame).toHaveAttribute('referrerpolicy', 'no-referrer')
    const document = frame.getAttribute('srcdoc') ?? ''
    expect(document).not.toContain('<script')
    expect(document).not.toContain('window.__pwned')
    expect(document).toContain('Content-Security-Policy')
    expect(document).toContain('/api/zones/reader/')
    expect(document).toContain('/page.html')
    expect('__pwned' in window).toBe(false)

    await user.click(screen.getByTestId('exit-html-reader'))
    expect(screen.queryByTestId('html-reader')).toBeNull()
    expect(await screen.findByTestId('file-text')).toHaveTextContent(
      '<script>window.__pwned=1</script>',
    )
    expect(screen.getByTestId('file-path')).toHaveTextContent('page.html')
  })

  it('阅读模式内打开 zone HTML，并把外部链接与 file 链接分流', async () => {
    const user = userEvent.setup()
    const open = vi.spyOn(window, 'open').mockImplementation(() => null)
    mockFetch((url) => {
      if (url.pathname === '/api/zones/file') {
        const path = url.searchParams.get('path') ?? ''
        if (path === 'page.html') {
          return jsonResponse(200, {
            zone: ZONE,
            path,
            name: 'page.html',
            size: 120,
            preview: {
              kind: 'text',
              content:
                '<a id="next" href="next.html">next</a><a id="outside" href="https://example.com/article">outside</a><a id="local" href="file:///C:/secret.html">local</a>',
            },
          })
        }
        if (path === 'next.html') {
          return jsonResponse(200, {
            zone: ZONE,
            path,
            name: 'next.html',
            size: 20,
            preview: { kind: 'text', content: '<h1 id="next-page">Next page</h1>' },
          })
        }
      }
      return standardHandler(url)
    })

    render(<App />)
    await openZone(user)
    await user.click(screen.getByTestId('entry:page.html'))
    await screen.findByTestId('file-text')
    await user.click(screen.getByTestId('read-html'))

    const frame = screen.getByTestId('html-reader-frame') as HTMLIFrameElement
    // jsdom 不填充 srcdoc 的 contentDocument；挂一份真实 Document 到 iframe 元素，驱动父页面的
    // 链接拦截。真实 iframe 文档的净化与渲染由 htmlReader.test 和 Chromium smoke 覆盖。
    const frameDocument = new DOMParser().parseFromString(
      '<a id="next" href="next.html">next</a>' +
        '<a id="outside" href="https://example.com/article">outside</a>' +
        '<a id="local" href="file:///C:/secret.html">local</a>',
      'text/html',
    )
    Object.defineProperty(frame, 'contentDocument', {
      configurable: true,
      value: frameDocument,
    })
    fireEvent.load(frame)

    frameDocument
      .querySelector('#next')!
      .dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true }))
    await waitFor(() => {
      expect(screen.getByTestId('html-reader-path')).toHaveTextContent('next.html')
      expect(frame.getAttribute('srcdoc')).toContain('next-page')
    })

    frameDocument
      .querySelector('#outside')!
      .dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true }))
    expect(open).toHaveBeenCalledWith(
      'https://example.com/article',
      '_blank',
      'noopener,noreferrer',
    )
    expect(screen.getByTestId('html-reader-path')).toHaveTextContent('next.html')

    frameDocument
      .querySelector('#local')!
      .dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true }))
    expect(await screen.findByTestId('html-reader-notice')).toHaveTextContent(
      'file:// 链接不会打开',
    )
    expect(screen.getByTestId('html-reader-path')).toHaveTextContent('next.html')
    open.mockRestore()
  })

  it('包含 HTML/script 的文件只作为文本展示', async () => {
    const user = userEvent.setup()
    mockFetch(standardHandler)

    render(<App />)
    await openZone(user)

    await user.click(screen.getByTestId('entry:page.html'))
    const text = await screen.findByTestId('file-text')
    expect(text.textContent).toBe('<script>window.__pwned=1</script>')
    expect('__pwned' in window).toBe(false)
    expect(document.querySelector('pre script')).toBeNull()
  })

  it('根外链接显示目标和不可进入原因，且没有进入按钮', async () => {
    const user = userEvent.setup()
    mockFetch(standardHandler)

    render(<App />)
    await openZone(user)

    expect(screen.getByTestId('link-target:link-out')).toHaveTextContent('C:\\outside\\target')
    expect(screen.getByTestId('entry:link-out')).toHaveTextContent('zone 之外')
    expect(screen.getByTestId('entry:link-out').querySelector('button')).toBeNull()
  })

  it('没有可用响应时给出未确认错误，不显示空目录', async () => {
    const user = userEvent.setup()
    mockFetch((url) => {
      if (url.pathname === '/api/zones') {
        return jsonResponse(200, { zones: [{ path: ZONE }] })
      }
      if (url.pathname === '/api/zones/tree') {
        return Promise.reject(new Error('connection reset'))
      }
      return undefined
    })

    render(<App />)
    await user.click(screen.getByTestId('tab-files'))
    await screen.findByTestId('zone-list')
    await user.click(screen.getByTestId('open-zone'))

    const error = await screen.findByTestId('directory-error')
    await waitFor(() => expect(error).toHaveTextContent('无法确认'))
    expect(screen.queryByTestId('directory-empty')).toBeNull()
  })
})
