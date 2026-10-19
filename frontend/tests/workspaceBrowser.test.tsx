import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import userEvent, { type UserEvent } from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import App from '../src/App'
import type { BrowseEntry, DirectoryListing, FileView } from '../src/browseApi'

const WORKSPACE = 'C:\\work\\project'

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
  { name: '.git', path: '.git', kind: 'directory' },
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
    linkReason: '链接指向 workspace 之外；本服务不跟随，未读取目标内容。',
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

const GIT_ENTRIES: BrowseEntry[] = [
  { name: 'HEAD', path: '.git/HEAD', kind: 'file', size: 21 },
]

const DIRECTORIES: Record<string, BrowseEntry[]> = {
  '': ROOT_ENTRIES,
  '.git': GIT_ENTRIES,
  src: SRC_ENTRIES,
  many: MANY_ENTRIES,
}

const FILES: Record<string, FileView> = {
  README: {
    workspace: WORKSPACE,
    path: 'README',
    name: 'README',
    size: 16,
    preview: { kind: 'text', content: 'extensionless text' },
  },
  '.hidden.txt': {
    workspace: WORKSPACE,
    path: '.hidden.txt',
    name: '.hidden.txt',
    size: 6,
    preview: { kind: 'text', content: 'hidden' },
  },
  '.gitignore': {
    workspace: WORKSPACE,
    path: '.gitignore',
    name: '.gitignore',
    size: 10,
    preview: { kind: 'text', content: 'ignored.log\n' },
  },
  'ignored.log': {
    workspace: WORKSPACE,
    path: 'ignored.log',
    name: 'ignored.log',
    size: 7,
    preview: { kind: 'text', content: 'ignored' },
  },
  '.git/HEAD': {
    workspace: WORKSPACE,
    path: '.git/HEAD',
    name: 'HEAD',
    size: 21,
    preview: { kind: 'text', content: 'ref: refs/heads/main\n' },
  },
  'src/main.ts': {
    workspace: WORKSPACE,
    path: 'src/main.ts',
    name: 'main.ts',
    size: 15,
    preview: { kind: 'text', content: 'console.log(1)\n' },
  },
  'page.html': {
    workspace: WORKSPACE,
    path: 'page.html',
    name: 'page.html',
    size: 34,
    preview: { kind: 'text', content: '<script>window.__pwned=1</script>' },
  },
  'binary.txt': {
    workspace: WORKSPACE,
    path: 'binary.txt',
    name: 'binary.txt',
    size: 4,
    preview: { kind: 'binary', reason: '文件包含 NUL 字节，不是可文本预览的文本。' },
  },
  'huge.txt': {
    workspace: WORKSPACE,
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
  return { workspace: WORKSPACE, path, entries: DIRECTORIES[path] ?? [] }
}

/** 标准工作区答复：列表、目录与文件都来自固定夹具，便于断言页面行为。 */
function standardHandler(url: URL): Response | undefined {
  if (url.pathname === '/api/workspaces') {
    return jsonResponse(200, { workspaces: [{ path: WORKSPACE }] })
  }
  if (url.pathname === '/api/workspaces/entries') {
    return jsonResponse(200, listing(url.searchParams.get('path') ?? ''))
  }
  if (url.pathname === '/api/workspaces/file') {
    const path = url.searchParams.get('path') ?? ''
    const file = FILES[path]
    return file ? jsonResponse(200, file) : undefined
  }
  return undefined
}

async function openWorkspace(user: UserEvent) {
  await user.click(screen.getByTestId('tab-files'))
  await screen.findByTestId('workspace-list')
  await user.click(screen.getByTestId('open-workspace'))
  await screen.findByTestId('workspace-browser')
  await screen.findByTestId('entry:README')
}

beforeEach(() => {
  Object.defineProperty(window, 'innerWidth', { configurable: true, value: 1024 })
})

afterEach(() => {
  vi.unstubAllGlobals()
})

describe('workspace 只读文件浏览', () => {
  it('列出隐藏文件、忽略文件与 Git 元数据，并按内容读取无后缀文本', async () => {
    const user = userEvent.setup()
    const calls = mockFetch(standardHandler)

    render(<App />)
    await openWorkspace(user)

    for (const name of ['.git', '.gitignore', '.hidden.txt', 'ignored.log', 'README']) {
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

    await user.click(screen.getByTestId('entry:.git'))
    await user.click(await screen.findByTestId('entry:.git/HEAD'))
    expect(await screen.findByTestId('file-text')).toHaveTextContent('ref: refs/heads/main')

    const fileRequest = calls.find(
      (url) =>
        url.pathname === '/api/workspaces/file' && url.searchParams.get('path') === 'README',
    )
    expect(fileRequest?.searchParams.get('workspace')).toBe(WORKSPACE)
  })

  it('二进制与超限文件给出明确原因，不显示完整内容', async () => {
    const user = userEvent.setup()
    mockFetch(standardHandler)

    render(<App />)
    await openWorkspace(user)

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
      if (url.pathname === '/api/workspaces') {
        return jsonResponse(200, { workspaces: [{ path: WORKSPACE }] })
      }
      if (url.pathname === '/api/workspaces/entries') {
        return failDirectories
          ? jsonResponse(404, {
              code: 'workspace-unavailable',
              message: 'workspace 目录已不存在或无法访问：拒绝访问。',
            })
          : jsonResponse(200, listing(url.searchParams.get('path') ?? ''))
      }
      if (url.pathname === '/api/workspaces/file') {
        return jsonResponse(404, {
          code: 'path-not-found',
          message: '路径在 workspace 中不存在或已被删除。',
        })
      }
      return undefined
    })

    render(<App />)
    await openWorkspace(user)

    await user.click(screen.getByTestId('entry:README'))
    expect(await screen.findByTestId('file-error')).toHaveTextContent('不存在或已被删除')
    expect(screen.queryByTestId('file-text')).toBeNull()

    // 外部删除后手动刷新：明确错误，且不显示空目录提示。
    failDirectories = true
    await user.click(screen.getByTestId('browser-refresh'))
    expect(await screen.findByTestId('directory-error')).toHaveTextContent(
      'workspace 目录已不存在或无法访问',
    )
    expect(screen.queryByTestId('directory-empty')).toBeNull()
    expect(screen.queryByTestId('entry:README')).toBeNull()
  })

  it('跨顶层 tab 往返保留 workspace、展开状态、所选文件与浏览滚动', async () => {
    const user = userEvent.setup()
    mockFetch(standardHandler)

    render(<App />)
    await openWorkspace(user)

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
    expect(await screen.findByTestId('browser-workspace')).toHaveTextContent(WORKSPACE)
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
      if (url.pathname === '/api/workspaces/entries') {
        return jsonResponse(200, listing(url.searchParams.get('path') ?? ''))
      }
      return undefined
    })

    render(<App />)
    await user.click(screen.getByTestId('tab-files'))
    await screen.findByTestId('workspace-list')
    await user.click(screen.getByTestId('open-workspace'))
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
      if (url.pathname === '/api/workspaces') {
        return jsonResponse(200, { workspaces: [{ path: WORKSPACE }] })
      }
      if (url.pathname === '/api/workspaces/entries') {
        const path = url.searchParams.get('path') ?? ''
        const entries = path === '' ? rootEntries : (DIRECTORIES[path] ?? [])
        return jsonResponse(200, { workspace: WORKSPACE, path, entries })
      }
      if (url.pathname === '/api/workspaces/file') {
        const path = url.searchParams.get('path') ?? ''
        const file = FILES[path]
        if (file) return jsonResponse(200, file)
        // many/* 的文件内容用固定文本，便于断言进入的是哪一个文件。
        if (path.startsWith('many/')) {
          const name = path.slice('many/'.length)
          return jsonResponse(200, {
            workspace: WORKSPACE,
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
    await screen.findByTestId('workspace-list')
    await user.click(screen.getByTestId('open-workspace'))
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

  it('包含 HTML/script 的文件只作为文本展示', async () => {
    const user = userEvent.setup()
    mockFetch(standardHandler)

    render(<App />)
    await openWorkspace(user)

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
    await openWorkspace(user)

    expect(screen.getByTestId('link-target:link-out')).toHaveTextContent('C:\\outside\\target')
    expect(screen.getByTestId('entry:link-out')).toHaveTextContent('workspace 之外')
    expect(screen.getByTestId('entry:link-out').querySelector('button')).toBeNull()
  })

  it('没有可用响应时给出未确认错误，不显示空目录', async () => {
    const user = userEvent.setup()
    mockFetch((url) => {
      if (url.pathname === '/api/workspaces') {
        return jsonResponse(200, { workspaces: [{ path: WORKSPACE }] })
      }
      if (url.pathname === '/api/workspaces/entries') {
        return Promise.reject(new Error('connection reset'))
      }
      return undefined
    })

    render(<App />)
    await user.click(screen.getByTestId('tab-files'))
    await screen.findByTestId('workspace-list')
    await user.click(screen.getByTestId('open-workspace'))

    const error = await screen.findByTestId('directory-error')
    await waitFor(() => expect(error).toHaveTextContent('无法确认'))
    expect(screen.queryByTestId('directory-empty')).toBeNull()
  })
})
