export type NodeTemplate = {
  schema_version: 1
  id: string
  title: string
  use_when: string
  description: string
  inputs: Record<string, string>
  outputs: Record<string, string>
  implementation: { type: 'chat' }
}

export type TemplateProblem = { code: string; message: string; files: string[]; template_id?: string }
export type TemplateCatalog = { templates: NodeTemplate[]; problems?: TemplateProblem[] }

export async function listTemplates(signal?: AbortSignal): Promise<TemplateCatalog> {
  const response = await fetch('/api/scheduler/templates', { signal })
  const value = await response.json()
  if (!response.ok) throw new Error(typeof value?.message === 'string' ? value.message : `共享模板目录读取失败：HTTP ${response.status}`)
  return value as TemplateCatalog
}

export type TemplateRevision = { file: string; original: string }

export async function loadTemplateDraft(id: string | null): Promise<{ id: string; content: string; revision: TemplateRevision | null }> {
  const response = await fetch(id === null ? '/api/scheduler/templates/new' : '/api/scheduler/templates/edit', id === null ? undefined : {
    method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ id }),
  })
  const value = await response.json()
  if (!response.ok) throw new Error(value.message ?? `模板读取失败：HTTP ${response.status}`)
  return id === null ? { id: '', content: JSON.stringify(value, null, 2), revision: null }
    : { id: value.id, content: JSON.stringify(value.content, null, 2), revision: value.revision }
}

export async function saveTemplate(draft: { id: string; content: string; revision: TemplateRevision | null }): Promise<NodeTemplate> {
  const response = await fetch('/api/scheduler/templates/save', {
    method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(draft),
  })
  const value = await response.json()
  if (!response.ok) throw new Error(value.message ?? `模板保存失败：HTTP ${response.status}`)
  return value as NodeTemplate
}

export type MapNode = {
  title: string
  node_template_id: string
  parameters: Record<string, unknown>
  blocked_by: string[]
}

export async function loadNodeDraft(file: string, id: string | null): Promise<{ id: string; content: string; revision: TemplateRevision; create: boolean }> {
  const base = `/api/scheduler/maps/${encodeURIComponent(file)}/nodes`
  const response = await fetch(`${base}/${id === null ? 'new' : 'edit'}`, id === null ? undefined : {
    method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ id }),
  })
  const value = await response.json()
  if (!response.ok) throw new Error(value.message ?? `node 读取失败：HTTP ${response.status}`)
  return { ...value, content: JSON.stringify(value.content, null, 2) }
}

export async function saveNode(file: string, draft: { id: string; content: string; revision: TemplateRevision | null; create: boolean }): Promise<void> {
  const response = await fetch(`/api/scheduler/maps/${encodeURIComponent(file)}/nodes/save`, {
    method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(draft),
  })
  const value = await response.json()
  if (!response.ok) throw new Error(value.message ?? `node 保存失败：HTTP ${response.status}`)
}

export type WorkflowMap = {
  schema_version: 1
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
