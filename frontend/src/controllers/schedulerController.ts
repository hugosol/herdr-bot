import type { BotRevision } from './botController'

export type MapFileInput = { id: string; required: boolean; description?: string }
export type RunFileInput = { id: string; path: string }
export type RunPreflightInput = { managerTaskId: string; zone: string; userInput: string; fileInputs?: RunFileInput[] }
export type PreflightIssue = { code: string; target: 'map' | 'node' | 'bot' | 'task' | 'manager' | 'zone' | 'run-input'; message: string; nodeId?: string; botId?: string; taskId?: string; inputId?: string; callChain?: string[] }
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

/** Only a membership-only write may advance an open draft's optimistic revision. */
export function groupOnlyRevision(previous: BotRevision | null, next: BotRevision): boolean {
  if (!previous || previous.file !== next.file) return false
  try {
    const withoutGroup = (source: string) => {
      const value: unknown = JSON.parse(source)
      if (typeof value !== 'object' || value === null || Array.isArray(value)) return null
      const { group: _group, ...definition } = value as Record<string, unknown>
      return JSON.stringify(definition)
    }
    const before = withoutGroup(previous.original)
    return before !== null && before === withoutGroup(next.original)
  } catch { return false }
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

function isInputRule(value: unknown): value is InputRule[] {
  return Array.isArray(value) && new Set(value.map(rule => rule?.id)).size === value.length && value.every(rule =>
    typeof rule === 'object' && rule !== null && !Array.isArray(rule) && typeof rule.id === 'string' &&
    (rule.type === 'file' ? typeof rule.path === 'string' && (!('zone_id' in rule) || typeof rule.zone_id === 'string') && Object.keys(rule).every(key => ['id', 'type', 'path', 'zone_id'].includes(key))
      : rule.type === 'run' ? typeof rule.input_id === 'string' && Object.keys(rule).every(key => ['id', 'type', 'input_id'].includes(key))
      : rule.type === 'node' && typeof rule.node_id === 'string' && typeof rule.output_id === 'string' && Object.keys(rule).every(key => ['id', 'type', 'node_id', 'output_id'].includes(key))))
}

function isOutputRule(value: unknown): value is OutputRule[] {
  return Array.isArray(value) && new Set(value.map(rule => rule?.id)).size === value.length && value.every(rule =>
    typeof rule === 'object' && rule !== null && !Array.isArray(rule) && typeof rule.id === 'string' && Object.keys(rule).every(key => key === 'id'))
}

function isMapFileInputs(value: unknown): value is MapFileInput[] {
  return Array.isArray(value) && new Set(value.map(input => input?.id)).size === value.length && value.every(input =>
    typeof input === 'object' && input !== null && !Array.isArray(input) && typeof input.id === 'string' && input.id.trim() !== '' && typeof input.required === 'boolean' && (!('description' in input) || typeof input.description === 'string') && Object.keys(input).every(key => ['id', 'required', 'description'].includes(key)))
}

function isMap(value: unknown): value is WorkflowMap {
  return typeof value === 'object' && value !== null && (!('group' in value) || value.group == null || (typeof value.group === 'string' && value.group.length > 0 && value.group.trim() === value.group)) && 'title' in value && typeof value.title === 'string' &&
    (!('input_files' in value) || isMapFileInputs(value.input_files)) &&
    (!('input_requirements' in value) || typeof value.input_requirements === 'string') &&
    'description' in value && typeof value.description === 'string' && 'nodes' in value && typeof value.nodes === 'object' && value.nodes !== null && !Array.isArray(value.nodes) &&
    Object.values(value.nodes).every(node => node && typeof node.title === 'string' && typeof node.bot_id === 'string' && typeof node.input === 'string' && !('inputs' in node) && (!('input_rule' in node) || isInputRule(node.input_rule)) && (!('output_rule' in node) || isOutputRule(node.output_rule)) && typeof node.zone === 'string' && Array.isArray(node.blocked_by) && node.blocked_by.every((id: unknown) => typeof id === 'string'))
}

function isCatalog(value: unknown): value is MapCatalog {
  return typeof value === 'object' && value !== null && 'files' in value && Array.isArray(value.files) && value.files.every(name => typeof name === 'string') &&
    (!('groups' in value) || (typeof value.groups === 'object' && value.groups !== null && !Array.isArray(value.groups) && Object.values(value.groups).every(group => typeof group === 'string' && group.length > 0 && group.trim() === group))) &&
    (!('titles' in value) || (typeof value.titles === 'object' && value.titles !== null && !Array.isArray(value.titles) && Object.values(value.titles).every(title => typeof title === 'string')))
}

export async function deleteMap(file: string, deleteHistory: boolean): Promise<MapCatalog> {
  return await writeDefinition(`/api/scheduler/maps/${encodeURIComponent(file)}/delete`, { deleteHistory }, isCatalog, 'Map 删除') as MapCatalog
}

export type InputRule = { id: string; type: 'file'; path: string; zone_id?: string } | { id: string; type: 'node'; node_id: string; output_id: string } | { id: string; type: 'run'; input_id: string }
export type OutputRule = { id: string }

export type MapNode = {
  title: string
  bot_id: string
  manager_task_id?: string
  input: string
  input_rule?: InputRule[]
  output_rule?: OutputRule[]
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

export async function loadMapInputs(file: string): Promise<{ input_requirements: string; input_files: MapFileInput[]; revision: BotRevision }> {
  const response = await fetch(`/api/scheduler/maps/${encodeURIComponent(file)}/inputs`)
  const value = await response.json()
  if (!response.ok) throw new Error(value.message ?? `Map 输入声明读取失败：HTTP ${response.status}`)
  if (typeof value.input_requirements !== 'string' || !isMapFileInputs(value.input_files) || typeof value.revision?.file !== 'string' || typeof value.revision?.original !== 'string') throw new Error('Map 输入声明响应无效。')
  return value
}

export async function saveMapInputs(file: string, draft: { input_requirements: string; input_files: unknown; revision: BotRevision }): Promise<WorkflowMap> {
  if (typeof draft.input_requirements !== 'string') throw new Error('Map 输入要求必须是 Markdown 文本。')
  return await writeDefinition(`/api/scheduler/maps/${encodeURIComponent(file)}/inputs`, draft, isMap, 'Map 输入声明保存') as WorkflowMap
}

export async function loadMapTitle(file: string): Promise<{ title: string; revision: BotRevision }> {
  const response = await fetch(`/api/scheduler/maps/${encodeURIComponent(file)}/title`)
  const value = await response.json()
  if (!response.ok) throw new Error(value.message ?? `Map 名称读取失败：HTTP ${response.status}`)
  if (typeof value.title !== 'string' || typeof value.revision?.file !== 'string' || typeof value.revision?.original !== 'string') throw new Error('Map 名称响应无效。')
  return value
}

export async function saveMapTitle(file: string, draft: { title: string; revision: BotRevision }): Promise<WorkflowMap> {
  return await writeDefinition(`/api/scheduler/maps/${encodeURIComponent(file)}/title`, draft, isMap, 'Map 名称保存') as WorkflowMap
}

export type WorkflowMap = {
  group?: string | null
  title: string
  description: string
  input_requirements?: string
  input_files?: MapFileInput[]
  nodes: Record<string, MapNode>
}

export async function loadMap(name: string, signal?: AbortSignal): Promise<WorkflowMap> {
  const response = await fetch(`/api/scheduler/maps/${encodeURIComponent(name)}`, { signal })
  const value = await response.json()
  if (!response.ok) throw Object.assign(new Error(typeof value?.message === 'string' ? value.message : `Map 读取失败：HTTP ${response.status}`), { status: response.status })
  if (!isMap(value)) throw new Error('Map 响应无效；请核实当前定义。')
  return value
}

export type MapCatalog = { files: string[]; titles?: Record<string, string>; groups?: Record<string, string> }

export async function updateMapGroup(id: string, group: string | null): Promise<BotRevision> {
  const response = await fetch(`/api/scheduler/maps/${encodeURIComponent(id)}/group`, { method: 'PATCH', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ group }) })
  const value = await response.json()
  if (!response.ok) throw new Error(value?.message ?? `分组保存失败：HTTP ${response.status}`)
  if (typeof value !== 'object' || value === null || (value.group ?? null) !== group || typeof value.revision?.file !== 'string' || typeof value.revision?.original !== 'string') throw new Error('无法确认分组保存结果：响应无效。请刷新核实。')
  return value.revision
}

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
