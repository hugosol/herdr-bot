import { useEffect, useLayoutEffect, useRef } from 'react'
import { RunList } from './RunList'
import { RunCurrentView } from './RunCurrentView'
import { useWorkflowRefresh } from './useWorkflowRefresh'
import { SavedChat } from './RunChat'
import { WorkflowStatus } from './WorkflowStatus'

export function selectedRunId(): string | null {
  // The explicit list route must clear selection, including on browser Back.
  if (window.location.hash === '#workflow') return null
  const match = /^#workflow\/([a-zA-Z0-9-]+)$/.exec(window.location.hash)
  return match?.[1] ?? null
}

export function RunDetails({ id, active }: { id: string | null; active: boolean }) {
  const { snapshot, error, refresh } = useWorkflowRefresh(id, active)
  const detail = snapshot?.id === id ? snapshot.detail : null
  const run = detail?.run
  const listScroll = useRef(0)
  const previousId = useRef(id)
  useEffect(() => {
    if (id || !active) return
    const remember = () => { listScroll.current = window.scrollY }
    window.addEventListener('scroll', remember, { passive: true })
    return () => window.removeEventListener('scroll', remember)
  }, [id, active])
  useLayoutEffect(() => {
    if (previousId.current !== id) {
      window.scrollTo(0, id ? 0 : listScroll.current)
      previousId.current = id
    }
  }, [id])
  return <section aria-label="workflow" className="workflow">
    {id && <header className="workflow-run-heading">
      <div><a className="workflow-back" href="#workflow"><span aria-hidden="true">← </span>返回全部 Run</a><h2 className="workflow-eyebrow workflow-run-detail-label">Run 详情</h2><h1>{(snapshot?.id === id ? snapshot.current?.map?.title : null) || run?.map || '运行记录'}</h1><code>{run ? `${run.map} / ${id}` : id}</code></div>
      {run && <div><WorkflowStatus status={run.status} saved={!detail?.managed} /><p className="workflow-muted">{Number.isFinite(run.createdAt) && !Number.isNaN(new Date(run.createdAt).getTime()) && <><time dateTime={new Date(run.createdAt).toISOString()}>{new Date(run.createdAt).toLocaleString()}</time> · </>}{Object.values(run.nodes).filter(node => node.status === 'completed').length} / {Object.keys(run.nodes).length} 已交接</p></div>}
    </header>}
    <div className="workflow-refresh-info"><p role="status" aria-label="workflow 更新">最后成功时间：{snapshot && snapshot.id === id ? <time dateTime={new Date(snapshot.at).toISOString()}>{new Date(snapshot.at).toLocaleString()}</time> : '尚无成功查询。'}</p><button type="button" onClick={refresh}>刷新 Run</button></div>
    {error && <p className="workflow-alert" role="alert">更新失败：{error} 保留上次成功现场；可见时继续按配置查询，也可手动刷新。查询不改写 Run，不重试任何运行操作。</p>}
    <div hidden={id !== null}><RunList entries={snapshot?.entries ?? null} error={error} onOpen={() => { listScroll.current = window.scrollY }} /></div>
    {id && <>
      {!detail && !error && <p>正在加载 Run 详情…</p>}
      {detail && !detail.managed && <p className="workflow-alert">历史现场 / 未接管：当前后端只读取已保存事实。重启前的 Run 不能修改状态、report 或 collect；不提供自动或手动接管。普通 Chat Resume 不恢复 Run 管理。</p>}
      {detail?.startupError && <section className="workflow-alert" role="alert" aria-label="管理者启动错误">
        <h3>管理者启动失败 · {detail.startupError.stage}</h3><p>{detail.startupError.message}</p>
        <p>已发生的外部效果未回滚；不会自动重试。请先核实下列真实引用，再决定如何处理。</p>
        <dl>{Object.entries(detail.startupError.effects ?? {}).map(([key, value]) => <div key={key}><dt>{key}</dt><dd>{String(value ?? '未知')}</dd></div>)}</dl>
      </section>}
      {detail?.readError && <p className="workflow-alert" role="alert">运行记录读取失败：{detail.readError}</p>}
      {run && detail && <>
        {snapshot?.current && <RunCurrentView key={run.id} run={run} managed={detail.managed} reports={detail.reports} current={snapshot.current} />}
        <details className="workflow-run-background">
          <summary>运行背景 · 本次要求与管理者</summary>
          <dl><dt>管理者 Task</dt><dd>{run.managerTaskId}</dd><dt>管理者 zone</dt><dd>{run.zone}</dd><dt>Workspace</dt><dd>{run.workspaceName}</dd></dl>
          <h3>本次要求（只读）</h3><pre data-testid="run-user-input">{run.userInput}</pre>
          <h3>管理者 Chat</h3>{run.manager ? <SavedChat label="管理者 Chat" chat={run.manager} /> : <p>尚无已保存管理者 Chat 关联。</p>}
        </details>
      </>}
    </>}
  </section>
}
