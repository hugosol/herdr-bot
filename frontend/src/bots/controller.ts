export type Bot = {
  group?: string | null
  id: string
  name: string
  when: string
  plan_guide: string
  node_guide: string
  task_id?: string
  map_file?: string
  rust_script?: string
}

export type BotProblem = { code: string; message: string; files: string[]; bot_id?: string }
export type BotCatalog = { bots: Bot[]; problems?: BotProblem[] }

function isBotCatalog(value: unknown): value is BotCatalog {
  if (typeof value !== 'object' || value === null || !('bots' in value) || !Array.isArray(value.bots)) return false
  if (!value.bots.every(bot => bot && (bot.group == null || (typeof bot.group === 'string' && bot.group.length > 0 && bot.group.trim() === bot.group)) && ['id', 'name', 'when', 'plan_guide', 'node_guide'].every(key => typeof bot[key] === 'string'))) return false
  return !('problems' in value) || (Array.isArray(value.problems) && value.problems.every(problem => problem && typeof problem.code === 'string' && typeof problem.message === 'string' && Array.isArray(problem.files)))
}

export async function listBots(signal?: AbortSignal): Promise<BotCatalog> {
  const response = await fetch('/api/scheduler/bots', { signal })
  const value = await response.json()
  if (!response.ok) throw new Error(typeof value?.message === 'string' ? value.message : `共享 Bot 目录读取失败：HTTP ${response.status}`)
  if (!isBotCatalog(value)) throw new Error('共享 Bot 目录响应无效。')
  return value
}

export type BotRevision = { file: string; original: string }

export async function loadBotDraft(id: string | null): Promise<{ id: string; content: string; revision: BotRevision | null }> {
  const response = await fetch(id === null ? '/api/scheduler/bots/new' : '/api/scheduler/bots/edit', id === null ? undefined : {
    method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ id }),
  })
  const value = await response.json()
  if (!response.ok) throw new Error(value.message ?? `Bot 读取失败：HTTP ${response.status}`)
  const content = id === null ? value : value.content
  const { group: _group, ...fields } = content
  return { id: id === null ? '' : value.id, content: JSON.stringify(fields, null, 2), revision: id === null ? null : value.revision }
}

async function writeDefinition(url: string, payload: unknown, valid: (value: unknown) => boolean, label: string): Promise<unknown> {
  let response: Response
  let value: unknown
  try {
    response = await fetch(url, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(payload) })
    value = await response.json()
  } catch {
    throw new Error(`无法确认${label}结果：未取得有效响应。请先核实当前定义，不要盲目重复提交；不会自动重试。`)
  }
  if (!response.ok) {
    throw new Error(typeof value === 'object' && value !== null && 'message' in value && typeof value.message === 'string'
      ? value.message : `${label}失败：HTTP ${response.status}`)
  }
  if (!valid(value)) throw new Error(`无法确认${label}结果：响应无效。请先核实当前定义，不要盲目重复提交；不会自动重试。`)
  return value
}

export async function saveBot(draft: { id: string; content: string; revision: BotRevision | null }): Promise<Bot> {
  return await writeDefinition('/api/scheduler/bots/save', draft, value => {
    if (typeof value !== 'object' || value === null) return false
    return ['id', 'name', 'when', 'plan_guide', 'node_guide'].every(key => typeof (value as Record<string, unknown>)[key] === 'string')
  }, 'Bot 保存') as Bot
}

export async function checkBotDelete(id: string): Promise<{ id: string; revision: BotRevision }> {
  const response = await fetch('/api/scheduler/bots/check-delete', {
    method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ id }),
  })
  const value = await response.json()
  if (!response.ok) throw new Error(value.message ?? `Bot 引用检查失败：HTTP ${response.status}`)
  return value
}

export async function deleteBot(target: { id: string; revision: BotRevision }): Promise<BotCatalog> {
  return await writeDefinition('/api/scheduler/bots/delete', target, isBotCatalog, 'Bot 删除') as BotCatalog
}

export async function updateBotGroup(id: string, group: string | null): Promise<BotRevision> {
  const response = await fetch(`/api/scheduler/bots/${encodeURIComponent(id)}/group`, { method: 'PATCH', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ group }) })
  const value = await response.json()
  if (!response.ok) throw new Error(value?.message ?? `分组保存失败：HTTP ${response.status}`)
  if (typeof value !== 'object' || value === null || (value.group ?? null) !== group || typeof value.revision?.file !== 'string' || typeof value.revision?.original !== 'string') throw new Error('无法确认分组保存结果：响应无效。请刷新核实。')
  return value.revision
}

