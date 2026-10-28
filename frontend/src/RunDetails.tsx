import { RunList } from './RunList'
import { RunCurrentView } from './RunCurrentView'
import { useWorkflowRefresh } from './useWorkflowRefresh'
import { SavedChat } from './RunChat'
export function selectedRunId(): string | null {
  const match = /^#workflow\/([a-zA-Z0-9-]+)$/.exec(window.location.hash)
  return match?.[1] ?? null
}
export function RunDetails({ id, active }: { id: string | null; active: boolean }) {
  const { snapshot, error, refresh } = useWorkflowRefresh(id, active)
  const detail = snapshot?.id === id ? snapshot.detail : null
  const run = detail?.run
  return <section aria-label="workflow" className="workflow scheduler">
    <button type="button" onClick={refresh}>刷新 Run</button>
    <RunList entries={snapshot?.entries ?? null} />
    <h2>Run 详情</h2>
    {!id && <p>从 scheduler 选择 Map 并发起 Run；详情链接可刷新或重新进入，不会再次启动。</p>}
    {id && <p>Run ID：<code>{id}</code></p>}
    <p role="status" aria-label="workflow 更新">最后成功时间：{snapshot && snapshot.id === id ? <time dateTime={new Date(snapshot.at).toISOString()}>{new Date(snapshot.at).toLocaleString()}</time> : '尚无成功查询。'}</p>
    {error && <p role="alert">更新失败：{error} 保留上次成功现场；可见时继续按配置查询，也可手动刷新。查询不改写 Run，不重试任何运行操作。</p>}
    {detail && !detail.managed && <p>历史现场 / 未接管：当前后端只读取已保存事实。重启前的 Run 不能修改状态、report 或 collect；不提供自动或手动接管。普通 Chat Resume 不恢复 Run 管理。</p>}
    {detail?.startupError && <section role="alert" aria-label="管理者启动错误">
      <h3>管理者启动失败 · {detail.startupError.stage}</h3><p>{detail.startupError.message}</p>
      <p>已发生的外部效果未回滚；不会自动重试。请先核实下列真实引用，再决定如何处理。</p>
      <dl>{Object.entries(detail.startupError.effects ?? {}).map(([key, value]) => <div key={key}><dt>{key}</dt><dd>{String(value ?? '未知')}</dd></div>)}</dl>
    </section>}
    {detail?.readError && <p role="alert">运行记录读取失败：{detail.readError}</p>}
    {run && <>
      <dl><dt>Map</dt><dd>{run.map}</dd><dt>Run 状态</dt><dd>{run.status}</dd><dt>管理者 Task</dt><dd>{run.managerTaskId}</dd><dt>管理者 zone</dt><dd>{run.zone}</dd><dt>Workspace</dt><dd>{run.workspaceName}</dd></dl>
      <h3>本次要求（只读）</h3><pre data-testid="run-user-input">{run.userInput}</pre>
      <h3>管理者 Chat</h3>{run.manager ? <SavedChat label="管理者 Chat" chat={run.manager} /> : <p>尚无已保存管理者 Chat 关联。</p>}
      {snapshot?.current && <RunCurrentView run={run} reports={detail?.reports ?? []} current={snapshot.current} />}
    </>}
  </section>
}
