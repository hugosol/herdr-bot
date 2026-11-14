import { useState } from 'react'
import { WorkflowRunGraph } from './WorkflowRunGraph'
import { orderedMapNodeIds } from '../scheduler/mapNodeOrder'
import { WorkflowStatus } from './WorkflowStatus'
import type { CurrentDefinition } from './useWorkflowRefresh'
import type { Execution, FileAddress, Report, Run } from './controller'
import { RunChatReference } from './RunChatReference'
import './workflow.css'

function FactTime({ at }: { at: number }) {
  return <time dateTime={new Date(at).toISOString()}>{new Date(at).toLocaleString()}</time>
}

function OutputAddresses({ outputs, empty }: { outputs: FileAddress[]; empty: string }) {
  return outputs.length ? <dl>{outputs.map(({ id, path }) => <div key={id}><dt>id: <code>{id}</code></dt><dd>path: <code className="workflow-path">{path}</code></dd></div>)}</dl> : <p className="workflow-muted">{empty}</p>
}

function ExecutionHandoff({ execution }: { execution: Execution }) {
  return <section aria-label="执行交接快照"><h4>inputRule</h4>{execution.inputRule === null ? <p>未知：历史执行未记录输入规则。</p> : <pre>{JSON.stringify(execution.inputRule, null, 2)}</pre>}<h4>outputRule</h4>{execution.outputRule === null ? <p>未知：历史执行未记录输出规则。</p> : <pre>{JSON.stringify(execution.outputRule, null, 2)}</pre>}<h4>inputs</h4>{execution.inputs === null ? <p>未知：历史执行未记录实际文件输入；不从当前 Map 或报告反推。</p> : <OutputAddresses outputs={execution.inputs} empty="本次执行无实际文件输入。" />}</section>
}

function ScriptExecutionFacts({ execution, handoff }: { execution: Extract<Execution, { kind: 'rust_script' }>; handoff: 'pending' | 'completed' | 'historical' }) {
  const lifecycle = execution.lifecycle
  return <>
    <dl><dt>id</dt><dd><code>{execution.id}</code></dd><dt>kind</dt><dd>rust_script</dd><dt>rustScript</dt><dd><code>{execution.rustScript}</code></dd><dt>zone</dt><dd className="workflow-path">{execution.zone}</dd></dl>
    <p>{lifecycle.status === 'running' ? '业务执行在途' : lifecycle.status === 'succeeded' ? '业务执行已成功结束' : '业务执行已失败结束'}</p>
    <p>startedAt: <FactTime at={lifecycle.startedAt} />{'endedAt' in lifecycle && <> · endedAt: <FactTime at={lifecycle.endedAt} /></>}</p>
    {lifecycle.status === 'failed' && <section aria-label="脚本业务错误与实际效果"><h4>原错误与实际效果</h4><pre>{JSON.stringify(lifecycle.failure, null, 2)}</pre></section>}
    <p>脚本自动报告；业务结束不等于节点 complete。{handoff === 'completed' ? '节点已由 manager 完成交接。' : handoff === 'historical' ? '旧执行不授予当前执行完成资格。' : '仍待 manager 显式处理。'}没有 Chat 或 Tab，无需关闭或恢复会话。</p>
    <ExecutionHandoff execution={execution} />
  </>
}

function ReportHandoff({ report }: { report: Report }) {
  return <section aria-label="报告交接声明"><p>canComplete: {report.canComplete === undefined ? '缺失（历史报告未声明）' : String(report.canComplete)}</p><h4>本次报告提交 outputs</h4><OutputAddresses outputs={report.outputs ?? []} empty="本次报告未提交输出地址；不会清除此前登记。" />{report.source?.kind === 'rust_script' ? <><p>source · rust_script · <code>{report.source.rustScript}</code></p><p>to · manager sessionId: <code>{report.to.sessionId}</code></p></> : <><RunChatReference label="from" chat={report.from} /><RunChatReference label="to" chat={report.to} /></>}</section>
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
  const nodeTargets = Object.fromEntries(ids.map(id => {
    const execution = run.nodes[id]?.execution
    const currentBot = bots.find(candidate => candidate.id === map?.nodes[id]?.bot_id)
    if (execution?.kind === 'rust_script') return [id, { label: '脚本', name: execution.rustScript }]
    if (execution) return [id, { label: 'Task', name: taskName(execution.taskId) }]
    if (currentBot && 'rust_script' in currentBot && typeof currentBot.rust_script === 'string') return [id, { label: '脚本', name: currentBot.rust_script }]
    if (currentBot?.map_file) return [id, { label: 'Map', name: currentBot.map_file }]
    return [id, { label: 'Task', name: currentBot?.task_id ? taskName(currentBot.task_id) : '未绑定或不可读' }]
  }))
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
    if (status === 'pending') {
      if (run.inputsReady !== true) status = 'preparing'
      else if (map) status = run.status === 'running' && map.nodes[id].blocked_by.every(source => run.nodes[source]?.status === 'completed') ? 'ready' : 'blocked'
    }
    const labels: Record<string, string> = { pending: '待执行', preparing: '等待输入准备', ready: '可派发', blocked: '等待依赖', running: '运行中', completed: '已完成', failed: '失败' }
    return [id, { status, label: labels[status] ?? status }]
  }))
  const applicable = node?.execution ? reports.filter(report => report.nodeId === nodeId && report.executionId === node.execution!.id) : []
  // The API preserves messages.jsonl append order, not timestamp order.
  const outputById = new Map<string, FileAddress>()
  for (const report of applicable) for (const output of report.outputs ?? []) outputById.set(output.id, output)
  const outputs = [...outputById.values()]
  const latestReport = applicable[applicable.length - 1]
  return <section className="workflow-current" aria-label="Run 节点事实">
    <div className="workflow-node-workspace">
      <aside className="workflow-node-sidebar" aria-label="节点导航">
        <h3>节点</h3>
        <p className="workflow-muted">{map ? '依赖层级排序 · 同层可并行' : '按已保存节点列出 · 依赖顺序不可用'}</p>
        <nav aria-label="已保存节点状态">{ids.map(id => <button type="button" key={id} aria-pressed={nodeId === id} data-node-id={id} data-node-state={states[id].status} onClick={() => setSelected(id)}>
          <span className="workflow-node-nav-content"><strong>{map?.nodes[id]?.title ?? id}</strong><code>{id}</code><span className="workflow-node-task">{nodeTargets[id].label} · {nodeTargets[id].name}</span><WorkflowStatus status={states[id].status} saved={!managed} animate={animateRunning} /></span>
        </button>)}</nav>
        {ids.length === 0 && <p>尚无已保存节点。</p>}
      </aside>
      <div className="workflow-mobile-node-selector"><label>选择节点<select value={nodeId ?? ''} onChange={event => setSelected(event.target.value)} disabled={ids.length === 0}>
        {ids.length === 0 && <option value="">尚无已保存节点</option>}
        {ids.map((id, index) => <option value={id} key={id}>{index + 1}. {nodeNames[id]} · {nodeTargets[id].label}：{nodeTargets[id].name} · {states[id].label}</option>)}
      </select></label></div>
      <div className="workflow-node-canvas">
        <section className="workflow-graph-panel" aria-label="只读状态图">
          <header><div><h2>{map?.title || '工作流现场'}</h2></div><span className="workflow-muted">前置 → 后续 · 当前 Map 定义</span></header>
          {mapError && <p role="alert">当前 Map 不可用：{mapError} 不补造历史图。</p>}
          {map && <WorkflowRunGraph map={map} selectedId={nodeId ?? null} onSelect={setSelected} nodeStates={states} nodeTargets={nodeTargets} nodeBots={nodeBots} nodeReportPolicies={nodeReportPolicies} animateRunning={animateRunning} />}
          <p className="workflow-graph-note">图与 Bot 名称来自当前定义，非历史定义快照。有执行时目标类型与能力／Task ID 来自已保存执行，Task 名称来自当前目录；未派发时为当前配置。箭头仅表示依赖，不表示自动执行或材料传递。{!animateRunning && '当前不播放运行动画。'}</p>
        </section>
        {catalogErrors.map((message, index) => <p role="alert" key={index}>{message}</p>)}
        {node && <section aria-label="当前节点详情" className="workflow-node-details">
          <header className="workflow-node-detail-heading"><div><p className="workflow-eyebrow">NODE / {nodeId}</p><h2>{map?.nodes[nodeId]?.title ?? nodeId}</h2><p className="workflow-detail-task">{nodeTargets[nodeId].label} · {nodeTargets[nodeId].name}<small>{node.execution ? '已保存执行' : '当前配置'}</small></p></div><WorkflowStatus status={states[nodeId].status} saved={!managed} animate={animateRunning} /></header>
          <p className="workflow-muted">{node.execution ? 'reportPolicy（已保存执行）' : 'report_policy（当前配置）'}: {nodeReportPolicies[nodeId] ?? 'auto'}</p>
          <section aria-label="当前执行交接事实"><h3>当前执行交接事实</h3><p>最新 canComplete: {!latestReport ? '无报告' : latestReport.canComplete === undefined ? '缺失（最新报告未声明）' : String(latestReport.canComplete)}</p><p className="workflow-muted">仅最新适用报告的严格 true 表示可交接声明；报告不等于管理者已完成节点。</p><h4>当前执行累计 outputs</h4><OutputAddresses outputs={outputs} empty="当前执行尚无已登记输出地址。" /><p className="workflow-muted">按报告追加顺序累计；同名后报覆盖，省略保留，不继承其他执行。这里只显示地址，不读取或验证文件。</p></section>
          <section aria-label="当前执行适用报告" className="workflow-current-reports"><h3 className="workflow-eyebrow">当前执行适用报告</h3>{applicable.length ? applicable.map(report => <article key={report.id}><p className="workflow-report-meta">id: <code>{report.id}</code> · at: <FactTime at={report.at} /></p><h4>body</h4><pre className="workflow-prose">{report.body}</pre><ReportHandoff report={report} /></article>) : <p className="workflow-muted">尚无当前执行适用报告。</p>}</section>
          <div className="workflow-node-journal">
            <section className="workflow-journal-entry" aria-label="当前工作要求"><span className="workflow-journal-dot" aria-hidden="true" /><h3>input</h3>{definition ? <><p>{definition.input || '当前定义未填写工作要求。'}</p><p className="workflow-muted">来自当前 Map 定义，非历史执行快照。</p></> : <p>当前定义不可用，不补造历史工作要求。</p>}</section>
            <section className="workflow-journal-entry workflow-current-execution" aria-label="当前执行事实"><span className="workflow-journal-dot" aria-hidden="true" /><h3>执行关联</h3>
              {node.execution?.kind === 'rust_script' ? <ScriptExecutionFacts execution={node.execution} handoff={node.status === 'completed' ? 'completed' : 'pending'} /> : node.execution ? <><dl><dt>id</dt><dd><code>{node.execution.id}</code></dd><dt>taskId</dt><dd className="workflow-reference">{taskName(node.execution.taskId)}</dd><dt>zone</dt><dd className="workflow-path">{node.execution.zone || '未记录（不使用当前 Map 补造）'}</dd><dt>reportPolicy</dt><dd>{node.execution.reportPolicy ?? 'auto（缺省按 auto）'}</dd><dt>autoClose</dt><dd>{node.execution.autoClose === undefined ? 'false（缺省按 false）' : String(node.execution.autoClose)}</dd></dl><p>实际 cwd 未单独记录。</p>{node.execution.childRunId ? <p>childRunId: <code>{node.execution.childRunId}</code> · <a href={`#workflow/${encodeURIComponent(node.execution.childRunId)}`}>查看子 Run</a></p> : <p className="workflow-muted">childRunId: 未保存关联（普通执行或关联缺失）；不按名称或当前定义推测子 Run。</p>}<ExecutionHandoff execution={node.execution} /><RunChatReference label="当前执行 Chat" chat={node.execution.chat} /></> : <p>尚无当前执行或 Chat 关联；读取不会创建执行。</p>}
            </section>
            {node.completion && <section aria-label="交接完成说明" className="workflow-journal-entry workflow-completion"><span className="workflow-journal-dot" aria-hidden="true" /><h3>完成说明</h3><dl><dt>id</dt><dd><code>{node.completion.id}</code></dd><dt>reason</dt><dd>{node.completion.reason}</dd><dt>at</dt><dd><FactTime at={node.completion.at} /></dd></dl></section>}
          </div>
          {node.failure && <section aria-label="节点失败" className="workflow-failure"><h3>失败原因 · stage: {node.failure.stage}</h3><p>message: {node.failure.message}</p>{node.failure.effects && <dl>{Object.entries(node.failure.effects).map(([key, value]) => <div key={key}><dt>{key}</dt><dd>{String(value ?? '未知')}</dd></div>)}</dl>}</section>}
          <details className="workflow-node-definition" key={`definition-${nodeId}`} aria-label="当前节点定义"><summary>当前定义（非执行快照）</summary>
            {definition ? <>
              <dl><dt>bot_id</dt><dd className="workflow-reference">{bot?.name ? `${bot.name} · ${bot.id}` : definition.bot_id}</dd><dt>task_id</dt><dd className="workflow-reference">{bot?.task_id ? taskName(bot.task_id) : '当前 Bot 无可读 Task 绑定。'}</dd><dt>zone</dt><dd className="workflow-path">{definition.zone || '未绑定 zone'}</dd><dt>report_policy</dt><dd>{definition.report_policy ?? 'auto'}</dd><dt>auto_close</dt><dd>{String(definition.auto_close ?? false)}</dd><dt>input</dt><dd><pre className="workflow-prose">{definition.input}</pre></dd><dt>input_rule</dt><dd><pre>{JSON.stringify(definition.input_rule ?? [], null, 2)}</pre></dd><dt>output_rule</dt><dd><pre>{JSON.stringify(definition.output_rule ?? [], null, 2)}</pre></dd><dt>blocked_by</dt><dd>{definition.blocked_by.length ? definition.blocked_by.map(id => nodeNames[id] ?? id).join('、') : '无前置依赖。'}</dd></dl>
              {node.status === 'pending' && <p>{run.inputsReady !== true && (run.inputsReady === false ? '输入尚未确认就绪，不可派发。' : '未保存输入准备确认，按未就绪处理，不可派发。')}{blockers.length ? `前置未完成：${blockers.join('、')}` : '前置依赖已满足。'}{run.status !== 'running' && ` Run 状态为 ${run.status}，不可派发。`}</p>}
            </> : <p>当前定义不可用；仅显示保存事实。</p>}
          </details>
          <details key={`history-${nodeId}`} aria-label="已保存旧执行事实"><summary>旧执行历史（{node.previousExecutions.length} 次已保存执行）</summary>
            {node.previousExecutions.length ? node.previousExecutions.map((fact, index) => <article key={fact.execution?.id ?? index}>
              {fact.execution?.kind === 'rust_script' ? <ScriptExecutionFacts execution={fact.execution} handoff="historical" /> : fact.execution ? <><dl><dt>id</dt><dd><code>{fact.execution.id}</code></dd><dt>taskId</dt><dd>{taskName(fact.execution.taskId)}</dd><dt>zone</dt><dd>{fact.execution.zone || '未记录（不使用当前 Map 补造）'}</dd><dt>reportPolicy</dt><dd>{fact.execution.reportPolicy ?? 'auto（缺省按 auto）'}</dd><dt>autoClose</dt><dd>{fact.execution.autoClose === undefined ? 'false（缺省按 false）' : String(fact.execution.autoClose)}</dd></dl><p>实际 cwd 未单独记录。</p>{fact.execution.childRunId ? <p>childRunId: <code>{fact.execution.childRunId}</code> · <a href={`#workflow/${encodeURIComponent(fact.execution.childRunId)}`}>查看子 Run</a></p> : <p className="workflow-muted">childRunId: 未保存关联（普通执行或关联缺失）；不按名称或当前定义推测子 Run。</p>}<ExecutionHandoff execution={fact.execution} /><RunChatReference label="旧执行 Chat" chat={fact.execution.chat} /></> : <p>此旧执行未保存完整执行关联。</p>}
              {fact.failure && <p>旧失败 · stage: {fact.failure.stage} · message: {fact.failure.message}</p>}
            </article>) : <p>尚无已保存旧执行。</p>}
            {reports.filter(report => report.nodeId === nodeId && report.executionId !== node.execution?.id).map(report => <article key={report.id}><p>旧报告 · id: <code>{report.id}</code> · executionId: <code>{report.executionId}</code> · at: <FactTime at={report.at} /></p><h4>body</h4><pre className="workflow-prose">{report.body}</pre><ReportHandoff report={report} /></article>)}
          </details>
        </section>}
      </div>
    </div>
    <p className="workflow-readonly-note">节点状态不代表 Chat 在线或工作状态。只读入口不提供状态操作。需要核实或交接，请向已关联管理者 Chat 表达意图；报告不自动完成节点，普通 Chat Resume 不接管 Run。</p>
  </section>
}
