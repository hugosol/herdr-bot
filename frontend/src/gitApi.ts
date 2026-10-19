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

/** 一个 workspace 的 Git 状态；普通目录是 `repository: false`。 */
export type GitStatus = {
  workspace: string
  repository: boolean
  branch: string | null
  detached: boolean
  changes: GitChange[]
  conflict: boolean
  pendingCount: number
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
  // 项数必须与集合一致；不一致的答复不能冒充可用状态。
  return value.pendingCount === value.changes.length
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
): Promise<GitResult<T>> {
  let response: Response
  try {
    response = await fetch(url)
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
