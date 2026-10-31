export type Bot = {
  id: string
  name: string
  when: string
  plan_guide: string
  node_guide: string
  task_id?: string
}

export type RunPreflightInput = { managerTaskId: string; zone: string; userInput: string }
export type PreflightIssue = { code: string; target: 'map' | 'node' | 'bot' | 'task' | 'manager' | 'zone'; message: string; nodeId?: string; botId?: string; taskId?: string }
export type RunPreflightResult = { kind: 'valid'; workspaceName: string } | { kind: 'rejected'; message: string; issues: PreflightIssue[] }

export async function preflightRun(file: string, input: RunPreflightInput): Promise<RunPreflightResult> {
  const response = await fetch(`/api/scheduler/maps/${encodeURIComponent(file)}/preflight`, {
    method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(input),
  })
  const value = await response.json()
  if (response.ok && typeof value?.workspaceName === 'string') return { kind: 'valid', workspaceName: value.workspaceName }
  if (!response.ok && value?.code === 'run-preflight-rejected' && Array.isArray(value.issues)) {
    return { kind: 'rejected', message: value.message, issues: value.issues }
  }
  throw new Error(value?.message ?? `启动预检未取得有效响应：HTTP ${response.status}`)
}

export async function startRun(file: string, input: RunPreflightInput): Promise<{ id: string } | Extract<RunPreflightResult, { kind: 'rejected' }>> {
  const response = await fetch(`/api/scheduler/maps/${encodeURIComponent(file)}/runs`, {
    method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(input),
  })
  const value = await response.json()
  // Startup failures also carry a Run ID and belong on its refreshable detail.
  if (typeof value?.id === 'string') return { id: value.id }
  if (value?.code === 'run-preflight-rejected' && Array.isArray(value.issues)) return { kind: 'rejected', message: value.message, issues: value.issues }
  throw new Error(value?.message ?? `启动未取得有效响应：HTTP ${response.status}；不会自动重试，请先核实现场。`)
}

export type BotProblem = { code: string; message: string; files: string[]; bot_id?: string }
export type BotCatalog = { bots: Bot[]; problems?: BotProblem[] }

function isBotCatalog(value: unknown): value is BotCatalog {
  if (typeof value !== 'object' || value === null || !('bots' in value) || !Array.isArray(value.bots)) return false
  if (!value.bots.every(bot => bot && ['id', 'name', 'when', 'plan_guide', 'node_guide'].every(key => typeof bot[key] === 'string'))) return false
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
  return id === null ? { id: '', content: JSON.stringify(value, null, 2), revision: null }
    : { id: value.id, content: JSON.stringify(value.content, null, 2), revision: value.revision }
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

function isMap(value: unknown): value is WorkflowMap {
  return typeof value === 'object' && value !== null && 'title' in value && typeof value.title === 'string' &&
    'description' in value && typeof value.description === 'string' && 'nodes' in value && typeof value.nodes === 'object' && value.nodes !== null && !Array.isArray(value.nodes) &&
    Object.values(value.nodes).every(node => node && typeof node.title === 'string' && typeof node.bot_id === 'string' && typeof node.input === 'string' && typeof node.zone === 'string' && Array.isArray(node.blocked_by) && node.blocked_by.every((id: unknown) => typeof id === 'string'))
}

function isCatalog(value: unknown): value is MapCatalog {
  return typeof value === 'object' && value !== null && 'files' in value && Array.isArray(value.files) && value.files.every(name => typeof name === 'string') &&
    (!('titles' in value) || (typeof value.titles === 'object' && value.titles !== null && !Array.isArray(value.titles) && Object.values(value.titles).every(title => typeof title === 'string')))
}

export async function deleteMap(file: string, deleteHistory: boolean): Promise<MapCatalog> {
  return await writeDefinition(`/api/scheduler/maps/${encodeURIComponent(file)}/delete`, { deleteHistory }, isCatalog, 'Map 删除') as MapCatalog
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

export type MapNode = {
  title: string
  bot_id: string
  input: string
  zone: string
  blocked_by: string[]
  report_policy?: 'auto' | 'manual'
  auto_close?: boolean
}

export async function loadNodeDraft(file: string, id: string | null): Promise<{ id: string; content: string; revision: BotRevision; create: boolean }> {
  const base = `/api/scheduler/maps/${encodeURIComponent(file)}/nodes`
  const response = await fetch(`${base}/${id === null ? 'new' : 'edit'}`, id === null ? undefined : {
    method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ id }),
  })
  const value = await response.json()
  if (!response.ok) throw new Error(value.message ?? `node 读取失败：HTTP ${response.status}`)
  return { ...value, content: JSON.stringify(value.content, null, 2) }
}

export async function saveNode(file: string, draft: { id: string; content: string; revision: BotRevision | null; create: boolean }): Promise<WorkflowMap> {
  return await writeDefinition(`/api/scheduler/maps/${encodeURIComponent(file)}/nodes/save`, draft, isMap, 'node 保存') as WorkflowMap
}

export async function checkNodeDeletion(file: string, id: string): Promise<{ id: string; revision: BotRevision }> {
  const response = await fetch(`/api/scheduler/maps/${encodeURIComponent(file)}/nodes/check-delete`, {
    method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ id }),
  })
  const value = await response.json()
  if (!response.ok) throw new Error(value.message ?? `node 删除检查失败：HTTP ${response.status}`)
  return value
}

export async function deleteNode(file: string, target: { id: string; revision: BotRevision }): Promise<WorkflowMap> {
  return await writeDefinition(`/api/scheduler/maps/${encodeURIComponent(file)}/nodes/delete`, target, isMap, 'node 删除') as WorkflowMap
}

export type WorkflowMap = {
  title: string
  description: string
  nodes: Record<string, MapNode>
}

export async function loadMap(name: string, signal?: AbortSignal): Promise<WorkflowMap> {
  const response = await fetch(`/api/scheduler/maps/${encodeURIComponent(name)}`, { signal })
  const value = await response.json()
  if (!response.ok) throw Object.assign(new Error(typeof value?.message === 'string' ? value.message : `Map 读取失败：HTTP ${response.status}`), { status: response.status })
  if (!isMap(value)) throw new Error('Map 响应无效；请核实当前定义。')
  return value
}

export type MapCatalog = { files: string[]; titles?: Record<string, string> }

export async function listMaps(signal?: AbortSignal): Promise<MapCatalog> {
  const response = await fetch('/api/scheduler/maps', { signal })
  const value: unknown = await response.json()
  if (!response.ok) {
    throw new Error(typeof value === 'object' && value !== null && 'message' in value && typeof value.message === 'string'
      ? value.message : `Map 目录读取失败：HTTP ${response.status}`)
  }
  if (!isCatalog(value)) throw new Error('Map 目录响应无效。')
  return value
}
