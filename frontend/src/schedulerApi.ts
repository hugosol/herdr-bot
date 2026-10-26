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

export async function listTemplates(signal?: AbortSignal): Promise<NodeTemplate[]> {
  const response = await fetch('/api/scheduler/templates', { signal })
  const value = await response.json()
  if (!response.ok) throw new Error(typeof value?.message === 'string' ? value.message : `共享模板目录读取失败：HTTP ${response.status}`)
  return value.templates as NodeTemplate[]
}

export type MapNode = {
  title: string
  node_template_id: string
  parameters: Record<string, unknown>
  blocked_by: string[]
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
