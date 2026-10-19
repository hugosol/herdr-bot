/**
 * workspace Git 状态接口客户端。
 *
 * 只读取当前分支与待提交变更；页面不自行判断 Git 规则，也不为了展示暂存或配对文件。
 * 读取失败必须显示为明确错误，不能当作「没有待提交变更」。
 */

export type ChangeKind = 'added' | 'modified' | 'deleted' | 'renamed' | 'conflicted'

/** 待提交变更的一项；重命名带 Git 给出的旧路径。 */
export type GitChange = {
  path: string
  oldPath?: string
  kind: ChangeKind
}

/** 当前分支已有的 upstream；`short` 是 Git 的简短写法（如 `upstream/feature`）。 */
export type UpstreamRef = {
  remote: string
  branch: string
  short: string
}

/**
 * 远端同步事实。
 *
 * `ahead`/`behind` 来自最近已知的本地远端跟踪引用；`remoteCheckedAt` 是服务端最后一次成功检查
 * 远端的时刻（Unix 毫秒）。`null` 表示尚未检查，页面不能把它当成实时或已同步。
 */
export type SyncFacts = {
  upstream: UpstreamRef | null
  ahead: number | null
  behind: number | null
  remoteCheckedAt: number | null
}

/** 一个 workspace 的 Git 状态；普通目录是 `repository: false`。 */
export type GitStatus = {
  workspace: string
  repository: boolean
  branch: string | null
  detached: boolean
  changes: GitChange[]
  conflict: boolean
  pendingCount: number
  /** 本次展示的审查版本；Commit 必须原样带回它。 */
  reviewToken?: string
  /** 仓库级阻塞原因；存在时 Commit 不可用。 */
  blockedReason?: string | null
  /** 远端同步事实；缺失时按“远端尚未检查”处理。 */
  sync?: SyncFacts
}

/** 一次已确认提交的回执。 */
export type CommitAccepted = {
  workspace: string
  outcome: 'committed'
  commit: string
  branch?: string | null
  message: string
}

/**
 * 一次提交请求在页面一侧的归宿。
 *
 * - `ok`：后端确认创建了提交。
 * - `rejected`：后端明确拒绝；没有提交或提交失败，错误类别与 Git 诊断原样显示。
 * - `unreachable`：观察中断；不能当成成功，也不能当成未执行。
 */
export type CommitResult =
  | { kind: 'ok'; value: CommitAccepted }
  | { kind: 'rejected'; code: string; message: string }
  | { kind: 'unreachable' }

/** 一次成功同步的回执；`pulled`/`pushed` 是本次实际整合与推送的提交数。 */
export type SyncAccepted = {
  workspace: string
  outcome: 'synced'
  branch: string
  upstream: UpstreamRef
  ahead: number
  behind: number
  remoteCheckedAt: number
  pulled: number
  pushed: number
  steps: { step: string; outcome: string }[]
}

/** 文件列表上的角标概览；不可访问的项带 `error`，普通目录没有待提交数量。 */
export type WorkspaceSummary = {
  path: string
  repository: boolean
  branch?: string | null
  pendingCount?: number
  conflict: boolean
  error?: string
}

/**
 * 一次 Git 请求在页面一侧的归宿。
 *
 * - `ok`：后端返回了形状正确的结果。
 * - `rejected`：后端给出明确失败类别与说明。
 * - `unreachable`：没有得到可用响应；不能当成干净仓库或零变更。
 */
export type GitResult<T> =
  | { kind: 'ok'; value: T }
  | { kind: 'rejected'; code: string; message: string }
  | { kind: 'unreachable' }

const CHANGE_KINDS: readonly ChangeKind[] = [
  'added',
  'modified',
  'deleted',
  'renamed',
  'conflicted',
]

function isChangeKind(value: unknown): value is ChangeKind {
  return CHANGE_KINDS.some((kind) => kind === value)
}

function isGitChange(value: unknown): value is GitChange {
  if (typeof value !== 'object' || value === null) return false
  if (!('path' in value) || typeof value.path !== 'string') return false
  if (!('kind' in value) || !isChangeKind(value.kind)) return false
  if ('oldPath' in value && value.oldPath !== undefined && typeof value.oldPath !== 'string') {
    return false
  }
  return true
}

function isUpstreamRef(value: unknown): value is UpstreamRef {
  if (typeof value !== 'object' || value === null) return false
  if (!('remote' in value) || typeof value.remote !== 'string') return false
  if (!('branch' in value) || typeof value.branch !== 'string') return false
  if (!('short' in value) || typeof value.short !== 'string') return false
  return true
}

function isOptionalCount(value: unknown): value is number | null {
  return value === null || typeof value === 'number'
}

function isSyncFacts(value: unknown): value is SyncFacts {
  if (typeof value !== 'object' || value === null) return false
  if (!('upstream' in value)) return false
  if (value.upstream !== null && !isUpstreamRef(value.upstream)) return false
  if (!('ahead' in value) || !isOptionalCount(value.ahead)) return false
  if (!('behind' in value) || !isOptionalCount(value.behind)) return false
  if (!('remoteCheckedAt' in value) || !isOptionalCount(value.remoteCheckedAt)) {
    return false
  }
  return true
}

function isGitStatus(value: unknown): value is GitStatus {
  if (typeof value !== 'object' || value === null) return false
  if (!('workspace' in value) || typeof value.workspace !== 'string') return false
  if (!('repository' in value) || typeof value.repository !== 'boolean') return false
  if (!('branch' in value)) return false
  if (value.branch !== null && typeof value.branch !== 'string') return false
  if (!('detached' in value) || typeof value.detached !== 'boolean') return false
  if (!('changes' in value) || !Array.isArray(value.changes)) return false
  if (!value.changes.every(isGitChange)) return false
  if (!('conflict' in value) || typeof value.conflict !== 'boolean') return false
  if (!('pendingCount' in value) || typeof value.pendingCount !== 'number') return false
  if (
    'reviewToken' in value &&
    value.reviewToken !== undefined &&
    typeof value.reviewToken !== 'string'
  ) {
    return false
  }
  if (
    'blockedReason' in value &&
    value.blockedReason !== undefined &&
    value.blockedReason !== null &&
    typeof value.blockedReason !== 'string'
  ) {
    return false
  }
  if ('sync' in value && value.sync !== undefined && !isSyncFacts(value.sync)) {
    return false
  }
  // 项数必须与集合一致；不一致的答复不能冒充可用状态。
  return value.pendingCount === value.changes.length
}

function isCommitAccepted(value: unknown): value is CommitAccepted {
  if (typeof value !== 'object' || value === null) return false
  if (!('workspace' in value) || typeof value.workspace !== 'string') return false
  if (!('outcome' in value) || value.outcome !== 'committed') return false
  if (!('commit' in value) || typeof value.commit !== 'string') return false
  if (!('message' in value) || typeof value.message !== 'string') return false
  if ('branch' in value && value.branch !== undefined && value.branch !== null) {
    if (typeof value.branch !== 'string') return false
  }
  return true
}

function isSyncAccepted(value: unknown): value is SyncAccepted {
  if (typeof value !== 'object' || value === null) return false
  if (!('workspace' in value) || typeof value.workspace !== 'string') return false
  if (!('outcome' in value) || value.outcome !== 'synced') return false
  if (!('branch' in value) || typeof value.branch !== 'string') return false
  if (!('upstream' in value) || !isUpstreamRef(value.upstream)) return false
  if (!('ahead' in value) || typeof value.ahead !== 'number') return false
  if (!('behind' in value) || typeof value.behind !== 'number') return false
  if (!('remoteCheckedAt' in value) || typeof value.remoteCheckedAt !== 'number') {
    return false
  }
  if (!('pulled' in value) || typeof value.pulled !== 'number') return false
  if (!('pushed' in value) || typeof value.pushed !== 'number') return false
  if (!('steps' in value) || !Array.isArray(value.steps)) return false
  return value.steps.every(
    (step) =>
      typeof step === 'object' &&
      step !== null &&
      'step' in step &&
      typeof step.step === 'string' &&
      'outcome' in step &&
      typeof step.outcome === 'string',
  )
}

function isWorkspaceSummary(value: unknown): value is WorkspaceSummary {
  if (typeof value !== 'object' || value === null) return false
  if (!('path' in value) || typeof value.path !== 'string') return false
  if (!('repository' in value) || typeof value.repository !== 'boolean') return false
  if (!('conflict' in value) || typeof value.conflict !== 'boolean') return false
  if ('branch' in value && value.branch !== undefined && value.branch !== null) {
    if (typeof value.branch !== 'string') return false
  }
  if (
    'pendingCount' in value &&
    value.pendingCount !== undefined &&
    typeof value.pendingCount !== 'number'
  ) {
    return false
  }
  if ('error' in value && value.error !== undefined && typeof value.error !== 'string') {
    return false
  }
  return true
}

/** 概览响应是带 `workspaces` 数组的对象；形状不符时返回 null，而不是猜一个空列表。 */
function parseOverview(payload: unknown): WorkspaceSummary[] | null {
  if (typeof payload !== 'object' || payload === null) return null
  if (!('workspaces' in payload)) return null
  const workspaces = payload.workspaces
  if (!Array.isArray(workspaces) || !workspaces.every(isWorkspaceSummary)) return null
  return workspaces
}

function isErrorBody(value: unknown): value is { code: string; message: string } {
  return (
    typeof value === 'object' &&
    value !== null &&
    'code' in value &&
    typeof value.code === 'string' &&
    'message' in value &&
    typeof value.message === 'string'
  )
}

async function gitRequest<T>(
  url: string,
  parse: (payload: unknown) => T | null,
  init?: RequestInit,
): Promise<GitResult<T>> {
  let response: Response
  try {
    response = await fetch(url, init)
  } catch {
    return { kind: 'unreachable' }
  }

  let payload: unknown
  try {
    payload = await response.json()
  } catch {
    payload = null
  }

  if (response.ok) {
    const value = parse(payload)
    return value === null ? { kind: 'unreachable' } : { kind: 'ok', value }
  }
  return isErrorBody(payload)
    ? { kind: 'rejected', code: payload.code, message: payload.message }
    : { kind: 'unreachable' }
}

/** 读取一个已登记 workspace 的当前分支与待提交变更。 */
export function fetchGitStatus(workspace: string): Promise<GitResult<GitStatus>> {
  const params = new URLSearchParams({ workspace })
  return gitRequest(`/api/workspaces/git-status?${params.toString()}`, (payload) =>
    isGitStatus(payload) ? payload : null,
  )
}

/** 读取登记列表的角标概览。 */
export function fetchWorkspaceOverview(): Promise<GitResult<WorkspaceSummary[]>> {
  return gitRequest('/api/workspaces/overview', parseOverview)
}

/**
 * 提交页面当前展示的审查版本。
 *
 * 只发送一次 POST；网络中断返回 `unreachable`，由调用方查询实际仓库状态而不是自动重试。
 */
export function commitWorkspace(
  workspace: string,
  message: string,
  reviewToken: string,
): Promise<GitResult<CommitAccepted>> {
  return gitRequest(
    '/api/workspaces/commit',
    (payload) => (isCommitAccepted(payload) ? payload : null),
    {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ workspace, message, reviewToken }),
    },
  )
}

/**
 * 主动检查当前分支的既有 upstream：服务端真实 fetch 并返回刷新后的完整 Git 状态。
 *
 * 只发送一次 POST；网络中断返回 `unreachable`，由调用方按实际状态重新读取，不自动重试。
 */
export function checkWorkspaceRemote(workspace: string): Promise<GitResult<GitStatus>> {
  return gitRequest(
    '/api/workspaces/check-remote',
    (payload) => (isGitStatus(payload) ? payload : null),
    {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ workspace }),
    },
  )
}

/**
 * 按既有 upstream 同步：服务端重新检查远端，再 pull 并按需 push。
 *
 * 页面不提供分支或远端参数；执行目标由服务端按当前分支的既有配置决定。
 */
export function syncWorkspace(workspace: string): Promise<GitResult<SyncAccepted>> {
  return gitRequest(
    '/api/workspaces/sync',
    (payload) => (isSyncAccepted(payload) ? payload : null),
    {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ workspace }),
    },
  )
}
