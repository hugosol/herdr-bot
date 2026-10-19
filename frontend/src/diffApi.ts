/**
 * 变更审查 diff 接口客户端。
 *
 * 旧侧与新的完整文本都由后端给出；超限或二进制时后端只给大小与原因，页面不会自行猜测完整内容。
 */

export type DiffSide =
  | { kind: 'absent' }
  | { kind: 'text'; content: string; size: number }
  | { kind: 'binary'; reason: string; size: number }
  | { kind: 'too-large'; reason: string; size: number; limit: number }

/** 一次变更审查：旧侧是 HEAD，新侧是最终本地内容。 */
export type ChangeDiff = {
  workspace: string
  path: string
  oldPath?: string
  old: DiffSide
  new: DiffSide
  complete: boolean
}

/**
 * 一次 diff 请求在页面一侧的归宿。
 *
 * - `ok`：后端返回了形状正确的旧新两侧事实。
 * - `rejected`：后端给出明确失败类别（越界、链接、不存在、Git 失败等）。
 * - `unreachable`：没有得到可用响应；不能当成空 diff 或相同内容。
 */
export type DiffResult<T> =
  | { kind: 'ok'; value: T }
  | { kind: 'rejected'; code: string; message: string }
  | { kind: 'unreachable' }

function isDiffSide(value: unknown): value is DiffSide {
  if (typeof value !== 'object' || value === null || !('kind' in value)) return false
  switch (value.kind) {
    case 'absent':
      return true
    case 'text':
      return (
        'content' in value &&
        typeof value.content === 'string' &&
        'size' in value &&
        typeof value.size === 'number'
      )
    case 'binary':
      return (
        'reason' in value &&
        typeof value.reason === 'string' &&
        'size' in value &&
        typeof value.size === 'number'
      )
    case 'too-large':
      return (
        'reason' in value &&
        typeof value.reason === 'string' &&
        'size' in value &&
        typeof value.size === 'number' &&
        'limit' in value &&
        typeof value.limit === 'number'
      )
    default:
      return false
  }
}

function isChangeDiff(value: unknown): value is ChangeDiff {
  if (typeof value !== 'object' || value === null) return false
  if (!('workspace' in value) || typeof value.workspace !== 'string') return false
  if (!('path' in value) || typeof value.path !== 'string') return false
  if ('oldPath' in value && value.oldPath !== undefined && typeof value.oldPath !== 'string') {
    return false
  }
  if (!('complete' in value) || typeof value.complete !== 'boolean') return false
  return (
    'old' in value &&
    'new' in value &&
    isDiffSide(value.old) &&
    isDiffSide(value.new)
  )
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

async function diffRequest<T>(
  url: string,
  accept: (payload: unknown) => payload is T,
): Promise<DiffResult<T>> {
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
    return accept(payload) ? { kind: 'ok', value: payload } : { kind: 'unreachable' }
  }
  return isErrorBody(payload)
    ? { kind: 'rejected', code: payload.code, message: payload.message }
    : { kind: 'unreachable' }
}

/** 读取一个变更从 HEAD 到最终本地内容的 diff；重命名时带上 Git 的旧路径。 */
export function fetchChangeDiff(
  workspace: string,
  path: string,
  oldPath?: string,
): Promise<DiffResult<ChangeDiff>> {
  const params = new URLSearchParams({ workspace, path })
  if (oldPath !== undefined) params.set('oldPath', oldPath)
  return diffRequest(`/api/workspaces/diff?${params.toString()}`, isChangeDiff)
}
