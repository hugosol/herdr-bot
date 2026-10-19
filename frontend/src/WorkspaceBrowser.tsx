import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type ReactNode,
} from 'react'

import {
  fetchFile,
  fetchTree,
  type BrowseEntry,
  type BrowseResult,
  type FileView,
} from './browseApi'
import { diffRows, sideBySideRows } from './diff'
import {
  fetchChangeDiff,
  type ChangeDiff,
  type DiffSide,
} from './diffApi'
import {
  checkWorkspaceRemote,
  commitWorkspace,
  fetchGitStatus,
  syncWorkspace,
  type ChangeKind,
  type GitChange,
  type GitStatus,
} from './gitApi'
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

/** 变更项身份：同一路径可能同时出现不同类别，比较时要把类别与旧路径一起纳入。 */
function changeIdentity(change: GitChange): string {
  return `${change.kind}:${change.path}<-${change.oldPath ?? ''}`
}

/** 单侧不能展示完整 diff 的原因；可展示时返回 null。 */
function diffSideProblem(side: DiffSide, label: string): string | null {
  if (side.kind === 'binary') return `${label}不可文本 diff：${side.reason}`
  if (side.kind === 'too-large') {
    return `${label}大小 ${side.size} 字节，超过预览上限 ${side.limit} 字节；未加载完整内容`
  }
  return null
}

/** 任一侧超限或二进制都不渲染完整 diff，而是明确说明限制发生在哪一侧。 */
function diffUnavailableReason(diff: ChangeDiff): string {
  const problems = [
    diffSideProblem(diff.old, '旧侧'),
    diffSideProblem(diff.new, '新侧'),
  ].filter((problem): problem is string => problem !== null)
  const detail =
    problems.length > 0
      ? problems.map((problem) => problem.replace(/。$/, '')).join('；')
      : '没有可渲染的文本内容'
  return `不可展示完整 diff：${detail}。`
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
  const [ignoredHidden, setIgnoredHidden] = useState(false)
  const [expanded, setExpanded] = useState<Set<string>>(() => new Set())
  const [selectedPath, setSelectedPath] = useState<string | null>(null)
  const [file, setFile] = useState<FileView | null>(null)
  const [fileError, setFileError] = useState<string | null>(null)
  const [fileLoading, setFileLoading] = useState(false)
  const [refreshing, setRefreshing] = useState(false)
  // 进入 workspace 时预读整棵目录树；展开目录只切换显示，不再等待网络。
  const [treeLoading, setTreeLoading] = useState(false)
  const [mobilePath, setMobilePath] = useState('')
  const [mobileShowingContent, setMobileShowingContent] = useState(false)
  // Git 状态与内部视图：普通目录只有浏览，不会出现变更视图或分支信息。
  const [gitStatus, setGitStatus] = useState<GitStatus | null>(null)
  const [gitError, setGitError] = useState<string | null>(null)
  const [view, setView] = useState<'browse' | 'changes'>('browse')
  // 变更审查：选中的变更、它的 HEAD→最终内容 diff，以及窄屏是否正停留在 diff 上。
  const [selectedChange, setSelectedChange] = useState<GitChange | null>(null)
  const [changeDiff, setChangeDiff] = useState<ChangeDiff | null>(null)
  const [changeDiffError, setChangeDiffError] = useState<string | null>(null)
  const [changeDiffLoading, setChangeDiffLoading] = useState(false)
  const [changesShowingDiff, setChangesShowingDiff] = useState(false)
  // 提交信息属于当前页面：内部视图与顶层 tab 往返都保持，整页刷新才丢弃。
  const [commitMessage, setCommitMessage] = useState('')
  const [commitConfirming, setCommitConfirming] = useState(false)
  const [commitRunning, setCommitRunning] = useState(false)
  // 提交反馈只描述这一次请求的归宿；成功、明确拒绝与观察中断必须分开。
  const [commitFeedback, setCommitFeedback] = useState<
    { tone: 'success' | 'error' | 'unconfirmed'; text: string } | null
  >(null)
  // 远端检查与同步：检查是可重复的显式读取；同步只发送一次写请求，确认后才能发起。
  const [remoteChecking, setRemoteChecking] = useState(false)
  const [syncConfirming, setSyncConfirming] = useState(false)
  const [syncRunning, setSyncRunning] = useState(false)
  const [syncFeedback, setSyncFeedback] = useState<
    { tone: 'success' | 'error' | 'unconfirmed'; text: string } | null
  >(null)

  const narrow = useNarrowLayout()
  const watchGeneration = watch?.generation ?? 0
  const watchProblem = describeWatchProblem(
    watch?.connection ?? 'live',
    watch?.entry ?? null,
  )
  const gitSequence = useRef(0)
  const diffSequence = useRef(0)
  // 整树预读的代际：切换 workspace 时，旧请求返回后不得补写新树。
  const treeRun = useRef(0)
  // 同一时刻只跑一轮整树扫描；扫描期间的刷新合并为结束后的一次补跑，避免反复从头开始。
  const treeInFlight = useRef<Promise<void> | null>(null)
  const treeReloadRequested = useRef(false)
  const fileSequence = useRef(0)
  const desktopTreeRef = useRef<HTMLDivElement>(null)
  const contentRef = useRef<HTMLDivElement>(null)
  const mobileListRef = useRef<HTMLDivElement>(null)
  const changesListRef = useRef<HTMLDivElement>(null)
  const selectedChangeRef = useRef<GitChange | null>(null)
  const changesScrollPosition = useRef(0)
  const scrollPositions = useRef(new Map<string, number>())
  const activePreviously = useRef(active)
  const seenWatchGeneration = useRef(watchGeneration)
  // 异步写操作返回时只更新仍属于同一个 workspace 的页面；切换 workspace 不能把旧结果贴到新目录。
  const workspaceRef = useRef(workspace)
  workspaceRef.current = workspace

  /**
   * 一次读取完整目录树。
   *
   * 后端在 Git workspace 中已经按 Git 原生忽略规则过滤；这里只替换前端快照。
   * 同一时间只跑一次请求，扫描期间收到的刷新合并为结束后的一次补跑。
   */
  const loadEntireTree = useCallback(async () => {
    if (treeInFlight.current !== null) {
      treeReloadRequested.current = true
      return treeInFlight.current
    }

    const scanTree = async (run: number) => {
      const result = await fetchTree(workspace)
      if (treeRun.current !== run || workspaceRef.current !== workspace) return

      if (result.kind === 'ok') {
        const nextEntries = new Map<string, BrowseEntry[]>()
        const nextErrors = new Map<string, string>()
        for (const directory of result.value.directories) {
          nextEntries.set(directory.path, directory.entries)
          if (directory.error !== undefined) {
            nextErrors.set(directory.path, directory.error.message)
          }
        }
        setEntries(nextEntries)
        setDirectoryErrors(nextErrors)
        setIgnoredHidden(result.value.ignoredHidden)
      } else {
        // 根目录或 Git 忽略查询失败：不能把旧树继续显示成当前事实。
        setEntries(new Map())
        setDirectoryErrors(
          new Map([['', failureMessage(result, '完整目录树')]]),
        )
        setIgnoredHidden(false)
        setSelectedPath(null)
        setFile(null)
        setFileError(null)
        setFileLoading(false)
        setMobileShowingContent(false)
      }
    }

    const promise = (async () => {
      setTreeLoading(true)
      while (true) {
        treeReloadRequested.current = false
        const run = treeRun.current + 1
        treeRun.current = run
        await scanTree(run)
        if (treeRun.current !== run) break
        if (!treeReloadRequested.current) break
      }
    })()

    treeInFlight.current = promise
    const settle = () => {
      if (treeInFlight.current !== promise) return
      treeInFlight.current = null
      setTreeLoading(false)
    }
    void promise.then(settle, settle)
    return promise
  }, [workspace])

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
      if (!result.value.repository) {
        setView('browse')
        selectedChangeRef.current = null
        setSelectedChange(null)
        setChangeDiff(null)
        setChangeDiffError(null)
        setChangesShowingDiff(false)
      }
    } else {
      // 读取失败时清除旧状态：不能把上一次的分支或「没有变更」继续显示成当前事实。
      setGitStatus(null)
      setGitError(
        result.kind === 'rejected'
          ? result.message
          : '无法确认 Git 状态：请求没有得到可用响应。',
      )
      setView('browse')
      selectedChangeRef.current = null
      setSelectedChange(null)
      setChangeDiff(null)
      setChangeDiffError(null)
      setChangesShowingDiff(false)
    }
  }, [workspace])

  const loadChangeDiff = useCallback(
    async (change: GitChange) => {
      const sequence = diffSequence.current + 1
      diffSequence.current = sequence
      setChangeDiffLoading(true)
      setChangeDiff(null)
      setChangeDiffError(null)

      const result = await fetchChangeDiff(workspace, change.path, change.oldPath)
      if (diffSequence.current !== sequence) return

      setChangeDiffLoading(false)
      if (result.kind === 'ok') {
        setChangeDiff(result.value)
      } else {
        // 读取失败时清除旧 diff：不能把上一次的内容继续显示成当前变更。
        setChangeDiff(null)
        setChangeDiffError(failureMessage(result, `变更 ${change.path} 的 diff`))
      }
    },
    [workspace],
  )

  const refresh = useCallback(async () => {
    setRefreshing(true)
    await Promise.all([loadEntireTree(), loadGitStatus()])
    if (selectedPath !== null) await loadFile(selectedPath)
    const currentChange = selectedChangeRef.current
    if (currentChange !== null) await loadChangeDiff(currentChange)
    setRefreshing(false)
  }, [selectedPath, loadEntireTree, loadFile, loadGitStatus, loadChangeDiff])

  // 打开 workspace（或切换 workspace）时读取一次完整目录树，并清空上一份临时浏览状态。
  useEffect(() => {
    // 切换 workspace 时立即作废旧代际；新 workspace 另起一轮，不复用旧树的尾随刷新。
    treeRun.current += 1
    treeInFlight.current = null
    treeReloadRequested.current = false
    setEntries(new Map())
    setDirectoryErrors(new Map())
    setIgnoredHidden(false)
    setExpanded(new Set())
    setSelectedPath(null)
    setFile(null)
    setFileError(null)
    setMobilePath('')
    setMobileShowingContent(false)
    setGitStatus(null)
    setGitError(null)
    setView('browse')
    selectedChangeRef.current = null
    setSelectedChange(null)
    setChangeDiff(null)
    setChangeDiffError(null)
    setChangesShowingDiff(false)
    setCommitMessage('')
    setCommitConfirming(false)
    setCommitRunning(false)
    setCommitFeedback(null)
    setRemoteChecking(false)
    setSyncConfirming(false)
    setSyncRunning(false)
    setSyncFeedback(null)
    changesScrollPosition.current = 0
    scrollPositions.current = new Map()
    void loadEntireTree()
    void loadGitStatus()
  }, [workspace, loadEntireTree, loadGitStatus])

  // 从其他顶层 tab 回到文件列表时只更新 Git 状态；完整目录树仅由手动刷新重读。
  useEffect(() => {
    if (active && !activePreviously.current) {
      void loadGitStatus()
    }
    activePreviously.current = active
  }, [active, loadGitStatus])

  // 文件事件只更新 Git 状态，不自动重读目录树；需要新的目录快照时由用户按刷新。
  useEffect(() => {
    if (watchGeneration === seenWatchGeneration.current) return
    seenWatchGeneration.current = watchGeneration
    void loadGitStatus()
  }, [watchGeneration, loadGitStatus])

  // 手动刷新或忽略规则变化后，当前文件可能已不在过滤后的树中：清除选择，避免树中无此项、
  // 右侧仍显示旧内容。entries 含根节点才说明当前树的快照已经落地。
  useEffect(() => {
    if (selectedPath === null || !entries.has('')) return
    const parent = selectedPath.includes('/')
      ? selectedPath.slice(0, selectedPath.lastIndexOf('/'))
      : ''
    const siblings = entries.get(parent)
    if (siblings?.some((entry) => entry.path === selectedPath)) return
    setSelectedPath(null)
    setFile(null)
    setFileError(null)
    setFileLoading(false)
    setMobileShowingContent(false)
  }, [entries, selectedPath])

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

  // 变更列表在内部视图或窄屏 diff 往返后恢复滚动位置；列表本身不是当前事实的缓存。
  useLayoutEffect(() => {
    if (view !== 'changes') return
    if (narrow && changesShowingDiff) return
    const list = changesListRef.current
    if (list) list.scrollTop = changesScrollPosition.current
  }, [view, narrow, changesShowingDiff, gitStatus])

  // 刷新后变更集合可能已经不含所选路径：清除失效选择，避免把旧 diff 当成当前变更。
  useEffect(() => {
    if (selectedChange === null || gitStatus === null) return
    const stillPresent = gitStatus.changes.some(
      (change) => changeIdentity(change) === changeIdentity(selectedChange),
    )
    if (stillPresent) return
    diffSequence.current += 1
    selectedChangeRef.current = null
    setSelectedChange(null)
    setChangeDiff(null)
    setChangeDiffError(null)
    setChangesShowingDiff(false)
  }, [gitStatus, selectedChange])

  function rememberChangesScroll() {
    const list = changesListRef.current
    if (list) changesScrollPosition.current = list.scrollTop
  }

  function selectChange(change: GitChange) {
    rememberChangesScroll()
    selectedChangeRef.current = change
    setSelectedChange(change)
    setChangesShowingDiff(true)
    void loadChangeDiff(change)
  }

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
    setExpanded((previous) => {
      const next = new Set(previous)
      if (next.has(path)) next.delete(path)
      else next.add(path)
      return next
    })
  }

  function openMobileDirectory(path: string) {
    rememberMobileScroll()
    setMobilePath(path)
    setMobileShowingContent(false)
  }

  function goMobileUp() {
    rememberMobileScroll()
    const parent = mobilePath.includes('/')
      ? mobilePath.slice(0, mobilePath.lastIndexOf('/'))
      : ''
    setMobilePath(parent)
    setMobileShowingContent(false)
  }

  const blockedReason =
    gitStatus?.blockedReason ??
    (gitStatus?.detached
      ? '当前处于 detached HEAD：普通 Commit 不可用。'
      : gitStatus?.conflict
        ? '存在未解决的冲突文件：请在外部工具解决冲突后再试。'
        : null)
  const commitAvailable =
    gitStatus !== null &&
    gitStatus.repository &&
    !gitStatus.detached &&
    !blockedReason &&
    gitStatus.pendingCount > 0 &&
    gitStatus.reviewToken !== undefined
  const canSubmitCommit =
    commitAvailable &&
    commitMessage.trim() !== '' &&
    !commitRunning &&
    !remoteChecking &&
    !syncRunning

  // 远端事实只来自后端返回的最近已知状态；未检查、未知 ahead/behind 分别给出原因，不把缓存说成实时。
  const sync = gitStatus?.sync
  const upstream = sync?.upstream ?? null
  const remoteCheckedAt = sync?.remoteCheckedAt ?? null
  const syncCounts =
    sync !== undefined && sync.ahead !== null && sync.behind !== null
      ? { ahead: sync.ahead, behind: sync.behind }
      : null
  const worktreeClean =
    gitStatus !== null && gitStatus.repository && gitStatus.pendingCount === 0
  const checkAvailable =
    gitStatus !== null &&
    gitStatus.repository &&
    upstream !== null &&
    !remoteChecking &&
    !syncRunning
  let syncButtonLabel = 'Sync'
  let syncStateHint: string
  if (blockedReason !== null) {
    syncStateHint = blockedReason
  } else if (!worktreeClean) {
    syncStateHint = '工作区有未提交变更：Sync 只对干净工作区可用。'
  } else if (upstream === null) {
    syncStateHint =
      '当前分支没有 upstream：Sync 不可用；本功能不会配置 upstream 或发布分支。'
  } else if (remoteCheckedAt === null || syncCounts === null) {
    syncStateHint = '远端尚未检查：请先检查远端；Sync 前必定重新检查。'
  } else if (syncCounts.ahead === 0 && syncCounts.behind === 0) {
    syncButtonLabel = '已同步'
    syncStateHint = '最近已知远端状态与本地一致，当前没有需要同步的提交。'
  } else {
    syncStateHint = `待拉取 ${syncCounts.behind} 个提交，待推送 ${syncCounts.ahead} 个提交。`
  }
  const syncAvailable =
    worktreeClean &&
    upstream !== null &&
    remoteCheckedAt !== null &&
    syncCounts !== null &&
    (syncCounts.ahead > 0 || syncCounts.behind > 0) &&
    blockedReason === null &&
    !remoteChecking &&
    !syncRunning
  const syncButtonState =
    !worktreeClean ||
    upstream === null ||
    remoteCheckedAt === null ||
    syncCounts === null ||
    blockedReason !== null
      ? 'disabled'
      : syncCounts.ahead === 0 && syncCounts.behind === 0
        ? 'synced'
        : 'ready'

  /**
   * 确认后只发送一次提交请求。
   *
   * 成功、明确拒绝与观察中断分别呈现；每一种归宿结束后都重新读取当前事实，但绝不自动重发写入。
   */
  async function runCommit() {
    setCommitConfirming(false)
    const token = gitStatus?.reviewToken
    if (!commitAvailable || token === undefined || commitMessage.trim() === '') {
      setCommitFeedback({ tone: 'error', text: '提交信息不能为空。' })
      return
    }

    setCommitRunning(true)
    setCommitFeedback(null)
    const submitted = commitMessage
    const result = await commitWorkspace(workspace, submitted, token)
    setCommitRunning(false)

    if (result.kind === 'ok') {
      setCommitFeedback({
        tone: 'success',
        text: `已提交 ${result.value.commit.slice(0, 8)}：${result.value.message}`,
      })
    } else if (result.kind === 'rejected') {
      setCommitFeedback({
        tone: 'error',
        text:
          result.code === 'review-invalid'
            ? `${result.message} 提交信息已保留，请重新审查后再次确认。`
            : result.message,
      })
    } else {
      setCommitFeedback({
        tone: 'unconfirmed',
        text:
          '未确认：没有收到提交响应，提交可能已经执行；页面不会自动重试，已按实际仓库状态刷新。',
      })
    }
    // 拒绝、失败或断线后都用当前事实刷新；不能把旧审查状态继续当成结果。
    await refresh()
  }

  /**
   * 主动检查远端：只发送一次请求。成功时用服务端刚读取的事实替换当前状态，并显示真实检查时间；
   * 明确拒绝原样显示 Git 诊断；观察中断显示未确认并重新读取实际状态，绝不自动重试。
   */
  async function runCheckRemote() {
    if (!checkAvailable) return
    const target = workspace
    setRemoteChecking(true)
    setSyncFeedback(null)
    const result = await checkWorkspaceRemote(target)
    if (workspaceRef.current !== target) return
    setRemoteChecking(false)

    if (result.kind === 'ok') {
      setGitStatus(result.value)
      setGitError(null)
      const facts = result.value.sync
      setSyncFeedback({
        tone: 'success',
        text: `远端已检查：待拉取 ${facts?.behind ?? 0} 个提交，待推送 ${facts?.ahead ?? 0} 个提交。`,
      })
      return
    }
    if (result.kind === 'rejected') {
      setSyncFeedback({ tone: 'error', text: result.message })
    } else {
      setSyncFeedback({
        tone: 'unconfirmed',
        text:
          '未确认：没有收到远端检查结果；页面不会自动重试，已重新读取本地可见的实际状态。',
      })
    }
    await loadGitStatus()
  }

  /**
   * 确认后只发送一次同步请求。
   *
   * 成功、明确拒绝与观察中断分别呈现；每一种归宿结束后都重新读取当前事实，但绝不自动重发写入。
   */
  async function runSync() {
    setSyncConfirming(false)
    if (!syncAvailable) return
    const target = workspace
    setSyncRunning(true)
    setSyncFeedback(null)
    const result = await syncWorkspace(target)
    if (workspaceRef.current !== target) return
    setSyncRunning(false)

    if (result.kind === 'ok') {
      setSyncFeedback({
        tone: 'success',
        text: `已同步（${result.value.upstream.short}）：拉取 ${result.value.pulled} 个提交，推送 ${result.value.pushed} 个提交。`,
      })
    } else if (result.kind === 'rejected') {
      setSyncFeedback({ tone: 'error', text: result.message })
    } else {
      setSyncFeedback({
        tone: 'unconfirmed',
        text:
          '未确认：没有收到同步响应，同步可能已经执行；页面不会自动重试，已按实际仓库状态刷新。',
      })
    }
    // 拒绝、失败或断线后都用当前事实刷新；不能把确认前的状态继续当成结果。
    await refresh()
  }

  function commitBar(): ReactNode {
    if (gitStatus === null || !gitStatus.repository) return null
    return (
      <form
        className="commit-bar"
        data-testid="commit-bar"
        onSubmit={(event) => {
          event.preventDefault()
          if (commitMessage.trim() === '') {
            setCommitFeedback({ tone: 'error', text: '提交信息不能为空。' })
            return
          }
          if (canSubmitCommit) setCommitConfirming(true)
        }}
      >
        <label className="commit-bar__label" htmlFor="commit-message">
          提交信息
        </label>
        <div className="commit-bar__row">
          <input
            id="commit-message"
            className="commit-bar__input"
            data-testid="commit-message"
            type="text"
            value={commitMessage}
            onChange={(event) => setCommitMessage(event.target.value)}
            autoComplete="off"
            spellCheck={false}
            disabled={commitRunning || syncRunning}
          />
          <button
            type="submit"
            className="commit-bar__button"
            data-testid="commit-button"
            disabled={!canSubmitCommit}
          >
            {commitRunning ? '提交中……' : 'Commit'}
          </button>
        </div>
        <div className="sync-bar" data-testid="sync-bar">
          <div className="sync-bar__row">
            <span className="sync-bar__upstream" data-testid="sync-upstream">
              {upstream === null ? 'upstream：未配置' : `upstream：${upstream.short}`}
            </span>
            <span
              className="sync-bar__checked"
              data-testid="remote-checked-at"
              data-checked-at={remoteCheckedAt ?? undefined}
            >
              {remoteCheckedAt === null
                ? '最近检查：尚未检查'
                : `最近检查：${new Date(remoteCheckedAt).toLocaleString()}`}
            </span>
            <button
              type="button"
              className="sync-bar__button"
              data-testid="check-remote-button"
              disabled={!checkAvailable}
              onClick={() => void runCheckRemote()}
            >
              {remoteChecking ? '检查中……' : '检查远端'}
            </button>
            {worktreeClean && (
              <button
                type="button"
                className="sync-bar__button sync-bar__button--primary"
                data-testid="sync-button"
                data-state={syncButtonState}
                disabled={!syncAvailable}
                onClick={() => setSyncConfirming(true)}
              >
                {syncRunning ? '同步中……' : syncButtonLabel}
              </button>
            )}
          </div>
          <p className="sync-bar__state" data-testid="sync-state">
            {syncStateHint}
          </p>
          {syncFeedback !== null && (
            <p
              className="commit-bar__feedback"
              data-testid="sync-feedback"
              data-tone={syncFeedback.tone}
              role={syncFeedback.tone === 'success' ? 'status' : 'alert'}
            >
              {syncFeedback.text}
            </p>
          )}
        </div>
        {blockedReason !== null && (
          <p
            className="commit-bar__blocked"
            data-testid="git-blocked-reason"
            role="alert"
          >
            {blockedReason}
          </p>
        )}
        {commitAvailable && commitMessage.trim() === '' && !commitRunning && (
          <p className="commit-bar__hint" data-testid="commit-message-required">
            请输入提交信息后即可 Commit。
          </p>
        )}
        {!commitAvailable && blockedReason === null && (
          <p className="commit-bar__hint" data-testid="commit-unavailable">
            {gitStatus.pendingCount === 0
              ? '当前没有待提交变更。'
              : '当前没有可用的审查版本，无法提交。'}
          </p>
        )}
        {commitFeedback !== null && (
          <p
            className="commit-bar__feedback"
            data-testid="commit-feedback"
            data-tone={commitFeedback.tone}
            role={commitFeedback.tone === 'success' ? 'status' : 'alert'}
          >
            {commitFeedback.text}
          </p>
        )}
      </form>
    )
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

  function changesList(): ReactNode {
    if (gitStatus === null) return null
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
            {gitStatus.changes.map((change) => {
              const selected =
                selectedChange !== null &&
                changeIdentity(selectedChange) === changeIdentity(change)
              return (
                <li key={changeIdentity(change)} className="change-item">
                  <button
                    type="button"
                    className={[
                      'change-row',
                      CHANGE_CLASS[change.kind],
                      selected ? 'is-selected' : '',
                    ]
                      .filter(Boolean)
                      .join(' ')}
                    data-testid={`change:${change.path}`}
                    data-change={change.kind}
                    aria-current={selected ? 'true' : undefined}
                    aria-label={`${change.path}：${CHANGE_LABEL[change.kind]}`}
                    onClick={() => selectChange(change)}
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
                  </button>
                </li>
              )
            })}
          </ul>
        )}
      </section>
    )
  }

  function diffBody(diff: ChangeDiff): ReactNode {
    const oldText = diff.old.kind === 'text' ? diff.old.content : ''
    const newText = diff.new.kind === 'text' ? diff.new.content : ''
    const rows = diffRows(oldText, newText)
    if (rows.length === 0) {
      return (
        <p className="browser-empty" data-testid="diff-empty">
          两侧内容相同，没有行级差异。
        </p>
      )
    }

    if (narrow) {
      return (
        <div className="diff-lines" data-testid="diff-view" data-mode="unified">
          {rows.map((row, index) => (
            <div
              key={`${index}:${row.kind}`}
              className={`diff-line diff-line--${row.kind}`}
              data-testid={`diff-row-${row.kind}`}
            >
              <span className="diff-line__number" aria-hidden="true">
                {row.oldNumber ?? ''}
              </span>
              <span className="diff-line__number" aria-hidden="true">
                {row.newNumber ?? ''}
              </span>
              <span className="diff-line__marker" aria-hidden="true">
                {row.kind === 'added' ? '+' : row.kind === 'removed' ? '-' : ' '}
              </span>
              <span className="diff-line__text">{row.text}</span>
            </div>
          ))}
        </div>
      )
    }

    return (
      <div className="diff-side-by-side" data-testid="diff-view" data-mode="side-by-side">
        {sideBySideRows(rows).map((pair, index) => (
          <div key={index} className="diff-pair">
            <div
              className={`diff-cell diff-cell--${pair.old.kind}`}
              data-testid={`diff-old-${pair.old.kind}`}
            >
              <span className="diff-line__number" aria-hidden="true">
                {pair.old.number ?? ''}
              </span>
              <span className="diff-line__text" data-testid="diff-old-text">
                {pair.old.text}
              </span>
            </div>
            <div
              className={`diff-cell diff-cell--${pair.new.kind}`}
              data-testid={`diff-new-${pair.new.kind}`}
            >
              <span className="diff-line__number" aria-hidden="true">
                {pair.new.number ?? ''}
              </span>
              <span className="diff-line__text" data-testid="diff-new-text">
                {pair.new.text}
              </span>
            </div>
          </div>
        ))}
      </div>
    )
  }

  function diffPane(): ReactNode {
    if (selectedChange === null) {
      return (
        <p className="browser-placeholder" data-testid="diff-placeholder">
          选择变更以审查从最新本地提交到最终本地内容的差异。
        </p>
      )
    }
    if (changeDiffLoading) {
      return (
        <p className="browser-loading" data-testid="diff-loading" role="status">
          正在读取 diff……
        </p>
      )
    }
    if (changeDiffError !== null) {
      return (
        <p className="browser-error" data-testid="diff-error" role="alert">
          {changeDiffError}
        </p>
      )
    }
    if (changeDiff === null) return null

    return (
      <section className="diff" data-testid="diff-pane">
        <header className="file-view__header">
          <code className="file-view__path" data-testid="diff-path">
            {changeDiff.oldPath !== undefined ? (
              <>
                <span className="change-row__old">{changeDiff.oldPath}</span>
                <span aria-hidden="true"> → </span>
                <span>{changeDiff.path}</span>
              </>
            ) : (
              changeDiff.path
            )}
          </code>
          <span className="file-view__size" data-testid="diff-basis">
            最新本地提交 → 最终本地内容
          </span>
        </header>
        {changeDiff.complete ? (
          diffBody(changeDiff)
        ) : (
          <p className="file-view__limited" data-testid="diff-unavailable" role="note">
            {diffUnavailableReason(changeDiff)}
          </p>
        )}
      </section>
    )
  }

  function changesView(): ReactNode {
    if (gitStatus === null) {
      return (
        <p className="browser-loading" data-testid="git-status-loading" role="status">
          正在读取 Git 状态……
        </p>
      )
    }
    if (!gitStatus.repository) return null

    if (narrow) {
      if (changesShowingDiff && selectedChange !== null) {
        return (
          <div className="browser-mobile" data-testid="changes-mobile">
            {commitBar()}
            <button
              type="button"
              className="browser-mobile__back"
              data-testid="back-to-changes-list"
              onClick={() => setChangesShowingDiff(false)}
            >
              返回变更列表
            </button>
            {diffPane()}
          </div>
        )
      }
      return (
        <div className="browser-mobile" data-testid="changes-mobile">
          {commitBar()}
          <div
            className="browser-mobile__list"
            data-testid="changes-list-scroll"
            ref={changesListRef}
            onScroll={rememberChangesScroll}
          >
            {changesList()}
          </div>
        </div>
      )
    }

    return (
      <>
        {commitBar()}
        <div className="changes-layout" data-testid="changes-layout">
          <div
            className="changes-list"
            data-testid="changes-list-scroll"
            ref={changesListRef}
            onScroll={rememberChangesScroll}
          >
            {changesList()}
          </div>
          <div className="changes-detail" data-testid="changes-detail">
            {diffPane()}
          </div>
        </div>
      </>
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
    if (entries.get(path) === undefined) return null
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
    if (directoryEntries === undefined) return null
    if (directoryEntries.length === 0) {
      return (
        <p
          className="browser-empty"
          data-testid={path === '' ? 'directory-empty' : `directory-empty:${path}`}
        >
          {ignoredHidden ? '此目录没有未忽略的内容。' : '此目录为空。'}
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
          {ignoredHidden ? '此目录没有未忽略的内容。' : '此目录为空。'}
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

      {gitStatus?.repository && blockedReason !== null && (
        <p
          className="browser-error"
          data-testid="git-blocked-reason-summary"
          role="alert"
        >
          {blockedReason}
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

      {/* 浏览与变更两个内部视图都保持挂载，只切换可见性：切走再切回时选择、展开与滚动位置
          仍由同一个组件实例持有，不因重新挂载而丢失。 */}
      <div className="browser-pane" hidden={view !== 'browse'}>
        {treeLoading && (
          <p className="browser-loading" data-testid="tree-loading" role="status">
            正在加载完整目录树……
          </p>
        )}
        {ignoredHidden && !treeLoading && (
          <p className="browser-note" data-testid="ignored-hidden-notice">
            已按 Git 忽略规则隐藏被忽略的文件和目录。
          </p>
        )}
        {narrow ? (
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
      </div>

      {gitStatus?.repository && (
        <div className="browser-pane" hidden={view !== 'changes'}>
          {changesView()}
        </div>
      )}

      {/*
        确认门禁只有一句消息与确认/取消：workspace、分支、文件数、提交信息与清单已经由
        变更界面承载，不在弹框里重复。取消只关闭门禁，不发送任何写请求。
      */}
      {commitConfirming && (
        <div
          className="commit-confirm"
          data-testid="commit-confirm-dialog"
          role="dialog"
          aria-modal="true"
          aria-label="确认提交"
        >
          <p className="commit-confirm__question">
            确认提交当前审查版本中的全部待提交变更？
          </p>
          <div className="commit-confirm__actions">
            <button
              type="button"
              className="commit-confirm__cancel"
              data-testid="commit-cancel"
              onClick={() => setCommitConfirming(false)}
            >
              取消
            </button>
            <button
              type="button"
              className="commit-confirm__go"
              data-testid="commit-confirm"
              onClick={() => void runCommit()}
            >
              确认提交
            </button>
          </div>
        </div>
      )}

      {/* Sync 的确认门禁同样只有一句消息；当前分支、upstream 与数量已由变更界面承载。 */}
      {syncConfirming && (
        <div
          className="commit-confirm"
          data-testid="sync-confirm-dialog"
          role="dialog"
          aria-modal="true"
          aria-label="确认同步"
        >
          <p className="commit-confirm__question">
            确认按当前分支的既有 upstream 检查远端、拉取并在需要时推送？
          </p>
          <div className="commit-confirm__actions">
            <button
              type="button"
              className="commit-confirm__cancel"
              data-testid="sync-cancel"
              onClick={() => setSyncConfirming(false)}
            >
              取消
            </button>
            <button
              type="button"
              className="commit-confirm__go"
              data-testid="sync-confirm"
              onClick={() => void runSync()}
            >
              确认同步
            </button>
          </div>
        </div>
      )}
    </section>
  )
}
