export type ChatReference = { sessionId: string; workspaceId: string; tabId: string; paneId: string; agentName: string }
export type Execution = { id: string; taskId: string; agent: 'omp' | 'pi'; zone: string; chat: ChatReference | null; reportPolicy?: 'auto' | 'manual' }
export type Failure = { stage: string; message: string; effects?: Record<string, unknown> }
export type NodeState = {
  status: string
  execution: Execution | null
  failure: Failure | null
  completion: { id: string; at: number; actor: ChatReference; reason: string } | null
  previousExecutions: { execution: Execution | null; failure: Failure | null }[]
}
export type Run = { id: string; name?: string; map: string; createdAt: number; managerTaskId: string; zone: string; workspaceName: string; userInput: string; status: string; manager: ChatReference | null; nodes: Record<string, NodeState> }
export type Report = { id: string; at: number; nodeId: string; executionId: string; body: string; source: ChatReference; target: ChatReference }
export type RunDetail = { id: string; run: Run | null; managed: boolean; readError: string | null; startupError: Failure | null; reports: Report[]; pendingReports: Record<string, string> }

export async function renameRun(id: string, name: string): Promise<RunDetail> {
  const response = await fetch(`/api/workflow/runs/${encodeURIComponent(id)}`, {
    method: 'PATCH', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ name }),
  })
  const value = await response.json()
  if (!response.ok) throw new Error(value.message ?? `重命名失败：HTTP ${response.status}`)
  return value
}
