import { useEffect, useState } from 'react'
import { GroupOptions, useGroupRefresh } from '../app/entityGroups'
import { fetchTasks, type Task } from '../tasks/taskApi'
import { listMaps, type MapCatalog } from './schedulerApi'

type Props = { content: string; disabled: boolean; onChange: (content: string) => void }

export function BotTaskSelect({ content, disabled, onChange }: Props) {
  const groupRevision = useGroupRefresh()
  const [tasks, setTasks] = useState<Task[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [maps, setMaps] = useState<MapCatalog | null>(null)
  const [mapError, setMapError] = useState<string | null>(null)

  useEffect(() => {
    let cancelled = false
    void fetchTasks().then(result => {
      if (cancelled) return
      setLoading(false)
      if (result.kind === 'ok') setTasks(result.tasks)
      else setError(result.kind === 'rejected' ? result.message : 'Task 列表读取失败；请保留草稿，重新打开编辑器后再选择。')
    })
    void listMaps().then(catalog => { if (!cancelled) setMaps(catalog) }).catch(error => {
      if (!cancelled) setMapError(error instanceof Error ? error.message : 'Map 列表读取失败。')
    })
    return () => { cancelled = true }
  }, [groupRevision])

  let definition: Record<string, unknown> | null = null
  let jsonError: string | null = null
  try {
    const value: unknown = JSON.parse(content)
    if (typeof value !== 'object' || value === null || Array.isArray(value)) {
      jsonError = '请先将 Bot 内容修正为 JSON 对象，再选择 Task。'
    } else {
      definition = value as Record<string, unknown>
      if (['task_id', 'map_file'].some(key => definition![key] != null && typeof definition![key] !== 'string')) {
        jsonError = '请先修正 JSON 中的 task_id / map_file：必须是字符串、null 或省略。'
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
    else { updated.task_id = value.slice('task:'.length); delete updated.map_file }
    onChange(JSON.stringify(updated, null, 2))
  }
  const mapBinding = typeof definition?.map_file === 'string' ? definition.map_file : ''
  function changeMap(value: string) {
    if (!definition || jsonError) return
    const updated = { ...definition }
    if (value === '') delete updated.map_file
    else { updated.map_file = value; delete updated.task_id }
    onChange(JSON.stringify(updated, null, 2))
  }

  return <div>
    <label htmlFor="bot-task-binding">Task
      <select id="bot-task-binding" value={binding === null ? '' : `task:${binding}`} disabled={disabled || loading || error !== null || jsonError !== null} onChange={event => change(event.target.value)} aria-describedby="bot-task-binding-help">
        <option value="">未绑定</option>
        {invalidBinding && <option value={`task:${binding}`} disabled>{loading || error ? `${binding || '空 task_id'}（尚未核实）` : invalidLabel}</option>}
        <GroupOptions items={tasks.filter(task => task.taskType === 'normal')} groupOf={task => task.group}>{task => <option key={task.id} value={`task:${task.id}`}>{task.name} · {task.id}</option>}</GroupOptions>
      </select>
    </label>
    <label htmlFor="bot-map-binding">Map
      <select id="bot-map-binding" value={mapBinding} disabled={disabled || maps === null || jsonError !== null} onChange={event => changeMap(event.target.value)}>
        <option value="">未绑定</option>
        {mapBinding && !maps?.files.includes(mapBinding) && <option value={mapBinding} disabled>{mapBinding}（Map 不存在或尚未核实）</option>}
        <GroupOptions items={maps?.files ?? []} groupOf={file => maps?.groups?.[file]}>{file => <option key={file} value={file}>{maps?.titles?.[file] || file} · {file}</option>}</GroupOptions>
      </select>
    </label>
    <p id="bot-task-binding-help">仅引用普通（normal）Task 或 Map，不复制配置，两者互斥。可保持未绑定并保存；执行前必须绑定有效目标。选择与下方 JSON 中的 task_id / map_file 同步；Map 的 manager Task 由调用节点指定。</p>
    {definition?.task_id != null && definition?.map_file != null && <p>task_id 与 map_file 互斥；双绑定不会保存或按优先级执行。</p>}
    {mapError && <p role="status">{mapError} 当前引用和 JSON 草稿未改变。</p>}
    {loading && <p role="status">正在读取 Task 列表…</p>}
    {error && <p role="status">{error} 当前引用和 JSON 草稿未改变。</p>}
    {jsonError && <p>{jsonError}</p>}
    {!loading && !error && invalidBinding && <p>当前引用无效：{invalidLabel}。不会自动替换；可保留保存，或主动选择其他 Task／未绑定。</p>}
  </div>
}
