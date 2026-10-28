import { useEffect, useState } from 'react'
import { MapGraph } from './MapGraph'
import { listBots, loadMap, type Bot, type WorkflowMap } from './schedulerApi'
import { fetchTasks, type Task } from './taskApi'
import type { Report, Run } from './workflowApi'
import { SavedChat } from './RunChat'
import './workflow.css'

function FactTime({ at }: { at: number }) {
  return <time dateTime={new Date(at).toISOString()}>{new Date(at).toLocaleString()}</time>
}

export function RunCurrentView({ run, reports, active }: { run: Run; reports: Report[]; active: boolean }) {
  const [map, setMap] = useState<WorkflowMap | null>(null)
  const [mapError, setMapError] = useState<string | null>(null)
  const [bots, setBots] = useState<Bot[]>([])
  const [tasks, setTasks] = useState<Task[]>([])
  const [catalogErrors, setCatalogErrors] = useState<string[]>([])
  const [selected, setSelected] = useState<string | null>(null)
  useEffect(() => {
    if (!active) return
    const controller = new AbortController()
    setMap(null); setMapError(null); setBots([]); setTasks([]); setCatalogErrors([])
    void loadMap(run.map, controller.signal).then(value => {
      if (controller.signal.aborted) return
      const savedIds = Object.keys(run.nodes), currentIds = Object.keys(value.nodes)
      if (savedIds.length !== currentIds.length || savedIds.some(id => !Object.hasOwn(value.nodes, id))) {
        setMapError('当前 node 标识集合与 Run 不一致，无法可靠对应。')
        return
      }
      setMap(value)
    }).catch(error => { if (!controller.signal.aborted) setMapError(error instanceof Error ? error.message : 'Map 无法读取。') })
    void listBots(controller.signal).then(value => {
      if (controller.signal.aborted) return
      setBots(value.bots)
      if (value.problems?.length) setCatalogErrors(errors => [...errors, ...value.problems!.map(problem => problem.message)])
    }).catch(error => { if (!controller.signal.aborted) setCatalogErrors(errors => [...errors, `Bot 名称不可读：${String(error)}`]) })
    void fetchTasks().then(value => {
      if (controller.signal.aborted) return
      if (value.kind === 'ok') setTasks(value.tasks)
      else setCatalogErrors(errors => [...errors, 'Task 目录不可读；显示保存标识。'])
    })
    return () => controller.abort()
  }, [run, active])
  const nodeId = selected && run.nodes[selected] ? selected : Object.keys(run.nodes)[0]
  const node = run.nodes[nodeId]
  const definition = map?.nodes[nodeId]
  const bot = bots.find(bot => bot.id === definition?.bot_id)
  const taskName = (id: string) => tasks.find(task => task.id === id)?.name ? `${tasks.find(task => task.id === id)!.name} · ${id}` : id
  const nodeNames = Object.fromEntries(Object.keys(run.nodes).map(id => [id, map?.nodes[id]?.title ? `${map.nodes[id].title} · ${id}` : id]))
  const blockers = definition?.blocked_by.filter(id => run.nodes[id]?.status !== 'completed') ?? []
  const states = Object.fromEntries(Object.entries(run.nodes).map(([id, state]) => {
    let status = state.status
    if (status === 'pending' && map) status = run.status === 'running' && map.nodes[id]?.blocked_by.every(source => run.nodes[source]?.status === 'completed') ? 'ready' : 'blocked'
    const marker: Record<string, string> = { pending: '…', ready: '○', blocked: '⊘', running: '▶', completed: '✓', failed: '!' }
    return [id, { status, label: `${marker[status] ?? '?'} ${status}` }]
  }))
  const applicable = reports.filter(report => report.nodeId === nodeId && report.executionId === node?.execution?.id)
  return <section className="workflow-current scheduler" aria-label="Run 节点事实">
    <h3>只读状态图</h3>
    <p>图来自当前 Map，非历史定义快照。</p>
    {mapError && <p role="alert">当前 Map 不可用：{mapError} 不补造历史图。</p>}
    {map && <MapGraph map={map} selectedId={selected && run.nodes[selected] ? selected : null} onSelect={setSelected} nodeStates={states} />}
    {catalogErrors.map((message, index) => <p role="alert" key={index}>{message}</p>)}
    <h3>已保存节点状态</h3>
    <ul>{Object.entries(run.nodes).map(([id, value]) => <li key={id}><button type="button" onClick={() => setSelected(id)} aria-pressed={nodeId === id}>{id}：{value.status}</button></li>)}</ul>
    {node && <section aria-label="当前节点详情" className="workflow-node-details">
      <h3>{nodeNames[nodeId]}</h3>
      <p>节点状态：{node.status}（不代表 Chat 在线或工作状态）</p>
      <h4>当前定义（非执行快照）</h4>
      {definition ? <>
        <dl><dt>Bot</dt><dd>{bot?.name ? `${bot.name} · ${bot.id}` : definition.bot_id}</dd><dt>Task</dt><dd>{bot?.task_id ? taskName(bot.task_id) : '当前 Bot 无可读 Task 绑定。'}</dd><dt>zone</dt><dd>{definition.zone || '未绑定 zone'}</dd><dt>input</dt><dd><pre>{definition.input}</pre></dd><dt>前置依赖</dt><dd>{definition.blocked_by.length ? definition.blocked_by.map(id => nodeNames[id] ?? id).join('、') : '无前置依赖。'}</dd></dl>
        {node.status === 'pending' && <p>{blockers.length ? `前置未完成：${blockers.join('、')}` : '前置依赖已满足。'}{run.status !== 'running' && ` Run 状态为 ${run.status}，不可派发。`}</p>}
      </> : <p>当前定义不可用；仅显示保存事实。</p>}
      <h4>当前执行（已保存事实）</h4>
      {node.execution ? <><dl><dt>执行 ID</dt><dd>{node.execution.id}</dd><dt>执行 Task</dt><dd>{taskName(node.execution.taskId)}</dd><dt>执行 zone</dt><dd>{node.execution.zone}</dd></dl><SavedChat label="当前执行 Chat" chat={node.execution.chat} /></> : <p>尚无当前执行或 Chat 关联；读取不会创建执行。</p>}
      {node.failure && <section aria-label="节点失败"><h4>失败原因 · {node.failure.stage}</h4><p>{node.failure.message}</p>{node.failure.effects && <dl>{Object.entries(node.failure.effects).map(([key, value]) => <div key={key}><dt>{key}</dt><dd>{String(value ?? '未知')}</dd></div>)}</dl>}</section>}
      {node.completion && <section aria-label="交接完成说明"><h4>完成说明</h4><p>{node.completion.reason}</p><FactTime at={node.completion.at} /></section>}
      <section aria-label="当前执行适用报告"><h4>当前执行适用报告</h4>{applicable.length ? applicable.map(report => <article key={report.id}><p>{report.id} · <FactTime at={report.at} /></p><pre>{report.body}</pre></article>) : <p>尚无当前执行适用报告。</p>}</section>
      <details key={nodeId} aria-label="已保存旧执行事实"><summary>旧执行历史（{node.previousExecutions.length} 次已保存执行）</summary>
        {node.previousExecutions.length ? node.previousExecutions.map((fact, index) => <article key={fact.execution?.id ?? index}>
          {fact.execution ? <><dl><dt>旧执行 ID</dt><dd>{fact.execution.id}</dd><dt>旧执行 Task</dt><dd>{taskName(fact.execution.taskId)}</dd><dt>旧执行 zone</dt><dd>{fact.execution.zone}</dd></dl><SavedChat label="旧执行 Chat" chat={fact.execution.chat} /></> : <p>此旧执行未保存完整执行关联。</p>}
          {fact.failure && <p>旧失败 · {fact.failure.stage}：{fact.failure.message}</p>}
        </article>) : <p>尚无已保存旧执行。</p>}
        {reports.filter(report => report.nodeId === nodeId && report.executionId !== node.execution?.id).map(report => <article key={report.id}><p>旧报告 · {report.id} · 执行 {report.executionId} · <FactTime at={report.at} /></p><pre>{report.body}</pre><SavedChat label="旧报告来源 Chat" chat={report.source} /></article>)}
      </details>
    </section>}
    <p>只读入口不提供状态操作。需要核实或交接，请向已关联管理者 Chat 表达意图；报告不自动完成节点，普通 Chat Resume 不接管 Run。</p>
  </section>
}
