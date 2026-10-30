import { useState } from 'react'
import { WorkflowRunGraph } from './WorkflowRunGraph'
import { orderedMapNodeIds } from '../scheduler/mapNodeOrder'
import { WorkflowStatus } from './WorkflowStatus'
import type { CurrentDefinition } from './useWorkflowRefresh'
import type { Report, Run } from './workflowApi'
import { RunChatReference } from './RunChatReference'
import './workflow.css'

function FactTime({ at }: { at: number }) {
  return <time dateTime={new Date(at).toISOString()}>{new Date(at).toLocaleString()}</time>
}

export function RunCurrentView({ run, reports, current, managed }: { run: Run; reports: Report[]; current: CurrentDefinition; managed: boolean }) {
  const { map, mapError, bots, tasks, catalogErrors } = current
  const [selected, setSelected] = useState<string | null>(null)
  const ids = map ? orderedMapNodeIds(map) : Object.keys(run.nodes)
  const nodeId = selected && run.nodes[selected] ? selected : ids[0]
  const node = run.nodes[nodeId]
  const definition = map?.nodes[nodeId]
  const bot = bots.find(bot => bot.id === definition?.bot_id)
  const taskName = (id: string) => {
    const name = tasks.find(task => task.id === id)?.name
    return name ? `${name} · ${id}` : id
  }
  const nodeNames = Object.fromEntries(ids.map(id => [id, map?.nodes[id]?.title ? `${map.nodes[id].title} · ${id}` : id]))
  const nodeTaskIds = Object.fromEntries(ids.map(id => {
    const currentBot = bots.find(candidate => candidate.id === map?.nodes[id]?.bot_id)
    return [id, run.nodes[id]?.execution?.taskId ?? currentBot?.task_id ?? null]
  }))
  const nodeTasks = Object.fromEntries(ids.map(id => [id, nodeTaskIds[id] ? tasks.find(task => task.id === nodeTaskIds[id])?.name || nodeTaskIds[id] : '未绑定或不可读']))
  const nodeBots = Object.fromEntries(ids.map(id => {
    const botId = map?.nodes[id]?.bot_id
    return [id, bots.find(candidate => candidate.id === botId)?.name || botId || '不可读']
  }))
  const nodeReportPolicies: Record<string, 'auto' | 'manual'> = Object.fromEntries(ids.map(id => {
    const execution = run.nodes[id]?.execution
    return [id, execution ? execution.reportPolicy ?? 'auto' : map?.nodes[id]?.report_policy ?? 'auto']
  }))
  const animateRunning = managed && run.status === 'running'
  const blockers = definition?.blocked_by.filter(id => run.nodes[id]?.status !== 'completed') ?? []
  const states = Object.fromEntries(ids.map(id => {
    let status = run.nodes[id].status
    if (status === 'pending' && map) status = run.status === 'running' && map.nodes[id].blocked_by.every(source => run.nodes[source]?.status === 'completed') ? 'ready' : 'blocked'
    const labels: Record<string, string> = { pending: '待执行', ready: '可派发', blocked: '等待依赖', running: '运行中', completed: '已完成', failed: '失败' }
    return [id, { status, label: labels[status] ?? status }]
  }))
  const applicable = reports.filter(report => report.nodeId === nodeId && report.executionId === node?.execution?.id)
  return <section className="workflow-current" aria-label="Run 节点事实">
    <div className="workflow-node-workspace">
      <aside className="workflow-node-sidebar" aria-label="节点导航">
        <p className="workflow-eyebrow">DEPENDENCY ORDER</p><h3>节点</h3>
        <p className="workflow-muted">{map ? '依赖层级排序 · 同层可并行' : '按已保存节点列出 · 依赖顺序不可用'}</p>
        <nav aria-label="已保存节点状态">{ids.map((id, index) => <button type="button" key={id} aria-pressed={nodeId === id} data-node-id={id} data-node-state={states[id].status} onClick={() => setSelected(id)}>
          <span className="workflow-node-number" aria-hidden="true">{String(index + 1).padStart(2, '0')}</span>
          <span className="workflow-node-nav-content"><strong>{map?.nodes[id]?.title ?? id}</strong><code>{id}</code><span className="workflow-node-task">Task · {nodeTasks[id]}</span><WorkflowStatus status={states[id].status} saved={!managed} animate={animateRunning} /></span>
        </button>)}</nav>
        {ids.length === 0 && <p>尚无已保存节点。</p>}
      </aside>
      <div className="workflow-mobile-node-selector"><label>选择节点<select value={nodeId ?? ''} onChange={event => setSelected(event.target.value)} disabled={ids.length === 0}>
        {ids.length === 0 && <option value="">尚无已保存节点</option>}
        {ids.map((id, index) => <option value={id} key={id}>{index + 1}. {nodeNames[id]} · Task：{nodeTasks[id]} · {states[id].label}</option>)}
      </select></label></div>
      <div className="workflow-node-canvas">
        <section className="workflow-graph-panel" aria-label="只读状态图">
          <header><div><p className="workflow-eyebrow">RUN OVERVIEW</p><h2>{map?.title || '工作流现场'}</h2></div><span className="workflow-muted">前置 → 后续 · 当前 Map 定义</span></header>
          {mapError && <p role="alert">当前 Map 不可用：{mapError} 不补造历史图。</p>}
          {map && <WorkflowRunGraph map={map} selectedId={nodeId ?? null} onSelect={setSelected} nodeStates={states} nodeTasks={nodeTasks} nodeBots={nodeBots} nodeReportPolicies={nodeReportPolicies} animateRunning={animateRunning} />}
          <p className="workflow-graph-note">图来自当前 Map，非历史定义快照。Task 名称来自当前目录；有执行时使用保存的执行 Task，未派发时为当前配置。箭头仅表示依赖，不表示自动执行或材料传递。{!animateRunning && '当前不播放运行动画。'}</p>
        </section>
        {catalogErrors.map((message, index) => <p role="alert" key={index}>{message}</p>)}
        {node && <section aria-label="当前节点详情" className="workflow-node-details">
          <header className="workflow-node-detail-heading"><div><p className="workflow-eyebrow">NODE / {nodeId}</p><h2>{map?.nodes[nodeId]?.title ?? nodeId}</h2><p className="workflow-detail-task">Task · {nodeTasks[nodeId]} {nodeTaskIds[nodeId] && <code>{nodeTaskIds[nodeId]}</code>}<small>{node.execution ? '已保存执行' : '当前配置'}</small></p></div><WorkflowStatus status={node.status} saved={!managed} animate={animateRunning} /></header>
          <p className="workflow-muted">report_policy: {nodeReportPolicies[nodeId] ?? 'auto'}</p>
          <section aria-label="当前执行适用报告" className="workflow-current-reports"><h3 className="workflow-eyebrow">当前执行适用报告</h3>{applicable.length ? applicable.map(report => <article key={report.id}><p className="workflow-report-meta">{report.id} · <FactTime at={report.at} /></p><pre>{report.body}</pre></article>) : <p className="workflow-muted">尚无当前执行适用报告。</p>}</section>
          <div className="workflow-node-journal">
            <section className="workflow-journal-entry" aria-label="当前工作要求"><span className="workflow-journal-dot" aria-hidden="true" /><h3>工作要求</h3>{definition ? <><p>{definition.input || '当前定义未填写工作要求。'}</p><p className="workflow-muted">来自当前 Map 定义，非历史执行快照。</p></> : <p>当前定义不可用，不补造历史工作要求。</p>}</section>
            <section className="workflow-journal-entry workflow-current-execution" aria-label="当前执行事实"><span className="workflow-journal-dot" aria-hidden="true" /><h3>执行关联</h3>
              {node.execution ? <><dl><dt>执行 ID</dt><dd>{node.execution.id}</dd><dt>执行 Task</dt><dd>{taskName(node.execution.taskId)}</dd><dt>执行 zone</dt><dd>{node.execution.zone}</dd></dl><RunChatReference label="当前执行 Chat" chat={node.execution.chat} /></> : <p>尚无当前执行或 Chat 关联；读取不会创建执行。</p>}
            </section>
            {node.completion && <section aria-label="交接完成说明" className="workflow-journal-entry workflow-completion"><span className="workflow-journal-dot" aria-hidden="true" /><h3>完成说明</h3><p>{node.completion.reason}</p><FactTime at={node.completion.at} /></section>}
          </div>
          {node.failure && <section aria-label="节点失败" className="workflow-failure"><h3>失败原因 · {node.failure.stage}</h3><p>{node.failure.message}</p>{node.failure.effects && <dl>{Object.entries(node.failure.effects).map(([key, value]) => <div key={key}><dt>{key}</dt><dd>{String(value ?? '未知')}</dd></div>)}</dl>}</section>}
          <details className="workflow-node-definition" key={`definition-${nodeId}`} aria-label="当前节点定义"><summary>当前定义（非执行快照）</summary>
            {definition ? <>
              <dl><dt>Bot</dt><dd>{bot?.name ? `${bot.name} · ${bot.id}` : definition.bot_id}</dd><dt>Task</dt><dd>{bot?.task_id ? taskName(bot.task_id) : '当前 Bot 无可读 Task 绑定。'}</dd><dt>zone</dt><dd>{definition.zone || '未绑定 zone'}</dd><dt>input</dt><dd><pre>{definition.input}</pre></dd><dt>前置依赖</dt><dd>{definition.blocked_by.length ? definition.blocked_by.map(id => nodeNames[id] ?? id).join('、') : '无前置依赖。'}</dd></dl>
              {node.status === 'pending' && <p>{blockers.length ? `前置未完成：${blockers.join('、')}` : '前置依赖已满足。'}{run.status !== 'running' && ` Run 状态为 ${run.status}，不可派发。`}</p>}
            </> : <p>当前定义不可用；仅显示保存事实。</p>}
          </details>
          <details key={`history-${nodeId}`} aria-label="已保存旧执行事实"><summary>旧执行历史（{node.previousExecutions.length} 次已保存执行）</summary>
            {node.previousExecutions.length ? node.previousExecutions.map((fact, index) => <article key={fact.execution?.id ?? index}>
              {fact.execution ? <><dl><dt>旧执行 ID</dt><dd>{fact.execution.id}</dd><dt>旧执行 Task</dt><dd>{taskName(fact.execution.taskId)}</dd><dt>旧执行 zone</dt><dd>{fact.execution.zone}</dd><dt>report_policy</dt><dd>{fact.execution.reportPolicy ?? 'auto'}</dd></dl><RunChatReference label="旧执行 Chat" chat={fact.execution.chat} /></> : <p>此旧执行未保存完整执行关联。</p>}
              {fact.failure && <p>旧失败 · {fact.failure.stage}：{fact.failure.message}</p>}
            </article>) : <p>尚无已保存旧执行。</p>}
            {reports.filter(report => report.nodeId === nodeId && report.executionId !== node.execution?.id).map(report => <article key={report.id}><p>旧报告 · {report.id} · 执行 {report.executionId} · <FactTime at={report.at} /></p><pre>{report.body}</pre><RunChatReference label="旧报告来源 Chat" chat={report.source} /></article>)}
          </details>
        </section>}
      </div>
    </div>
    <p className="workflow-readonly-note">节点状态不代表 Chat 在线或工作状态。只读入口不提供状态操作。需要核实或交接，请向已关联管理者 Chat 表达意图；报告不自动完成节点，普通 Chat Resume 不接管 Run。</p>
  </section>
}
