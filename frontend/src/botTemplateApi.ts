/**
 * Bot 模板接口客户端：页面与接口同源，请求地址直接使用相对路径。
 *
 * 模板由后端持久保存，电脑与手机读取同一份；页面不缓存模板，也不自行生成固定选项。
 * 能力事实（agent / model / thinking / skill）由后端公开，保存失败时页面不能表示已保存。
 */

export type TemplateAgent = 'omp' | 'pi'
export type TemplateModel = string
export type TemplateThinking = string
export type TemplateSkill = string

/** 一个已保存的模板；字段与后端存储一致。 */
export type BotTemplate = {
  id: string
  name: string
  botName: string
  agent: TemplateAgent
  model: TemplateModel
  thinking: TemplateThinking
  skill: TemplateSkill | null
  prompt: string
}

/** 新增或编辑时提交的字段（标识由后端维护）。 */
export type BotTemplateInput = Omit<BotTemplate, 'id'>

/** 固定选项；`thinking[agent][model]` 是显式级别，`default` 模型对应空数组。 */
export type BotTemplateOptions = {
  agents: TemplateAgent[]
  models: TemplateModel[]
  thinking: Record<string, Record<string, TemplateThinking[]>>
  skills: TemplateSkill[]
  labels?: Record<string, Record<string, string>>
  extensions?: { id: string; label: string; agent: TemplateAgent; kind: 'fixed' | 'optional'; entrypoint: string }[]
}

export type BotTemplateList = {
  templates: BotTemplate[]
  options: BotTemplateOptions
}

/**
 * 一次模板请求在页面一侧的可能归宿。
 *
 * - `ok`：后端返回了当前完整列表；只有这时才能更新界面或显示成功。
 * - `rejected`：后端给出了明确的失败类别与说明（字段非法、存储失败等）。
 * - `unreachable`：没有得到可用响应（网络中断或响应无法解析），不能当成成功或失败。
 */
export type BotTemplateResult =
  | { kind: 'ok'; templates: BotTemplate[]; options: BotTemplateOptions }
  | { kind: 'rejected'; code: string; message: string }
  | { kind: 'unreachable' }

function isStringArray(value: unknown): value is string[] {
  return Array.isArray(value) && value.every((entry) => typeof entry === 'string')
}

function isTemplate(value: unknown): value is BotTemplate {
  if (typeof value !== 'object' || value === null) return false
  const template = value as Record<string, unknown>
  return (
    typeof template.id === 'string' &&
    typeof template.name === 'string' &&
    typeof template.botName === 'string' &&
    typeof template.agent === 'string' &&
    typeof template.model === 'string' &&
    typeof template.thinking === 'string' &&
    (template.skill === null || typeof template.skill === 'string') &&
    typeof template.prompt === 'string'
  )
}

function isOptions(value: unknown): value is BotTemplateOptions {
  if (typeof value !== 'object' || value === null) return false
  const options = value as Record<string, unknown>
  return (
    isStringArray(options.agents) &&
    isStringArray(options.models) &&
    isStringArray(options.skills) &&
    typeof options.thinking === 'object' &&
    options.thinking !== null
  )
}

function isTemplateList(value: unknown): value is BotTemplateList {
  if (typeof value !== 'object' || value === null) return false
  const payload = value as Record<string, unknown>
  return (
    Array.isArray(payload.templates) &&
    payload.templates.every(isTemplate) &&
    isOptions(payload.options)
  )
}

function isError(value: unknown): value is { code: string; message: string } {
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
 * 发出一次模板请求并归一化答复。
 *
 * 只有 HTTP 成功且响应形状正确才算 `ok`；明确的错误码与说明原样交给界面；
 * 其余情形一律是 `unreachable`，不能显示成成功。
 */
async function templateRequest(
  input: RequestInfo | URL,
  init?: RequestInit,
): Promise<BotTemplateResult> {
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
    return isTemplateList(payload)
      ? {
          kind: 'ok',
          templates: payload.templates,
          options: payload.options,
        }
      : { kind: 'unreachable' }
  }

  return isError(payload)
    ? { kind: 'rejected', code: payload.code, message: payload.message }
    : { kind: 'unreachable' }
}

function jsonInit(method: string, body: BotTemplateInput): RequestInit {
  return {
    method,
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  }
}

/** 读取当前模板列表与固定选项；每次进入该 tab 都重新读取后端。 */
export function fetchBotTemplates(): Promise<BotTemplateResult> {
  return templateRequest('/api/bot-templates')
}

/** 新增一个模板；成功只意味着后端已经持久化。 */
export function createBotTemplate(input: BotTemplateInput): Promise<BotTemplateResult> {
  return templateRequest('/api/bot-templates', jsonInit('POST', input))
}

/** 编辑一个模板；标识不变，成功只意味着后端已经持久化。 */
export function updateBotTemplate(
  id: string,
  input: BotTemplateInput,
): Promise<BotTemplateResult> {
  return templateRequest(`/api/bot-templates/${encodeURIComponent(id)}`, jsonInit('PUT', input))
}

/** 删除一个模板；只移除模板，不改变任何已经创建的 Bot。 */
export function deleteBotTemplate(id: string): Promise<BotTemplateResult> {
  return templateRequest(`/api/bot-templates/${encodeURIComponent(id)}`, {
    method: 'DELETE',
  })
}
