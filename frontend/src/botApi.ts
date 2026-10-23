/**
 * Bot 接口客户端：页面与接口同源，请求地址直接使用相对路径。
 *
 * Bot 由后端持久保存，电脑与手机读取同一份；页面不缓存 Bot，也不自行生成固定选项。
 * 能力事实（agent / model / thinking / skill）由后端公开，保存失败时页面不能表示已保存。
 */

export type BotAgent = 'omp' | 'pi'
export type BotModel = string
export type BotThinking = string
export type BotSkill = string

/** 一个已保存的 Bot；字段与后端存储一致。 */
export type Bot = {
  id: string
  name: string
  chatName: string
  agent: BotAgent
  model: BotModel
  thinking: BotThinking
  skill: BotSkill | null
  prompt: string
  extensions: string[]
}

/** 新增或编辑时提交的字段（标识由后端维护）。 */
export type BotInput = Omit<Bot, 'id'>

/** 当前元数据选项；`thinking[agent][model]` 是显式级别，`default` 模型对应空数组。 */
export type BotOptions = {
  agents: BotAgent[]
  models: BotModel[]
  thinking: Record<string, Record<string, BotThinking[]>>
  skills: BotSkill[]
  labels?: Record<string, Record<string, string>>
  extensions?: { id: string; label: string; agent: BotAgent; kind: 'fixed' | 'optional'; entrypoint: string }[]
}

export type BotList = {
  bots: Bot[]
  options: BotOptions
}

/**
 * 一次 Bot 请求在页面一侧的可能归宿。
 *
 * - `ok`：后端返回了当前完整列表；只有这时才能更新界面或显示成功。
 * - `rejected`：后端给出了明确的失败类别与说明（字段非法、存储失败等）。
 * - `unreachable`：没有得到可用响应（网络中断或响应无法解析），不能当成成功或失败。
 */
export type BotResult =
  | { kind: 'ok'; bots: Bot[]; options: BotOptions }
  | { kind: 'rejected'; code: string; message: string }
  | { kind: 'unreachable' }

function isStringArray(value: unknown): value is string[] {
  return Array.isArray(value) && value.every((entry) => typeof entry === 'string')
}

function isBot(value: unknown): value is Bot {
  if (typeof value !== 'object' || value === null) return false
  const bot = value as Record<string, unknown>
  return (
    typeof bot.id === 'string' &&
    typeof bot.name === 'string' &&
    typeof bot.chatName === 'string' &&
    typeof bot.agent === 'string' &&
    typeof bot.model === 'string' &&
    typeof bot.thinking === 'string' &&
    (bot.skill === null || typeof bot.skill === 'string') &&
    typeof bot.prompt === 'string' &&
    isStringArray(bot.extensions)
  )
}

function isOptions(value: unknown): value is BotOptions {
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

function isBotList(value: unknown): value is BotList {
  if (typeof value !== 'object' || value === null) return false
  const payload = value as Record<string, unknown>
  return (
    Array.isArray(payload.bots) &&
    payload.bots.every(isBot) &&
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
 * 发出一次 Bot 请求并归一化答复。
 *
 * 只有 HTTP 成功且响应形状正确才算 `ok`；明确的错误码与说明原样交给界面；
 * 其余情形一律是 `unreachable`，不能显示成成功。
 */
async function botRequest(
  input: RequestInfo | URL,
  init?: RequestInit,
): Promise<BotResult> {
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
    return isBotList(payload)
      ? {
          kind: 'ok',
          bots: payload.bots,
          options: payload.options,
        }
      : { kind: 'unreachable' }
  }

  return isError(payload)
    ? { kind: 'rejected', code: payload.code, message: payload.message }
    : { kind: 'unreachable' }
}

function jsonInit(method: string, body: BotInput): RequestInit {
  return {
    method,
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  }
}

/** 读取当前 Bot 列表与固定选项；每次进入该 tab 都重新读取后端。 */
export function fetchBots(): Promise<BotResult> {
  return botRequest('/api/bots')
}

/** 新增一个 Bot；成功只意味着后端已经持久化。 */
export function createBot(input: BotInput): Promise<BotResult> {
  return botRequest('/api/bots', jsonInit('POST', input))
}

/** 编辑一个 Bot；标识不变，成功只意味着后端已经持久化。 */
export function updateBot(
  id: string,
  input: BotInput,
): Promise<BotResult> {
  return botRequest(`/api/bots/${encodeURIComponent(id)}`, jsonInit('PUT', input))
}

/** 删除一个 Bot；只移除 Bot，不改变任何已经发起的 Chat。 */
export function deleteBot(id: string): Promise<BotResult> {
  return botRequest(`/api/bots/${encodeURIComponent(id)}`, {
    method: 'DELETE',
  })
}
