import type { InputRule, OutputRule } from './schedulerController'

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

export type ChatReference = { sessionId: string; workspaceId: string; tabId: string; paneId: string; agentName: string }
export type FileAddress = { id: string; path: string }
export type ScriptLifecycle = { status: 'running'; startedAt: number } | { status: 'succeeded'; startedAt: number; endedAt: number } | { status: 'failed'; startedAt: number; endedAt: number; failure: unknown }
type ExecutionFiles = { id: string; zone: string; inputRule: InputRule[] | null; outputRule: OutputRule[] | null; inputs: FileAddress[] | null; reportPolicy?: 'auto' | 'manual'; autoClose?: boolean }
export type Execution = ExecutionFiles & ({ kind: 'rust_script'; rustScript: string; lifecycle: ScriptLifecycle; taskId?: never; agent?: never; chat?: never; childRunId?: never } | { kind?: never; taskId: string; agent: 'omp' | 'pi'; chat: ChatReference | null; childRunId?: string })
export type Failure = { stage: string; message: string; effects?: Record<string, unknown> }
export type NodeState = {
  status: string
  execution: Execution | null
  failure: Failure | null
  completion: { id: string; at: number; actor: ChatReference; reason: string } | null
  previousExecutions: { execution: Execution | null; failure: Failure | null }[]
}
export type Run = { id: string; name?: string; map: string; createdAt: number; managerTaskId: string; zone: string; managerZoneId?: string; zoneBindings?: Record<string, string>; workspaceName: string; userInput: string; fileInputs?: FileAddress[]; inputsReady?: boolean; status: string; manager: ChatReference | null; nodes: Record<string, NodeState> }
export type Report = { id: string; at: number; nodeId: string; executionId: string; body: string; from: ChatReference | null; source?: { kind: 'rust_script'; rustScript: string }; to: ChatReference; canComplete?: boolean; outputs: FileAddress[] }
export type RunDetail = { id: string; run: Run | null; managed: boolean; readError: string | null; startupError: Failure | null; reports: Report[]; pendingReports: Record<string, string> }

export type RunEntry = { id: string; managed: boolean; readError: string | null; run: { name?: string; map: string; createdAt: number; status: string; nodes: Record<string, { status: string }> } | null }
async function readJSON<T>(url: string): Promise<T> {
  const response = await fetch(url)
  const value = await response.json()
  if (!response.ok) throw new Error(value.message ?? `查询失败：HTTP ${response.status}`)
  return value
}

export async function listRuns(): Promise<{ runs: RunEntry[]; pollIntervalSeconds: number }> {
  return await readJSON('/api/workflow/runs')
}

export async function loadRun(id: string): Promise<RunDetail> {
  return await readJSON(`/api/workflow/runs/${encodeURIComponent(id)}`)
}

export async function renameRun(id: string, name: string): Promise<RunDetail> {
  const response = await fetch(`/api/workflow/runs/${encodeURIComponent(id)}`, {
    method: 'PATCH', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ name }),
  })
  const value = await response.json()
  if (!response.ok) throw new Error(value.message ?? `重命名失败：HTTP ${response.status}`)
  return value
}
