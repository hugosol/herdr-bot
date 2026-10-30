import { useEffect, useState } from 'react'
import { fetchTasks, type Task } from '../tasks/taskApi'

type Props = { content: string; disabled: boolean; onChange: (content: string) => void }

export function BotTaskSelect({ content, disabled, onChange }: Props) {
  const [tasks, setTasks] = useState<Task[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    let cancelled = false
    void fetchTasks().then(result => {
      if (cancelled) return
      setLoading(false)
      if (result.kind === 'ok') setTasks(result.tasks)
      else setError(result.kind === 'rejected' ? result.message : 'Task 列表读取失败；请保留草稿，重新打开编辑器后再选择。')
    })
    return () => { cancelled = true }
  }, [])

  let definition: Record<string, unknown> | null = null
  let jsonError: string | null = null
  try {
    const value: unknown = JSON.parse(content)
    if (typeof value !== 'object' || value === null || Array.isArray(value)) {
      jsonError = '请先将 Bot 内容修正为 JSON 对象，再选择 Task。'
    } else {
      definition = value as Record<string, unknown>
      if (definition.task_id != null && typeof definition.task_id !== 'string') {
        jsonError = '请先修正 JSON 中的 task_id：必须是字符串、null 或省略。'
      }
    }
  } catch {
    jsonError = '请先修正 Bot 内容 JSON，再选择 Task；原始草稿保持不变。'
  }

  const binding = typeof definition?.task_id === 'string' ? definition.task_id : null
  const selectedTask = tasks.find(task => task.id === binding)
  const invalidBinding = binding !== null && (!selectedTask || selectedTask.taskType !== 'normal')
  const invalidLabel = selectedTask ? `${selectedTask.name} · ${binding}（非 normal Task，不可执行）` : `${binding || '空 task_id'}（Task 不存在或不可用）`

  function change(value: string) {
    if (!definition || jsonError) return
    const updated = { ...definition }
    if (value === '') delete updated.task_id
    else updated.task_id = value.slice('task:'.length)
    onChange(JSON.stringify(updated, null, 2))
  }

  return <div>
    <label htmlFor="bot-task-binding">Task
      <select id="bot-task-binding" value={binding === null ? '' : `task:${binding}`} disabled={disabled || loading || error !== null || jsonError !== null} onChange={event => change(event.target.value)} aria-describedby="bot-task-binding-help">
        <option value="">未绑定</option>
        {invalidBinding && <option value={`task:${binding}`} disabled>{loading || error ? `${binding || '空 task_id'}（尚未核实）` : invalidLabel}</option>}
        {tasks.filter(task => task.taskType === 'normal').map(task => <option key={task.id} value={`task:${task.id}`}>{task.name} · {task.id}</option>)}
      </select>
    </label>
    <p id="bot-task-binding-help">仅引用普通（normal）Task，不复制其配置。可保持未绑定并保存；执行前必须绑定有效 Task。选择与下方 JSON 中的 task_id 同步。</p>
    {loading && <p role="status">正在读取 Task 列表…</p>}
    {error && <p role="status">{error} 当前引用和 JSON 草稿未改变。</p>}
    {jsonError && <p>{jsonError}</p>}
    {!loading && !error && invalidBinding && <p>当前引用无效：{invalidLabel}。不会自动替换；可保留保存，或主动选择其他 Task／未绑定。</p>}
  </div>
}
