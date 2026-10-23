/**
 * 从 zone 创建 Bot 的接口客户端：页面与接口同源，请求地址直接使用相对路径。
 *
 * 后端预览提供模板命名规则与真实上下文，页面本地预览本次规则编辑，创建时由后端再次校验名称。
 * 创建请求返回后端拥有的生命周期，页面只轮询同一次创建，不把“请求已发出”或“启动已排队”当成成功。
 */

export type BotCreationStage = 'starting' | 'submitting' | 'succeeded' | 'failed'
export type BotFailureStage = 'workspace' | 'tab' | 'start' | 'submit'

export type BotPreview = {
  botId: string
  name: string
  zone: string
  templateId: string
  agent: string
  botNameRule: string
  templateName: string
  zoneName: string
}

export type BotCreationFailure = {
  stage: BotFailureStage
  code: string
  message: string
}

export type BotCreation = {
  id: string
  name: string
  zone: string
  templateId: string
  stage: BotCreationStage
  tabId?: string
  paneId?: string
  agentName?: string
  inputSubmitted: boolean
  failure?: BotCreationFailure
}

/**
 * 一次 Bot 请求在页面一侧的可能归宿。
 *
 * - `ok`：后端返回了可理解的响应。
 * - `rejected`：后端给出明确类别与说明。
 * - `unreachable`：没有得到可用响应；不能当成成功或失败。
 */
export type BotResult<T> =
  | { kind: 'ok'; value: T }
  | { kind: 'rejected'; code: string; message: string }
  | { kind: 'unreachable' }

function isString(value: unknown): value is string {
  return typeof value === 'string'
}

function isNullableString(value: unknown): value is string | undefined {
  return value === undefined || typeof value === 'string'
}

function isPreview(value: unknown): value is BotPreview {
  if (typeof value !== 'object' || value === null) return false
  const preview = value as Record<string, unknown>
  return (
    isString(preview.botId) &&
    isString(preview.name) &&
    isString(preview.zone) &&
    isString(preview.templateId) &&
    isString(preview.agent) &&
    isString(preview.botNameRule) &&
    isString(preview.templateName) &&
    isString(preview.zoneName)
  )
}

const STAGES: BotCreationStage[] = ['starting', 'submitting', 'succeeded', 'failed']
const FAILURE_STAGES: BotFailureStage[] = ['workspace', 'tab', 'start', 'submit']

function isCreation(value: unknown): value is BotCreation {
  if (typeof value !== 'object' || value === null) return false
  const creation = value as Record<string, unknown>
  const failure = creation.failure
  const failureShape =
    failure === undefined ||
    (typeof failure === 'object' &&
      failure !== null &&
      isString((failure as Record<string, unknown>).stage) &&
      FAILURE_STAGES.includes((failure as Record<string, unknown>).stage as BotFailureStage) &&
      isString((failure as Record<string, unknown>).code) &&
      isString((failure as Record<string, unknown>).message))
  return (
    isString(creation.id) &&
    isString(creation.name) &&
    isString(creation.zone) &&
    isString(creation.templateId) &&
    isString(creation.stage) &&
    STAGES.includes(creation.stage as BotCreationStage) &&
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
): Promise<BotResult<T>> {
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

/** 读取本次创建实际会使用的名称与短实例标识；不创建 Tab，也不启动 agent。 */
export function previewBot(zone: string, templateId: string): Promise<BotResult<BotPreview>> {
  return request('/api/bots/preview', jsonInit('POST', { zone, templateId }), isPreview)
}

/** 接受一次创建；响应只表示后端已经接手，后续阶段由 `fetchBotCreation` 观察。 */
export function createBot(input: {
  zone: string
  templateId: string
  botId: string
  expectedName: string
  botNameRule?: string
  input: string
  workspaceTarget?: 'default' | 'current'
}): Promise<BotResult<BotCreation>> {
  return request('/api/bots', jsonInit('POST', input), isCreation)
}

/** 读取同一次创建当前阶段；成功只在后端确认就绪与提交之后出现。 */
export function fetchBotCreation(id: string): Promise<BotResult<BotCreation>> {
  return request(`/api/bots/${encodeURIComponent(id)}`, undefined, isCreation)
}
