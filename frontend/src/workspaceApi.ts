/**
 * workspace 登记接口客户端：页面与接口同源，请求地址直接使用相对路径。
 *
 * 登记列表由后端持久保存，电脑与手机读取同一份；页面不缓存列表，也不使用客户端文件选择器。
 */

/** 一个已登记的 workspace：服务器上的绝对目录路径。 */
export type WorkspaceRegistration = {
  path: string
}

/**
 * 一次 workspace 请求在页面一侧的可能归宿。
 *
 * - `ok`：后端返回了当前完整列表；只有这时才能更新界面或显示成功。
 * - `rejected`：后端给出了明确的失败类别与说明（非法路径、存储失败等）。
 * - `unreachable`：没有得到可用响应（网络中断或响应无法解析），不能当成成功或失败。
 */
export type WorkspaceResult =
  | { kind: 'ok'; workspaces: WorkspaceRegistration[] }
  | { kind: 'rejected'; code: string; message: string }
  | { kind: 'unreachable' }

/** 单个登记项的形状检查。 */
function isWorkspaceRegistration(value: unknown): value is WorkspaceRegistration {
  return (
    typeof value === 'object' &&
    value !== null &&
    'path' in value &&
    typeof value.path === 'string'
  )
}

/** 列表响应的形状检查：形状不对的响应不能被当作成功。 */
function isWorkspaceList(value: unknown): value is { workspaces: WorkspaceRegistration[] } {
  if (typeof value !== 'object' || value === null || !('workspaces' in value)) {
    return false
  }
  return Array.isArray(value.workspaces) && value.workspaces.every(isWorkspaceRegistration)
}

/** 明确失败响应的形状检查。 */
function isWorkspaceError(value: unknown): value is { code: string; message: string } {
  return (
    typeof value === 'object' &&
    value !== null &&
    'code' in value &&
    typeof value.code === 'string' &&
    'message' in value &&
    typeof value.message === 'string'
  )
}

/**
 * 发出一次 workspace 请求并归一化答复。
 *
 * 只有 HTTP 成功且响应形状正确才算 `ok`；明确的错误码与说明原样交给界面；
 * 其余情形（网络中断、非 JSON、形状不符）一律是 `unreachable`，不能显示成成功。
 */
async function workspaceRequest(
  input: RequestInfo | URL,
  init?: RequestInit,
): Promise<WorkspaceResult> {
  let response: Response
  try {
    response = await fetch(input, init)
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
    return isWorkspaceList(payload)
      ? { kind: 'ok', workspaces: payload.workspaces }
      : { kind: 'unreachable' }
  }

  return isWorkspaceError(payload)
    ? { kind: 'rejected', code: payload.code, message: payload.message }
    : { kind: 'unreachable' }
}

/** 读取当前登记列表；每次进入文件列表都重新读取后端，不使用页面缓存。 */
export function fetchWorkspaces(): Promise<WorkspaceResult> {
  return workspaceRequest('/api/workspaces')
}

/** 登记一个服务器目录；成功只意味着后端已经持久化。 */
export function registerWorkspace(path: string): Promise<WorkspaceResult> {
  return workspaceRequest('/api/workspaces', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ path }),
  })
}

/** 取消登记；只移除登记项，不删除目录或文件。 */
export function unregisterWorkspace(path: string): Promise<WorkspaceResult> {
  return workspaceRequest('/api/workspaces/remove', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ path }),
  })
}
