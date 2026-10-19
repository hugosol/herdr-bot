import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type ReactNode,
} from 'react'

import {
  fetchDirectory,
  fetchFile,
  type BrowseEntry,
  type BrowseResult,
  type FileView,
} from './browseApi'
import { fetchGitStatus, type ChangeKind, type GitChange, type GitStatus } from './gitApi'
import type { ConnectionStatus } from './sessionFeed'
import {
  describeWatchProblem,
  type WorkspaceWatchEntry,
} from './workspaceWatch'

/** 窄屏断点：与 styles.css 的浏览器布局断点保持一致。 */
const NARROW_LAYOUT_MAX_WIDTH = 720

/**
 * Git 变更类别到视觉类别的映射。两个视图共用同一份映射，因此同一变更在浏览树与变更列表中
 * 颜色一致；新增绿、修改与已识别重命名蓝、删除红、冲突用独立警告色。
 */
const CHANGE_CLASS: Record<ChangeKind, string> = {
  added: 'change-added',
  modified: 'change-modified',
  deleted: 'change-deleted',
  renamed: 'change-modified',
  conflicted: 'change-conflicted',
}

/** 给颜色补上非视觉文字，供读屏与测试使用；界面不显示 A/M/D 状态字母。 */
const CHANGE_LABEL: Record<ChangeKind, string> = {
  added: '新增',
  modified: '修改',
  deleted: '删除',
  renamed: '重命名',
  conflicted: '冲突',
}

function useNarrowLayout(): boolean {
  const [narrow, setNarrow] = useState(() => window.innerWidth <= NARROW_LAYOUT_MAX_WIDTH)
  useEffect(() => {
    const update = () => setNarrow(window.innerWidth <= NARROW_LAYOUT_MAX_WIDTH)
    update()
    window.addEventListener('resize', update)
    return () => window.removeEventListener('resize', update)
  }, [])
  return narrow
}

/** 把非成功结果翻译成页面上的明确说明；未得到响应时不能冒充空目录或空内容。 */
function failureMessage(result: BrowseResult<unknown>, target: string): string {
  return result.kind === 'rejected'
    ? result.message
    : `无法确认 ${target} 的读取结果：请求没有得到可用响应。`
}

function previewPane(file: FileView) {
  if (file.preview.kind === 'text') {
    return (
      <pre className="file-view__text" data-testid="file-text">
        {file.preview.content}
      </pre>
    )
  }
  if (file.preview.kind === 'binary') {
    return (
      <p className="file-view__limited" data-testid="file-unavailable" role="note">
        不可文本预览：{file.preview.reason}
      </p>
    )
  }
  return (
    <p className="file-view__limited" data-testid="file-unavailable" role="note">
      文件大小 {file.preview.size} 字节，超过预览上限 {file.preview.limit} 字节；未加载完整内容。
    </p>
  )
}

/**
 * workspace 内的只读浏览：电脑左树右内容，窄屏逐层进入并在文件内容处明确返回。
 *
 * 所有读取都经过后端公开入口；页面只显示后端返回的事实，不自行判断路径边界或文本类型。
 * 组件在顶层 tab 切换时保持挂载：workspace、展开状态、所选文件与浏览滚动因此保留；回到页面时
 * 重新读取当前目录与所选文件，不把旧结果当成当前事实。
 */
export function WorkspaceBrowser({
  workspace,
  active,
  watch,
  onBack,
}: {
  workspace: string
  active: boolean
  watch?: {
    entry: WorkspaceWatchEntry | null
    connection: ConnectionStatus
    generation: number
  }
  onBack: () => void
}) {
  const [entries, setEntries] = useState<Map<string, BrowseEntry[]>>(() => new Map())
  const [directoryErrors, setDirectoryErrors] = useState<Map<string, string>>(() => new Map())
  const [loadingDirectories, setLoadingDirectories] = useState<Set<string>>(() => new Set())
  const [expanded, setExpanded] = useState<Set<string>>(() => new Set())
  const [selectedPath, setSelectedPath] = useState<string | null>(null)
  const [file, setFile] = useState<FileView | null>(null)
  const [fileError, setFileError] = useState<string | null>(null)
  const [fileLoading, setFileLoading] = useState(false)
  const [refreshing, setRefreshing] = useState(false)
  const [mobilePath, setMobilePath] = useState('')
  const [mobileShowingContent, setMobileShowingContent] = useState(false)
  // Git 状态与内部视图：普通目录只有浏览，不会出现变更视图或分支信息。
  const [gitStatus, setGitStatus] = useState<GitStatus | null>(null)
  const [gitError, setGitError] = useState<string | null>(null)
  const [view, setView] = useState<'browse' | 'changes'>('browse')

  const narrow = useNarrowLayout()
  const watchGeneration = watch?.generation ?? 0
  const watchProblem = describeWatchProblem(
    watch?.connection ?? 'live',
    watch?.entry ?? null,
  )
  const gitSequence = useRef(0)
  const directorySequences = useRef(new Map<string, number>())
  const fileSequence = useRef(0)
  const desktopTreeRef = useRef<HTMLDivElement>(null)
  const contentRef = useRef<HTMLDivElement>(null)
  const mobileListRef = useRef<HTMLDivElement>(null)
  const scrollPositions = useRef(new Map<string, number>())
  const activePreviously = useRef(active)
  const seenWatchGeneration = useRef(watchGeneration)

  const loadDirectory = useCallback(
    async (path: string) => {
      const sequence = (directorySequences.current.get(path) ?? 0) + 1
      directorySequences.current.set(path, sequence)
      setLoadingDirectories((previous) => new Set(previous).add(path))

      const result = await fetchDirectory(workspace, path)
      if (directorySequences.current.get(path) !== sequence) return

      setLoadingDirectories((previous) => {
        const next = new Set(previous)
        next.delete(path)
        return next
      })
      if (result.kind === 'ok') {
        setEntries((previous) => new Map(previous).set(path, result.value.entries))
        setDirectoryErrors((previous) => {
          const next = new Map(previous)
          next.delete(path)
          return next
        })
      } else {
        // 读取失败时清除该目录的旧内容：不能继续把上一次的结果显示成当前事实。
        setEntries((previous) => {
          const next = new Map(previous)
          next.delete(path)
          return next
        })
        setDirectoryErrors((previous) =>
          new Map(previous).set(
            path,
            failureMessage(result, path === '' ? '根目录' : `目录 ${path}`),
          ),
        )
      }
    },
    [workspace],
  )

  const loadFile = useCallback(
    async (path: string) => {
      const sequence = fileSequence.current + 1
      fileSequence.current = sequence
      setFileLoading(true)
      setFile(null)
      setFileError(null)

      const result = await fetchFile(workspace, path)
      if (fileSequence.current !== sequence) return

      setFileLoading(false)
      if (result.kind === 'ok') {
        setFile(result.value)
      } else {
        setFileError(failureMessage(result, `文件 ${path}`))
      }
    },
    [workspace],
  )

  const loadGitStatus = useCallback(async () => {
    const sequence = gitSequence.current + 1
    gitSequence.current = sequence
    const result = await fetchGitStatus(workspace)
    if (gitSequence.current !== sequence) return
    if (result.kind === 'ok') {
      setGitStatus(result.value)
      setGitError(null)
      if (!result.value.repository) setView('browse')
    } else {
      // 读取失败时清除旧状态：不能把上一次的分支或「没有变更」继续显示成当前事实。
      setGitStatus(null)
      setGitError(
        result.kind === 'rejected'
          ? result.message
          : '无法确认 Git 状态：请求没有得到可用响应。',
      )
      setView('browse')
    }
  }, [workspace])

  const refresh = useCallback(async () => {
    const paths = new Set(entries.keys())
    paths.add('')
    setRefreshing(true)
    await Promise.all([...paths].map((path) => loadDirectory(path)).concat(loadGitStatus()))
    if (selectedPath !== null) await loadFile(selectedPath)
    setRefreshing(false)
  }, [entries, selectedPath, loadDirectory, loadFile, loadGitStatus])

  const refreshLatest = useRef(refresh)
  refreshLatest.current = refresh

  // 打开 workspace（或切换 workspace）时从根目录重新读取，并清空上一份临时浏览状态。
  useEffect(() => {
    setEntries(new Map())
    setDirectoryErrors(new Map())
    setExpanded(new Set())
    setSelectedPath(null)
    setFile(null)
    setFileError(null)
    setMobilePath('')
    setMobileShowingContent(false)
    setGitStatus(null)
    setGitError(null)
    setView('browse')
    scrollPositions.current = new Map()
    void loadDirectory('')
    void loadGitStatus()
  }, [workspace, loadDirectory, loadGitStatus])

  // 从其他顶层 tab 回到文件列表时，重新读取当前目录与所选文件，保留展开、选择与滚动。
  useEffect(() => {
    if (active && !activePreviously.current) {
      void refreshLatest.current()
    }
    activePreviously.current = active
  }, [active])

  // 后端合并后的刷新通知只决定「何时重新读取」；读取本身与手动刷新完全相同，不另设缓存。
  useEffect(() => {
    if (watchGeneration === seenWatchGeneration.current) return
    seenWatchGeneration.current = watchGeneration
    void refreshLatest.current()
  }, [watchGeneration])

  // 换一个文件时从内容顶部开始显示；否则上一份长内容的滚动位置会让新文件看起来是空白的。
  useEffect(() => {
    if (contentRef.current) contentRef.current.scrollTop = 0
  }, [selectedPath])

  // 窄屏列表回到前台时恢复该目录的滚动位置（进入目录/文件前已记录）。
  useLayoutEffect(() => {
    if (!narrow || mobileShowingContent) return
    const list = mobileListRef.current
    if (list) list.scrollTop = scrollPositions.current.get(mobilePath) ?? 0
  }, [narrow, mobileShowingContent, mobilePath, entries])

  function rememberMobileScroll() {
    const list = mobileListRef.current
    if (list) scrollPositions.current.set(mobilePath, list.scrollTop)
  }

  function selectFile(path: string) {
    rememberMobileScroll()
    setSelectedPath(path)
    setMobileShowingContent(true)
    void loadFile(path)
  }

  function toggleDirectory(path: string) {
    const willExpand = !expanded.has(path)
    setExpanded((previous) => {
      const next = new Set(previous)
      if (next.has(path)) next.delete(path)
      else next.add(path)
      return next
    })
    if (willExpand && !entries.has(path)) void loadDirectory(path)
  }

  function openMobileDirectory(path: string) {
    rememberMobileScroll()
    setMobilePath(path)
    setMobileShowingContent(false)
    if (!entries.has(path)) void loadDirectory(path)
  }

  function goMobileUp() {
    rememberMobileScroll()
    const parent = mobilePath.includes('/')
      ? mobilePath.slice(0, mobilePath.lastIndexOf('/'))
      : ''
    setMobilePath(parent)
    setMobileShowingContent(false)
    if (!entries.has(parent)) void loadDirectory(parent)
  }

  // 两个视图共用同一个状态快照：浏览树的颜色、祖先提示与变更列表都来自这份 changes。
  const changeByPath = new Map<string, GitChange>()
  const changedPaths: string[] = []
  for (const change of gitStatus?.changes ?? []) {
    // 极端情况下同一路径可以同时出现「已暂存删除 + 未跟踪新增」；浏览树只显示磁盘上真正存在的
    // 文件，因此后到的删除不能覆盖仍存在文件的着色。
    const existing = changeByPath.get(change.path)
    if (existing === undefined || existing.kind === 'deleted') {
      changeByPath.set(change.path, change)
    }
    changedPaths.push(change.path)
    // 删除与重命名旧路径已不在磁盘上，但它们的现存祖先仍应提示变化。
    if (change.kind === 'renamed' && change.oldPath !== undefined) {
      changedPaths.push(change.oldPath)
    }
  }

  /** 目录只要包含任意待提交变更（含已删除路径）就提示蓝色；不继承子文件的绿/红。 */
  function directoryHasChanges(path: string) {
    const prefix = `${path}/`
    return changedPaths.some(
      (changed) => changed === path || changed.startsWith(prefix),
    )
  }

  function changesView(): ReactNode {
    if (gitError !== null) {
      return (
        <p className="browser-error" data-testid="git-status-error" role="alert">
          {gitError}
        </p>
      )
    }
    if (gitStatus === null) {
      return (
        <p className="browser-loading" data-testid="git-status-loading" role="status">
          正在读取 Git 状态……
        </p>
      )
    }
    if (!gitStatus.repository) return null

    return (
      <section className="changes" data-testid="changes-view">
        {gitStatus.conflict && (
          <p className="change-conflict-warning" data-testid="conflict-warning" role="alert">
            存在冲突文件：冲突不是普通修改，请先在外部工具中解决。
          </p>
        )}
        {gitStatus.changes.length === 0 ? (
          <p className="browser-empty" data-testid="changes-empty">
            没有待提交变更。
          </p>
        ) : (
          <ul className="change-list" data-testid="change-list">
            {gitStatus.changes.map((change) => (
              <li
                key={`${change.kind}:${change.path}<-${change.oldPath ?? ''}`}
                className={`change-row ${CHANGE_CLASS[change.kind]}`}
                data-testid={`change:${change.path}`}
                data-change={change.kind}
                aria-label={`${change.path}：${CHANGE_LABEL[change.kind]}`}
              >
                <code className="change-row__path">
                  {change.kind === 'renamed' && change.oldPath !== undefined ? (
                    <>
                      <span className="change-row__old">{change.oldPath}</span>
                      <span aria-hidden="true"> → </span>
                      <span>{change.path}</span>
                    </>
                  ) : (
                    change.path
                  )}
                </code>
                {change.kind === 'renamed' && (
                  <span className="change-row__note">重命名</span>
                )}
                {change.kind === 'conflicted' && (
                  <span className="change-row__warning">冲突</span>
                )}
              </li>
            ))}
          </ul>
        )}
      </section>
    )
  }

  function filePane(): ReactNode {
    if (selectedPath === null) {
      return (
        <p className="browser-placeholder" data-testid="file-placeholder">
          选择文件以只读查看内容。
        </p>
      )
    }
    if (fileLoading) {
      return (
        <p className="browser-loading" data-testid="file-loading" role="status">
          正在读取文件……
        </p>
      )
    }
    if (fileError !== null) {
      return (
        <p className="browser-error" data-testid="file-error" role="alert">
          {fileError}
        </p>
      )
    }
    if (file === null) return null
    return (
      <article className="file-view" data-testid="file-view">
        <header className="file-view__header">
          <code className="file-view__path" data-testid="file-path">
            {file.path}
          </code>
          <span className="file-view__size" data-testid="file-size">
            {file.size} 字节
          </span>
        </header>
        {previewPane(file)}
      </article>
    )
  }

  function directoryState(path: string, content: ReactNode): ReactNode {
    const error = directoryErrors.get(path)
    if (error !== undefined) {
      return (
        <p
          className="browser-error"
          data-testid={path === '' ? 'directory-error' : `directory-error:${path}`}
          role="alert"
        >
          {error}
        </p>
      )
    }
    if (entries.get(path) === undefined) {
      if (!loadingDirectories.has(path)) return null
      return (
        <p
          className="browser-loading"
          data-testid={path === '' ? 'directory-loading' : `directory-loading:${path}`}
          role="status"
        >
          正在读取目录……
        </p>
      )
    }
    return content
  }

  function desktopTree(path: string, depth: number): ReactNode {
    const error = directoryErrors.get(path)
    if (error !== undefined) {
      return (
        <p
          className="browser-error browser-error--inline"
          data-testid={path === '' ? 'directory-error' : `directory-error:${path}`}
          role="alert"
        >
          {error}
        </p>
      )
    }
    const directoryEntries = entries.get(path)
    if (directoryEntries === undefined) {
      return loadingDirectories.has(path) ? (
        <p
          className="browser-loading"
          data-testid={path === '' ? 'directory-loading' : `directory-loading:${path}`}
          role="status"
        >
          正在读取目录……
        </p>
      ) : null
    }
    if (directoryEntries.length === 0) {
      return (
        <p
          className="browser-empty"
          data-testid={path === '' ? 'directory-empty' : `directory-empty:${path}`}
        >
          此目录为空。
        </p>
      )
    }

    return (
      <ul className="tree-list" style={depth === 0 ? undefined : { marginLeft: '0.9rem' }}>
        {directoryEntries.map((entry) => {
          if (entry.kind === 'directory') {
            const isExpanded = expanded.has(entry.path)
            const hasChanges = directoryHasChanges(entry.path)
            return (
              <li key={entry.path} className="tree-item">
                <button
                  type="button"
                  className={
                    hasChanges
                      ? 'tree-row tree-row--directory change-directory'
                      : 'tree-row tree-row--directory'
                  }
                  data-testid={`entry:${entry.path}`}
                  data-change={hasChanges ? 'directory' : undefined}
                  aria-expanded={isExpanded}
                  onClick={() => toggleDirectory(entry.path)}
                >
                  <span className="tree-row__caret" aria-hidden="true">
                    {isExpanded ? '▾' : '▸'}
                  </span>
                  <span className="tree-row__name">{entry.name}</span>
                </button>
                {isExpanded && desktopTree(entry.path, depth + 1)}
              </li>
            )
          }

          if (entry.kind === 'file') {
            const selected = selectedPath === entry.path
            const change = changeByPath.get(entry.path)
            return (
              <li key={entry.path} className="tree-item">
                <button
                  type="button"
                  className={[
                    'tree-row',
                    'tree-row--file',
                    selected ? 'is-selected' : '',
                    change ? CHANGE_CLASS[change.kind] : '',
                  ]
                    .filter(Boolean)
                    .join(' ')}
                  data-testid={`entry:${entry.path}`}
                  data-change={change?.kind}
                  aria-current={selected ? 'true' : undefined}
                  aria-label={
                    change === undefined
                      ? undefined
                      : `${entry.name}：${CHANGE_LABEL[change.kind]}`
                  }
                  onClick={() => selectFile(entry.path)}
                >
                  <span className="tree-row__name">{entry.name}</span>
                </button>
              </li>
            )
          }

          return (
            <li key={entry.path} className="tree-item tree-item--link">
              <div className="tree-row tree-row--link" data-testid={`entry:${entry.path}`}>
                <span className="tree-row__name">{entry.name}</span>
                <span className="tree-row__link-target" data-testid={`link-target:${entry.path}`}>
                  → {entry.linkTarget ?? '目标不可读取'}
                </span>
                <span className="tree-row__link-reason">
                  {entry.linkReason ?? '链接不会被跟随。'}
                </span>
              </div>
            </li>
          )
        })}
      </ul>
    )
  }

  function mobileEntryList(path: string): ReactNode {
    const directoryEntries = entries.get(path)
    if (directoryEntries === undefined) return null
    if (directoryEntries.length === 0) {
      return (
        <p
          className="browser-empty"
          data-testid={path === '' ? 'directory-empty' : `directory-empty:${path}`}
        >
          此目录为空。
        </p>
      )
    }
    return (
      <ul className="mobile-list">
        {directoryEntries.map((entry) => {
          if (entry.kind === 'directory') {
            const hasChanges = directoryHasChanges(entry.path)
            return (
              <li key={entry.path} className="mobile-item">
                <button
                  type="button"
                  className={
                    hasChanges
                      ? 'mobile-row mobile-row--directory change-directory'
                      : 'mobile-row mobile-row--directory'
                  }
                  data-testid={`entry:${entry.path}`}
                  data-change={hasChanges ? 'directory' : undefined}
                  onClick={() => openMobileDirectory(entry.path)}
                >
                  <span className="tree-row__name">{entry.name}</span>
                  <span className="mobile-row__hint" aria-hidden="true">
                    ›
                  </span>
                </button>
              </li>
            )
          }
          if (entry.kind === 'file') {
            const change = changeByPath.get(entry.path)
            return (
              <li key={entry.path} className="mobile-item">
                <button
                  type="button"
                  className={[
                    'mobile-row',
                    'mobile-row--file',
                    change ? CHANGE_CLASS[change.kind] : '',
                  ]
                    .filter(Boolean)
                    .join(' ')}
                  data-testid={`entry:${entry.path}`}
                  data-change={change?.kind}
                  aria-label={
                    change === undefined
                      ? undefined
                      : `${entry.name}：${CHANGE_LABEL[change.kind]}`
                  }
                  onClick={() => selectFile(entry.path)}
                >
                  <span className="tree-row__name">{entry.name}</span>
                  <span className="mobile-row__hint" aria-hidden="true">
                    ›
                  </span>
                </button>
              </li>
            )
          }
          return (
            <li key={entry.path} className="mobile-item mobile-item--link">
              <div className="mobile-row mobile-row--link" data-testid={`entry:${entry.path}`}>
                <span className="tree-row__name">{entry.name}</span>
                <span className="tree-row__link-target">
                  → {entry.linkTarget ?? '目标不可读取'}
                </span>
                <span className="tree-row__link-reason">
                  {entry.linkReason ?? '链接不会被跟随。'}
                </span>
              </div>
            </li>
          )
        })}
      </ul>
    )
  }

  return (
    <section className="browser" data-testid="workspace-browser">
      <header className="browser-toolbar">
        <button
          type="button"
          className="browser-toolbar__button"
          data-testid="back-to-workspaces"
          onClick={onBack}
        >
          返回 workspace 列表
        </button>
        <code className="browser-toolbar__workspace" data-testid="browser-workspace">
          {workspace}
        </code>
        {gitStatus?.repository && (
          <span className="browser-toolbar__branch" data-testid="browser-branch">
            分支：
            {gitStatus.branch ?? (gitStatus.detached ? 'HEAD（已分离）' : '未知')}
          </span>
        )}
        <button
          type="button"
          className="browser-toolbar__button"
          data-testid="browser-refresh"
          onClick={() => void refresh()}
          disabled={refreshing}
        >
          {refreshing ? '刷新中……' : '刷新'}
        </button>
      </header>

      {watchProblem !== null && (
        <p
          className="watch-warning"
          data-testid={watchProblem.kind === 'listener' ? 'watch-error' : 'watch-stale'}
          role="alert"
        >
          {watchProblem.text}
        </p>
      )}

      {gitError !== null && (
        <p className="browser-error" data-testid="git-status-error" role="alert">
          {gitError}
        </p>
      )}

      {/* Git workspace 才有的内部子 tab；普通目录只保留浏览，不出现空的变更入口。 */}
      {gitStatus?.repository && (
        <div className="browser-views" role="tablist" aria-label="workspace 视图">
          <button
            type="button"
            role="tab"
            className={view === 'browse' ? 'browser-view is-active' : 'browser-view'}
            aria-selected={view === 'browse'}
            data-testid="browser-view-browse"
            onClick={() => setView('browse')}
          >
            文件浏览
          </button>
          <button
            type="button"
            role="tab"
            className={view === 'changes' ? 'browser-view is-active' : 'browser-view'}
            aria-selected={view === 'changes'}
            data-testid="browser-view-changes"
            onClick={() => setView('changes')}
          >
            变更{gitStatus.pendingCount > 0 ? `（${gitStatus.pendingCount}）` : ''}
          </button>
        </div>
      )}

      {view === 'changes' && gitStatus?.repository ? (
        changesView()
      ) : narrow ? (
        <div className="browser-mobile" data-testid="browser-mobile">
          {mobileShowingContent ? (
            <>
              <button
                type="button"
                className="browser-mobile__back"
                data-testid="back-to-file-list"
                onClick={() => setMobileShowingContent(false)}
              >
                返回文件列表
              </button>
              {filePane()}
            </>
          ) : (
            <>
              <p className="browser-current-path" data-testid="browser-current-path">
                {mobilePath === '' ? workspace : `${workspace}\\${mobilePath}`}
              </p>
              {mobilePath !== '' && (
                <button
                  type="button"
                  className="browser-mobile__back"
                  data-testid="mobile-parent"
                  onClick={goMobileUp}
                >
                  上一级
                </button>
              )}
              <div
                className="browser-mobile__list"
                data-testid="browser-mobile-list"
                ref={mobileListRef}
                onScroll={rememberMobileScroll}
              >
                {directoryState(mobilePath, mobileEntryList(mobilePath))}
              </div>
            </>
          )}
        </div>
      ) : (
        <div className="browser-layout" data-testid="browser-layout">
          <div className="browser-tree" data-testid="browser-tree" ref={desktopTreeRef}>
            {desktopTree('', 0)}
          </div>
          <div className="browser-content" data-testid="browser-content" ref={contentRef}>
            {filePane()}
          </div>
        </div>
      )}
    </section>
  )
}
