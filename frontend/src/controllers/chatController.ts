/**
 * Chat controller：拥有创建、历史、备忘与恢复请求；页面保留状态、竞态和展示。
 * 页面与接口同源，请求地址直接使用相对路径。
 *
 * 后端预览提供 Task 命名规则与真实上下文，页面本地预览本次规则编辑，创建时由后端再次校验名称。
 * 创建请求返回后端拥有的生命周期，页面只轮询同一次创建，不把“请求已发出”或“启动已排队”当成成功。
 */

import type { TaskModel, TaskThinking } from './taskController'

export type ChatCreationStage = 'starting' | 'submitting' | 'succeeded' | 'failed'
export type ChatFailureStage = 'workspace' | 'tab' | 'start' | 'submit'

export type ChatPreview = {
  chatId: string
  name: string
  zone: string
  taskId: string
  agent: string
  chatNameRule: string
  taskName: string
  zoneName: string
}

export type ChatCreationFailure = {
  stage: ChatFailureStage
  code: string
  message: string
}

export type ChatCreation = {
  id: string
  name: string
  zone: string
  taskId: string
  stage: ChatCreationStage
  tabId?: string
  paneId?: string
  agentName?: string
  inputSubmitted: boolean
  failure?: ChatCreationFailure
}

/**
 * 一次 Chat 请求在页面一侧的可能归宿。
 *
 * - `ok`：后端返回了可理解的响应。
 * - `rejected`：后端给出明确类别与说明。
 * - `unreachable`：没有得到可用响应；不能当成成功或失败。
 */
export type ChatResult<T> =
  | { kind: 'ok'; value: T }
  | { kind: 'rejected'; code: string; message: string }
  | { kind: 'unreachable' }

function isString(value: unknown): value is string {
  return typeof value === 'string'
}

function isNullableString(value: unknown): value is string | undefined {
  return value === undefined || typeof value === 'string'
}

function isPreview(value: unknown): value is ChatPreview {
  if (typeof value !== 'object' || value === null) return false
  const preview = value as Record<string, unknown>
  return (
    isString(preview.chatId) &&
    isString(preview.name) &&
    isString(preview.zone) &&
    isString(preview.taskId) &&
    isString(preview.agent) &&
    isString(preview.chatNameRule) &&
    isString(preview.taskName) &&
    isString(preview.zoneName)
  )
}

const STAGES: ChatCreationStage[] = ['starting', 'submitting', 'succeeded', 'failed']
const FAILURE_STAGES: ChatFailureStage[] = ['workspace', 'tab', 'start', 'submit']

function isCreation(value: unknown): value is ChatCreation {
  if (typeof value !== 'object' || value === null) return false
  const creation = value as Record<string, unknown>
  const failure = creation.failure
  const failureShape =
    failure === undefined ||
    (typeof failure === 'object' &&
      failure !== null &&
      isString((failure as Record<string, unknown>).stage) &&
      FAILURE_STAGES.includes((failure as Record<string, unknown>).stage as ChatFailureStage) &&
      isString((failure as Record<string, unknown>).code) &&
      isString((failure as Record<string, unknown>).message))
  return (
    isString(creation.id) &&
    isString(creation.name) &&
    isString(creation.zone) &&
    isString(creation.taskId) &&
    isString(creation.stage) &&
    STAGES.includes(creation.stage as ChatCreationStage) &&
    isNullableString(creation.tabId) &&
    isNullableString(creation.paneId) &&
    isNullableString(creation.agentName) &&
    typeof creation.inputSubmitted === 'boolean' &&
    failureShape
  )
}

function isErrorBody(value: unknown): value is { code: string; message: string } {
  if (typeof value !== 'object' || value === null) return false
  const error = value as Record<string, unknown>
  return isString(error.code) && isString(error.message)
}

async function request<T>(
  input: RequestInfo | URL,
  init: RequestInit | undefined,
  valid: (value: unknown) => value is T,
): Promise<ChatResult<T>> {
  let response: Response
  try {
    response = await fetch(input, init)
  } catch {
    return { kind: 'unreachable' }
  }

  let body: unknown
  try {
    body = await response.json()
  } catch {
    return { kind: 'unreachable' }
  }

  if (response.ok && valid(body)) {
    return { kind: 'ok', value: body }
  }
  if (!response.ok && isErrorBody(body)) {
    return { kind: 'rejected', code: body.code, message: body.message }
  }
  return { kind: 'unreachable' }
}

function jsonInit(method: string, body: unknown): RequestInit {
  return {
    method,
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  }
}

/** zone 为稳定 ID；预览与创建结果中的 zone 仍为启动时解析的实际目录。读取预览不创建 Tab 或启动 agent。 */
export function previewChat(zone: string, taskId: string): Promise<ChatResult<ChatPreview>> {
  return request('/api/chats/preview', jsonInit('POST', { zone, taskId }), isPreview)
}

/** 接受一次创建；响应只表示后端已经接手，后续阶段由 `fetchChatCreation` 观察。 */
export function createChat(input: {
  zone: string
  taskId: string
  chatId: string
  expectedName: string
  chatNameRule?: string
  input: string
  workspaceTarget?: 'default' | 'current'
  model?: TaskModel
  thinking?: TaskThinking
}): Promise<ChatResult<ChatCreation>> {
  return request('/api/chats', jsonInit('POST', input), isCreation)
}

/** 读取同一次创建当前阶段；成功只在后端确认就绪与提交之后出现。 */
export function fetchChatCreation(id: string): Promise<ChatResult<ChatCreation>> {
  return request(`/api/chats/${encodeURIComponent(id)}`, undefined, isCreation)
}

export type HistorySession = {
  sessionId: string
  sessionName: string | null
  agentType: string | null
  cwd: string | null
  sessionDirectory?: string
  originalWorkspaceId?: string
  originalWorkspaceName?: string
  memo?: string
  firstRecordedAt: number
}

export type HistoryPage = { sessions: HistorySession[]; page: number; total: number; totalPages: number }

export async function fetchChatHistory(page: number, signal: AbortSignal): Promise<HistoryPage> {
  const response = await fetch(`/api/sessions/history?page=${page}`, { signal, cache: 'no-store' })
  if (!response.ok) throw new Error(`HTTP ${response.status}`)
  return await response.json() as HistoryPage
}

export async function saveChatMemo(sessionId: string, memo: string): Promise<string> {
  const response = await fetch('/api/sessions/history/memo', {
    method: 'PUT', headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ sessionId, memo }),
  })
  if (!response.ok) throw new Error(`HTTP ${response.status}`)
  const body: unknown = await response.json()
  if (typeof body !== 'object' || body === null || !('sessionId' in body) || body.sessionId !== sessionId || !('memo' in body) || typeof body.memo !== 'string') {
    throw new Error('unconfirmed memo response')
  }
  return body.memo
}

// Both callers share transport, but retain their existing response acceptance rules.
async function requestChatResume(sessionId: string): Promise<{ ok: boolean; body: unknown }> {
  const response = await fetch('/api/chats/resume', {
    method: 'POST', headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ sessionId }),
  })
  return { ok: response.ok, body: await response.json() }
}

export type ChatResumeResult = {
  failed: boolean
  message: string
  located: boolean
  target: Partial<Record<'workspaceId' | 'tabId' | 'paneId', string>>
  agentName?: string
}

export async function resumeChat(sessionId: string): Promise<ChatResumeResult> {
  const { ok, body } = await requestChatResume(sessionId)
  if (typeof body !== 'object' || body === null || !('message' in body) || typeof body.message !== 'string') {
    throw new Error('unconfirmed response')
  }
  if (ok && (!('outcome' in body) || !['located', 'restored'].includes(String(body.outcome)))) {
    throw new Error('unconfirmed outcome')
  }
  const located = ok && 'outcome' in body && body.outcome === 'located'
  const target: ChatResumeResult['target'] = {}
  if (!located && 'target' in body && typeof body.target === 'object' && body.target !== null) {
    for (const key of ['workspaceId', 'tabId', 'paneId'] as const) {
      if (key in body.target) {
        const value = (body.target as Record<string, unknown>)[key]
        if (typeof value === 'string' && value.length > 0) target[key] = value
      }
    }
  }
  const agentName = !located && 'agentName' in body && typeof body.agentName === 'string' && body.agentName.length > 0
    ? body.agentName : undefined
  return { failed: !ok, message: body.message, located, target, agentName }
}

export type ChiefOpenResult =
  | { kind: 'absent' }
  | { kind: 'starting' }
  | { kind: 'resumed'; failed: boolean; message: string }

/** Resolve on every click; a different browser may have changed chief authority. */
export async function openChiefChat(): Promise<ChiefOpenResult> {
  const response = await fetch('/api/chief', { cache: 'no-store' })
  if (!response.ok) throw new Error('unconfirmed authority')
  const body = await response.json()
  if (!body || !Object.hasOwn(body, 'current')) throw new Error('unconfirmed authority')
  if (body.current === null) return { kind: 'absent' }
  if (typeof body.current.sessionId === 'string') {
    const { ok, body: result } = await requestChatResume(body.current.sessionId)
    if (typeof result !== 'object' || result === null || !('message' in result) || typeof result.message !== 'string' || (ok && (!('outcome' in result) || !['located', 'restored'].includes(result.outcome as string)))) {
      throw new Error('unconfirmed navigation')
    }
    return { kind: 'resumed', failed: !ok, message: result.message }
  }
  if (typeof body.current.phase === 'string') return { kind: 'starting' }
  throw new Error('unconfirmed authority')
}
