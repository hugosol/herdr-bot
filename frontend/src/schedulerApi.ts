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

export type BotProblem = { code: string; message: string; files: string[]; bot_id?: string }
export type BotCatalog = { bots: Bot[]; problems?: BotProblem[] }

export async function listBots(signal?: AbortSignal): Promise<BotCatalog> {
  const response = await fetch('/api/scheduler/bots', { signal })
  const value = await response.json()
  if (!response.ok) throw new Error(typeof value?.message === 'string' ? value.message : `共享 Bot 目录读取失败：HTTP ${response.status}`)
  return value as BotCatalog
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

export async function saveBot(draft: { id: string; content: string; revision: BotRevision | null }): Promise<Bot> {
  const response = await fetch('/api/scheduler/bots/save', {
    method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(draft),
  })
  const value = await response.json()
  if (!response.ok) throw new Error(value.message ?? `Bot 保存失败：HTTP ${response.status}`)
  return value as Bot
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
  const response = await fetch('/api/scheduler/bots/delete', {
    method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(target),
  })
  const value = await response.json()
  if (!response.ok) throw new Error(value.message ?? `Bot 删除失败：HTTP ${response.status}`)
  return value as BotCatalog
}

export type MapNode = {
  title: string
  bot_id: string
  input: string
  zone: string
  blocked_by: string[]
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

export async function saveNode(file: string, draft: { id: string; content: string; revision: BotRevision | null; create: boolean }): Promise<void> {
  const response = await fetch(`/api/scheduler/maps/${encodeURIComponent(file)}/nodes/save`, {
    method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(draft),
  })
  const value = await response.json()
  if (!response.ok) throw new Error(value.message ?? `node 保存失败：HTTP ${response.status}`)
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
  const response = await fetch(`/api/scheduler/maps/${encodeURIComponent(file)}/nodes/delete`, {
    method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(target),
  })
  const value = await response.json()
  if (!response.ok) throw new Error(value.message ?? `node 删除失败：HTTP ${response.status}`)
  return value
}

export type WorkflowMap = {
  title: string
  description: string
  nodes: Record<string, MapNode>
}

export async function loadMap(name: string, signal?: AbortSignal): Promise<WorkflowMap> {
  const response = await fetch(`/api/scheduler/maps/${encodeURIComponent(name)}`, { signal })
  const value = await response.json()
  if (!response.ok) throw new Error(typeof value?.message === 'string' ? value.message : `Map 读取失败：HTTP ${response.status}`)
  return value as WorkflowMap
}

export async function listMaps(signal?: AbortSignal): Promise<string[]> {
  const response = await fetch('/api/scheduler/maps', { signal })
  const value: unknown = await response.json()
  if (!response.ok) {
    throw new Error(typeof value === 'object' && value !== null && 'message' in value && typeof value.message === 'string'
      ? value.message : `Map 目录读取失败：HTTP ${response.status}`)
  }
  if (typeof value !== 'object' || value === null || !('files' in value) ||
      !Array.isArray(value.files) || !value.files.every((name) => typeof name === 'string')) {
    throw new Error('Map 目录响应无效。')
  }
  return value.files
}
