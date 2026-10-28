export type ChatReference = { sessionId: string; workspaceId: string; tabId: string; paneId: string; agentName: string }
export type Execution = { id: string; taskId: string; zone: string; chat: ChatReference | null }
export type Failure = { stage: string; message: string; effects?: Record<string, unknown> }
export type NodeState = {
  status: string
  execution: Execution | null
  failure: Failure | null
  completion: { id: string; at: number; actor: ChatReference; reason: string } | null
  previousExecutions: { execution: Execution | null; failure: Failure | null }[]
}
export type Run = { id: string; map: string; createdAt: number; managerTaskId: string; zone: string; workspaceName: string; userInput: string; status: string; manager: ChatReference | null; nodes: Record<string, NodeState> }
export type Report = { id: string; at: number; nodeId: string; executionId: string; body: string; source: ChatReference; target: ChatReference }
export type RunDetail = { id: string; run: Run | null; managed: boolean; readError: string | null; startupError: Failure | null; reports: Report[]; pendingReports: Record<string, string> }
