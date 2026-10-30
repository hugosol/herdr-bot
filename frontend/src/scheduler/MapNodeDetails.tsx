import { useEffect, useState } from 'react'
import type { Bot, WorkflowMap } from './schedulerApi'
import { fetchTasks, type TaskResult } from '../tasks/taskApi'

export function MapNodeDetails({ map, nodeId, botProblem, bot, onSelect, onBot }: {
  map: WorkflowMap; nodeId: string; botProblem?: string; bot?: Bot
  onSelect: (id: string) => void
  onBot: (id: string) => void
}) {
  const node = map.nodes[nodeId]
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
      : bot.task_id == null ? '尚未绑定 Task。'
        : result == null ? '正在读取 Task prompt…'
          : result.kind !== 'ok' ? `Task prompt 读取失败${result.kind === 'rejected' ? `：${result.message}` : '，请刷新重试。'}`
            : !task ? `绑定的 Task 不存在（${bot.task_id}）` : null
  const dependents = Object.keys(map.nodes).filter((id) => map.nodes[id].blocked_by.includes(nodeId))
  return <section aria-label="node 详情" className="scheduler-details">
    <h3>{node.title} · {nodeId}</h3>
    <p className="scheduler-text">bot_id：{node.bot_id}</p>
    <p className="scheduler-text">report_policy: {node.report_policy ?? 'auto'}</p>
    {botProblem && <p role="alert">{botProblem}</p>}
    <p className="scheduler-text">zone 路径引用：{node.zone || '尚未绑定 zone'}</p>
    <section aria-label="Task prompt">
      <h4>Task prompt（固定提示词）</h4>
      {taskProblem ? <p>{taskProblem}</p> : <>
        <p className="scheduler-text">Task：{task!.name || '（Task 未命名）'}</p>
        {task!.prompt === '' ? <p>固定 prompt 为空。</p> : <pre className="scheduler-text">{task!.prompt}</pre>}
      </>}
    </section>
    <section aria-label="本次输入">
      <h4>node.input（本次输入）</h4>
      <pre className="scheduler-text" data-testid="node-input">{node.input}</pre>
    </section>
    <section aria-label="前置依赖">
      <h4>前置依赖</h4>
      {node.blocked_by.length === 0 ? <p>无前置依赖。</p> : <ul>
        {node.blocked_by.map((id) => <li key={id}><button type="button" aria-label={`查看前置节点 ${map.nodes[id].title} ${id}`} onClick={() => onSelect(id)}>{map.nodes[id].title} · {id}</button></li>)}
      </ul>}
    </section>
    <section aria-label="依赖此节点">
      <h4>依赖此节点</h4>
      {dependents.length === 0 ? <p>无后继节点。</p> : <ul>{dependents.map((id) => <li key={id}>
        <button type="button" aria-label={`查看后继节点 ${map.nodes[id].title} ${id}`} onClick={() => onSelect(id)}>{map.nodes[id].title} · {id}</button>
      </li>)}</ul>}
    </section>
    <button type="button" onClick={() => onBot(node.bot_id)}>Bot 明细</button>
  </section>
}
