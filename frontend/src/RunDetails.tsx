import { useEffect, useState } from 'react'

type Reference = { sessionId: string; workspaceId: string; tabId: string; paneId: string; agentName: string }
type Run = { id: string; map: string; managerTaskId: string; zone: string; workspaceName: string; userInput: string; status: string; manager: Reference | null; nodes: Record<string, { status: string }> }
type Detail = { id: string; run: Run | null; managed: boolean; readError: string | null; startupError: { stage: string; message: string; effects: { recordsInitialized: boolean; agentStarted: boolean; associationSaved: boolean; inputSubmitted: boolean } & Partial<Reference> } | null }
export function selectedRunId(): string | null {
  const match = /^#workflow\/([a-zA-Z0-9-]+)$/.exec(window.location.hash)
  return match?.[1] ?? null
}
export function RunDetails({ id, active }: { id: string | null; active: boolean }) {
  const [detail, setDetail] = useState<Detail | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [revision, setRevision] = useState(0)
  useEffect(() => {
    if (!active || !id) return
    const controller = new AbortController()
    setDetail(null)
    setError(null)
    void fetch(`/api/workflow/runs/${encodeURIComponent(id)}`, { signal: controller.signal }).then(async response => {
      const value = await response.json()
      if (!response.ok) throw new Error(value.message ?? `Run 查询失败：HTTP ${response.status}`)
      if (!controller.signal.aborted) setDetail(value)
    }).catch(failure => { if (!controller.signal.aborted) setError(failure instanceof Error ? failure.message : 'Run 查询未取得有效响应。') })
    return () => controller.abort()
  }, [id, active, revision])
  const run = detail?.run
  return <section aria-label="workflow">
    <h2>Run 详情</h2>
    {!id && <p>从 scheduler 选择 Map 并发起 Run；详情链接可刷新或重新进入，不会再次启动。</p>}
    {id && <><p>Run ID：<code>{id}</code></p><button type="button" onClick={() => setRevision(value => value + 1)}>刷新 Run</button></>}
    {error && <p role="alert">{error} 不会自动重试或改写 Run。</p>}
    {detail && !detail.managed && <p>历史现场 / 未接管：当前后端只读取已保存事实。</p>}
    {detail?.startupError && <section role="alert" aria-label="管理者启动错误">
      <h3>管理者启动失败 · {detail.startupError.stage}</h3><p>{detail.startupError.message}</p>
      <p>已发生的外部效果未回滚；不会自动重试。请先核实下列真实引用，再决定如何处理。</p>
      <dl>{Object.entries(detail.startupError.effects).map(([key, value]) => <div key={key}><dt>{key}</dt><dd>{String(value ?? '未知')}</dd></div>)}</dl>
    </section>}
    {detail?.readError && <p role="alert">运行记录读取失败：{detail.readError}</p>}
    {run && <>
      <dl><dt>Map</dt><dd>{run.map}</dd><dt>Run 状态</dt><dd>{run.status}</dd><dt>管理者 Task</dt><dd>{run.managerTaskId}</dd><dt>管理者 zone</dt><dd>{run.zone}</dd><dt>Workspace</dt><dd>{run.workspaceName}</dd></dl>
      <h3>本次要求（只读）</h3><pre data-testid="run-user-input">{run.userInput}</pre>
      <h3>管理者 Chat</h3>{run.manager ? <dl>{Object.entries(run.manager).map(([key, value]) => <div key={key}><dt>{key}</dt><dd>{value}</dd></div>)}</dl> : <p>尚无已保存管理者 Chat 关联。</p>}
      <h3>节点状态</h3><ul>{Object.entries(run.nodes).map(([node, value]) => <li key={node}>{node}：{value.status}</li>)}</ul>
    </>}
  </section>
}
