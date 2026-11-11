import { useEffect, useState } from 'react'
import { MapInputRequirements } from './MapInputRequirements'
import { GroupOptions, useGroupRefresh } from '../app/entityGroups'
import { fetchTasks, type Task } from '../tasks/taskApi'
import { fetchZones, type ZoneRegistration } from '../zones/zoneApi'
import { preflightRun, startRun, type MapFileInput, type PreflightIssue, type RunPreflightInput, type RunPreflightResult } from './schedulerApi'

type Props = {
  file: string
  declarations?: MapFileInput[]
  requirements?: string
  active: boolean
  onClose: () => void
  onNode: (id: string) => void
  onBot: (id: string) => void
  onTask: (id?: string) => void
  onZones: () => void
}

/** Preflight and launch share the backend's whole-Map admission check. */
export function RunLaunchForm({ file, declarations = [], requirements = '', active, onClose, onNode, onBot, onTask, onZones }: Props) {
  const groupRevision = useGroupRefresh()
  const [tasks, setTasks] = useState<Task[]>([])
  const [zones, setZones] = useState<ZoneRegistration[]>([])
  const [input, setInput] = useState<RunPreflightInput>({ managerTaskId: '', zone: '', userInput: '' })
  const [loading, setLoading] = useState(true)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [result, setResult] = useState<RunPreflightResult | null>(null)
  const [revision, setRevision] = useState(0)

  useEffect(() => {
    if (!active) return
    let cancelled = false
    setLoading(true)
    setError(null)
    void Promise.all([fetchTasks(), fetchZones()]).then(([taskResult, zoneResult]) => {
      if (cancelled) return
      setLoading(false)
      if (taskResult.kind !== 'ok' || zoneResult.kind !== 'ok') {
        setError([taskResult, zoneResult].filter(value => value.kind !== 'ok').map(value => value.kind === 'rejected' ? value.message : '配置读取未取得有效响应，请手动刷新选项。').join('；'))
        return
      }
      setTasks(taskResult.tasks.filter(task => task.taskType === 'manager'))
      setZones(zoneResult.zones)
      setResult(null)
    })
    return () => { cancelled = true }
  }, [active, revision, groupRevision])

  function change(patch: Partial<RunPreflightInput>) {
    setInput(value => ({ ...value, ...patch }))
    setResult(null)
    setError(null)
  }

  async function check() {
    if (busy || loading) return
    setBusy(true)
    setError(null)
    setResult(null)
    try { setResult(await preflightRun(file, input)) }
    catch (failure) { setError(failure instanceof Error ? failure.message : '启动预检未取得有效响应；不会自动重试。') }
    finally { setBusy(false) }
  }

  async function launch() {
    if (busy || loading) return
    setBusy(true)
    setError(null)
    setResult(null)
    try {
      const result = await startRun(file, input)
      if ('id' in result) window.location.hash = `workflow/${encodeURIComponent(result.id)}`
      else setResult(result)
    } catch (failure) { setError(failure instanceof Error ? failure.message : '启动未取得有效响应；不会自动重试，请先核实现场。') }
    finally { setBusy(false) }
  }

  function correction(issue: PreflightIssue) {
    if (issue.target === 'run-input') return <button type="button" onClick={() => document.getElementById(`run-file-input-${issue.inputId}`)?.focus()}>修正文件输入 {issue.inputId}</button>
    if (issue.taskId) return <>
      <button type="button" onClick={() => onTask(issue.taskId)}>修正 Task {issue.taskId}</button>
      {issue.botId && <button type="button" onClick={() => onBot(issue.botId!)}>修正 Bot {issue.botId}</button>}
    </>
    if (issue.code.startsWith('zone-')) return <>
      <button type="button" onClick={onZones}>管理 zone 登记</button>
      {issue.nodeId && <button type="button" onClick={() => onNode(issue.nodeId!)}>修正 node {issue.nodeId}</button>}
    </>
    if (issue.botId) return <button type="button" onClick={() => onBot(issue.botId!)}>修正 Bot {issue.botId}</button>
    if (issue.nodeId) return <button type="button" onClick={() => onNode(issue.nodeId!)}>修正 node {issue.nodeId}</button>
    if (issue.target === 'task') return <button type="button" onClick={() => onTask()}>管理 Task</button>
    if (issue.target === 'zone') return <button type="button" onClick={onZones}>管理 zone 登记</button>
    return <button type="button" onClick={onClose}>返回 Map 修正</button>
  }

  const workspaceName = zones.find(zone => zone.id === input.zone)?.path.replace(/[\\/]+$/, '').split(/[\\/]/).at(-1) ?? ''
  return <form className="scheduler-run-form" aria-label="发起 Run" onSubmit={event => { event.preventDefault(); void check() }}>
    <h3>发起 Run · {file}</h3>
    <p>选择管理者配置并检查整张 Map。校验不创建 Run 或 Chat；启动 Run 会再次预检，保存本次要求并启动真实管理者，取得标识后进入只读详情。</p>
    <label>管理者 Task<select aria-label="管理者 Task" value={input.managerTaskId} disabled={loading || busy} onChange={event => change({ managerTaskId: event.target.value })}>
      <option value="">请选择管理者 Task</option>
      <GroupOptions items={tasks} groupOf={task => task.group}>{task => <option key={task.id} value={task.id}>{task.name}</option>}</GroupOptions>
    </select></label>
    {!loading && tasks.length === 0 && <p>没有可选的 manager 类型 Task。<button type="button" onClick={() => onTask()}>管理 Task</button></p>}
    <label>管理者 zone<select aria-label="管理者 zone" value={input.zone} disabled={loading || busy} onChange={event => change({ zone: event.target.value })}>
      <option value="">请选择已登记 zone</option>
      {zones.map(zone => <option key={zone.id} value={zone.id}>{zone.id} · {zone.path}</option>)}
    </select></label>
    {!loading && zones.length === 0 && <p>尚未登记 zone。<button type="button" onClick={onZones}>管理 zone 登记</button></p>}
    <label>Workspace<input value={workspaceName} readOnly /></label>
    <p>Workspace 从管理者 zone 目录名推导；不选择当前或默认 Workspace。实际目标可用性在启动时确认。</p>
    <MapInputRequirements content={requirements} />
    <p>可在本次要求中描述材料，或启动后与管理者对话补充文字、文件和澄清。管理者确认输入就绪后才会派发业务节点。</p>
    <label>本次要求<textarea value={input.userInput} disabled={busy} onChange={event => change({ userInput: event.target.value })} placeholder="可空，支持 Markdown；不改写 node 输入或 Task 固定指示。" /></label>
    <fieldset disabled={busy} aria-label="Run 文件输入绑定"><legend>本次文件输入</legend>
      <p>启动时所有文件均可留空，包括执行前必需项。已有文件可填写服务器绝对路径，与本次要求分别保存；也可稍后与管理者澄清、补充并确认绑定。提供路径不代表已读取或理解文件；不自动找文件，不生成默认地址，换文件不修改 Map。</p>
      {declarations.length === 0 && <p>此 Map 无文件输入声明，无需绑定文件。</p>}
      {declarations.map(declaration => <label key={declaration.id}>文件输入 {declaration.id}（{declaration.required ? '执行前必需，启动可留空' : '可选'}）<input
        id={`run-file-input-${declaration.id}`} value={input.fileInputs?.find(binding => binding.id === declaration.id)?.path ?? ''}
        onChange={event => change({ fileInputs: [
          ...(input.fileInputs ?? []).filter(binding => binding.id !== declaration.id),
          ...(event.target.value === '' ? [] : [{ id: declaration.id, path: event.target.value }]),
        ] })} placeholder="服务器已有文件的绝对路径" /></label>)}
    </fieldset>
    {loading && <p role="status">正在读取启动选项……</p>}
    {error && <p role="alert">{error}</p>}
    {result?.kind === 'rejected' && <div role="alert"><p>{result.message}</p><ul>{result.issues.map((issue, index) => <li key={index}>
      <p>{issue.nodeId && `node ${issue.nodeId}：`}{issue.message}</p><div className="scheduler-actions">{correction(issue)}</div>
    </li>)}</ul></div>}
    {result?.kind === 'valid' && <p role="status">配置校验通过，Workspace：{result.workspaceName}。尚未启动，未创建 Run 或 Chat。</p>}
    <div className="scheduler-actions">
      <button type="button" disabled={loading || busy} onClick={() => void launch()}>启动 Run</button>
      <button type="submit" disabled={loading || busy}>{busy ? '正在处理……' : '校验启动配置'}</button>
      <button type="button" disabled={loading || busy} onClick={() => setRevision(value => value + 1)}>刷新启动选项</button>
      <button type="button" disabled={busy} onClick={onClose}>返回 Map</button>
    </div>
  </form>
}
