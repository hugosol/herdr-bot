import { useEffect, useId, useState } from 'react'
import type { Bot, BotCatalog } from '../controllers/botController'
import type { WorkflowMap } from '../controllers/schedulerController'
import { GroupOptions, useGroupRefresh } from '../app/entityGroups'
import { fetchTasks, type TaskResult } from '../controllers/taskController'
import { NodeSettings, type NodeSettingsValue } from './NodeSettings'

type NodeEdit = {
  content: string
  settings: NodeSettingsValue
  create: boolean
  disabled: boolean
  onContentChange: (content: string) => void
  onSettingsChange: (patch: Partial<NodeSettingsValue>) => void
  onIdChange: (id: string) => void
}

function parseContent(content: string): { definition: Record<string, unknown> | null; error: string | null } {
  try {
    const value: unknown = JSON.parse(content)
    if (typeof value !== 'object' || value === null || Array.isArray(value)) {
      return { definition: null, error: 'node 内容必须是 JSON 对象，不能是数组、null 或其他值。请在 JSON 编辑中修正。' }
    }
    return { definition: value as Record<string, unknown>, error: null }
  } catch (error) {
    const detail = error instanceof Error ? error.message : 'JSON 格式无效'
    return { definition: null, error: `JSON 解析失败：${detail}。请检查引号、逗号和括号；原始草稿已保留，修正为有效对象后可切换表单。` }
  }
}

function TaskPrompt({ bot, botProblem }: { bot?: Bot; botProblem?: string }) {
  const [taskLookup, setTaskLookup] = useState<{ bot: Bot; result: TaskResult } | null>(null)
  useEffect(() => {
    if (!bot || botProblem || bot.task_id == null) return
    let cancelled = false
    void fetchTasks().then(result => {
      if (!cancelled) setTaskLookup({ bot, result })
    })
    return () => { cancelled = true }
  }, [bot, botProblem])
  const result = taskLookup?.bot === bot ? taskLookup?.result : null
  const task = result?.kind === 'ok' ? result.tasks.find(task => task.id === bot?.task_id) : undefined
  const taskProblem = botProblem ? 'Bot 无效，无法读取绑定 Task 的 prompt。'
    : !bot ? '尚未取得关联 Bot。'
      : bot.rust_script != null ? `固定 Rust 能力：${bot.rust_script}。不使用 Task.prompt 或 manager Task；使用命名文件材料，不消费 node.input。`
      : bot.map_file != null ? 'Map 型 Bot；使用调用节点指定的 manager Task，不绑定普通 Task。'
      : bot.task_id == null ? '尚未绑定 Task。'
        : result == null ? '正在读取 Task prompt…'
          : result.kind !== 'ok' ? `Task prompt 读取失败${result.kind === 'rejected' ? `：${result.message}` : '，请刷新重试。'}`
            : !task ? `绑定的 Task 不存在（${bot.task_id}）` : null
  return <section aria-label="Task.prompt"><h4>Task.prompt</h4>
    {taskProblem ? <p>{taskProblem}</p> : <>
      <p className="scheduler-text scheduler-reference">Task：{task!.name || '（Task 未命名）'}</p>
      {task!.prompt === '' ? <p>固定 prompt 为空。</p> : <pre className="scheduler-text scheduler-prose">{task!.prompt}</pre>}
    </>}
  </section>
}

/** Read, create and edit share the same field order and relationship frame. */
export function MapNodeDetails({ map, nodeId, catalog, onSelect, edit }: {
  map: WorkflowMap; nodeId: string; catalog?: BotCatalog
  onSelect: (id: string) => void
  edit?: NodeEdit
}) {
  const id = useId()
  const groupRevision = useGroupRefresh()
  const [jsonMode, setJsonMode] = useState(false)
  const [managerLookup, setManagerLookup] = useState<TaskResult | null>(null)
  const editing = !!edit
  useEffect(() => {
    if (!editing) return
    let cancelled = false
    void fetchTasks().then(result => { if (!cancelled) setManagerLookup(result) })
    return () => { cancelled = true }
  }, [editing, groupRevision])
  const parsed = edit ? parseContent(edit.content) : { definition: map.nodes[nodeId], error: null }
  const { definition, error } = parsed
  const showJson = !!edit && (jsonMode || definition === null)
  const title = typeof definition?.title === 'string' ? definition.title : ''
  const botId = typeof definition?.bot_id === 'string' ? definition.bot_id : ''
  const input = typeof definition?.input === 'string' ? definition.input : ''
  const dependencies: unknown[] = Array.isArray(definition?.blocked_by) ? definition.blocked_by : []
  const prerequisites = dependencies.filter((value): value is string => typeof value === 'string')
  const dependents = Object.keys(map.nodes).filter(key => map.nodes[key].blocked_by.includes(nodeId))
  const candidates = [...new Set([...Object.keys(map.nodes), ...prerequisites])].filter(key => key !== nodeId)
  const bot = catalog?.bots.find(item => item.id === botId)
  const problem = catalog?.problems?.find(item => item.bot_id === botId)
  const botProblem = catalog && botId ? problem ? `共享 Bot ${botId}：${problem.message}` : !bot ? `缺失共享 Bot：${botId}` : undefined : undefined
  const node = map.nodes[nodeId]
  const settings = edit?.settings ?? { zone: node.zone ?? '', report_policy: node.report_policy ?? 'auto', auto_close: node.auto_close ?? false }

  function changeField(field: string, value: unknown) {
    if (!edit || edit.disabled || !definition) return
    edit.onContentChange(JSON.stringify({ ...definition, [field]: value }, null, 2))
  }
  function nodeName(key: string) {
    return Object.hasOwn(map.nodes, key) ? `${map.nodes[key].title} · ${key}` : `${key}（当前节点列表中不存在）`
  }
  function relatedNodes(keys: string[], direction: '前置' | '后继') {
    return keys.length === 0 ? <p className="scheduler-relation-empty">{direction === '前置' ? '无前置依赖。' : '无后继节点。'}</p> : <ul>
      {keys.map(key => {
        const identity = <><strong>{map.nodes[key]?.title ?? '当前节点列表中不存在'}</strong><span className="scheduler-identifier">{key}</span></>
        return <li key={key}>{edit ? <span className="scheduler-relation-node">{identity}</span> : <button type="button" aria-label={`查看${direction}节点 ${map.nodes[key]?.title ?? ''} ${key}`} onClick={() => onSelect(key)}>{identity}</button>}</li>
      })}
    </ul>
  }

  return <section aria-label="node 详情" className="scheduler-details scheduler-node-content">
    <div className="scheduler-node-identity">
      <div><label htmlFor={`${id}-node-id`}>node_id</label>{edit ? <input id={`${id}-node-id`} value={nodeId} readOnly={!edit.create} disabled={edit.disabled} onChange={event => edit.onIdChange(event.target.value)} /> : <p className="scheduler-node-value scheduler-identifier">{nodeId}</p>}</div>
      <div><label htmlFor={`${id}-title`}>节点标题</label>{edit && !showJson ? <input id={`${id}-title`} value={title} disabled={edit.disabled} onChange={event => changeField('title', event.target.value)} /> : <h3 className="scheduler-node-value">{title || (showJson ? '由下方 JSON 编辑' : '未命名节点')}</h3>}</div>
    </div>
    {edit ? <NodeSettings value={settings} script={bot?.rust_script != null} disabled={edit.disabled} onChange={edit.onSettingsChange} /> : <NodeSettings value={settings} script={bot?.rust_script != null} readOnly />}
    <div className="scheduler-node-bot"><label htmlFor={`${id}-bot`}>Bot ID</label>
      {edit && !showJson ? <><input id={`${id}-bot`} value={botId} disabled={edit.disabled} onChange={event => changeField('bot_id', event.target.value)} /><select aria-label="选择已有 Bot" value={catalog?.bots.some(item => item.id === botId) ? botId : ''} disabled={edit.disabled} onChange={event => { if (event.target.value) changeField('bot_id', event.target.value) }}><option value="">选择已有 Bot</option><GroupOptions items={catalog?.bots ?? []} groupOf={item => item.group}>{item => <option key={item.id} value={item.id}>{item.name} · {item.id}</option>}</GroupOptions></select></> : <p className="scheduler-node-value scheduler-identifier">{botId || '尚未填写 Bot ID'}</p>}
      {botProblem && <p role="alert">{botProblem}</p>}
    </div>
    {(bot?.map_file != null || definition?.manager_task_id != null) && <section aria-label="Map 调用配置">
      <p className="scheduler-path">Map：{bot?.map_file ?? '当前 Bot 不是 Map 型；请修正 manager_task_id'}</p>
      <label htmlFor={`${id}-manager`}>manager Task</label>
      {bot?.rust_script != null ? <><p className="scheduler-identifier">脚本节点不能指定 manager_task_id：{String(definition?.manager_task_id)}</p>
        {edit && <button type="button" disabled={edit.disabled} onClick={() => changeField('manager_task_id', undefined)}>移除 manager_task_id</button>}
      </> : edit && !showJson ? <select id={`${id}-manager`} value={typeof definition?.manager_task_id === 'string' ? definition.manager_task_id : ''} disabled={edit.disabled || managerLookup?.kind !== 'ok'} onChange={event => changeField('manager_task_id', event.target.value || undefined)}>
        <option value="">未指定</option>
        {typeof definition?.manager_task_id === 'string' && !(managerLookup?.kind === 'ok' && managerLookup.tasks.some(task => task.id === definition.manager_task_id && task.taskType === 'manager')) && <option value={definition.manager_task_id} disabled>{definition.manager_task_id}（不存在、类型无效或尚未核实）</option>}
        {managerLookup?.kind === 'ok' && <GroupOptions items={managerLookup.tasks.filter(task => task.taskType === 'manager')} groupOf={task => task.group}>{task => <option key={task.id} value={task.id}>{task.name} · {task.id}</option>}</GroupOptions>}
      </select> : <p className="scheduler-identifier">{typeof definition?.manager_task_id === 'string' ? definition.manager_task_id : '未指定 manager Task'}</p>}
      {edit && managerLookup && managerLookup.kind !== 'ok' && <p role="status">manager Task 列表读取失败；当前配置未改变。</p>}
      <p className="scheduler-help">manager_task_id 属于本次调用节点，不保存到 Bot；仅 manager 类型 Task 可管理子 Run。</p>
    </section>}
    <div className="scheduler-node-relations">
      <section aria-label="blocked_by"><h4><span aria-hidden="true">→ </span>前置依赖 <small>blocked_by</small></h4>
        {relatedNodes(prerequisites, '前置')}
        {edit && !showJson && <details className="scheduler-dependency-picker"><summary>选择前置节点</summary>
          <fieldset disabled={edit.disabled} aria-label="前置依赖（blocked_by）">
            {candidates.length === 0 ? <p>没有其他可选节点。</p> : candidates.map(key => <label key={key} className="scheduler-setting-choice"><input type="checkbox" checked={prerequisites.includes(key)} onChange={event => changeField('blocked_by', event.target.checked ? [...dependencies, key] : dependencies.filter(value => value !== key))} /><span>{nodeName(key)}</span></label>)}
          </fieldset>
        </details>}
      </section>
      <section aria-label="依赖此节点"><h4><span aria-hidden="true">→ </span>依赖此节点 <small>后继节点</small></h4>{relatedNodes(dependents, '后继')}</section>
    </div>
    <div className="scheduler-node-reference">
      {edit && <div className="scheduler-content-mode"><span>参考内容</span><button type="button" disabled={edit.disabled || (showJson && definition === null)} onClick={() => setJsonMode(!showJson)} aria-controls={`${id}-reference`}>{showJson ? '表单编辑' : 'JSON 编辑'}</button></div>}
      <div id={`${id}-reference`}>
        {showJson && edit ? <div className="scheduler-node-content-json"><label htmlFor={`${id}-json`}>node 内容 JSON</label>
          <textarea id={`${id}-json`} value={edit.content} disabled={edit.disabled} rows={18} spellCheck={false} aria-invalid={error !== null} aria-describedby={`${id}-json-help${error ? ` ${id}-json-error` : ''}`} onChange={event => { setJsonMode(true); edit.onContentChange(event.target.value) }} />
          <p id={`${id}-json-help`} className="scheduler-field-help">可编辑 input_rule、output_rule 及其他内容字段。无效 JSON 不会被自动修复或覆盖；有效对象可切换回表单。</p>
          {error && <p id={`${id}-json-error`} className="scheduler-node-content-error" role="status">{error}</p>}
        </div> : <section aria-label="input"><h4>输入内容 · input</h4>
          {edit ? <textarea id={`${id}-input`} aria-label="输入内容" value={input} disabled={edit.disabled} rows={6} onChange={event => changeField('input', event.target.value)} /> : <>{input === '' && <p>未填写输入内容。</p>}<pre className="scheduler-text scheduler-prose" data-testid="node-input">{input}</pre></>}
        </section>}
        <TaskPrompt bot={bot} botProblem={botProblem} />
        {(['input_rule', 'output_rule'] as const).map(field => <section key={field} aria-label={field}><h4>{field}</h4><pre className="scheduler-text">{definition ? JSON.stringify(definition[field] ?? [], null, 2) : 'JSON 无效，无法预览。'}</pre></section>)}
      </div>
    </div>
  </section>
}
