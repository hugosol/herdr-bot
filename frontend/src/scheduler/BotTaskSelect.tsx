import { useEffect, useState } from 'react'
import { GroupOptions, useGroupRefresh } from '../app/entityGroups'
import { fetchTasks, type Task } from '../tasks/controller'
import { listMaps, type MapCatalog } from './controller'

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
      jsonError = '请先将 Bot 内容修正为 JSON 对象，再选择执行目标。'
    } else {
      definition = value as Record<string, unknown>
      if (['task_id', 'map_file', 'rust_script'].some(key => definition![key] != null && typeof definition![key] !== 'string')) {
        jsonError = '请先修正 JSON 中的 task_id / map_file / rust_script：必须是字符串、null 或省略。'
      }
    }
  } catch {
    jsonError = '请先修正 Bot 内容 JSON，再选择执行目标；原始草稿保持不变。'
  }

  const binding = typeof definition?.task_id === 'string' ? definition.task_id : null
  const selectedTask = tasks.find(task => task.id === binding)
  const invalidBinding = binding !== null && (!selectedTask || selectedTask.taskType !== 'normal')
  const invalidLabel = selectedTask ? `${selectedTask.name} · ${binding}（非 normal Task，不可执行）` : `${binding || '空 task_id'}（Task 不存在或不可用）`

  function changeTarget(field: 'task_id' | 'map_file' | 'rust_script', value: string) {
    if (!definition || jsonError) return
    const updated = { ...definition }
    if (value === '') delete updated[field]
    else {
      delete updated.task_id
      delete updated.map_file
      delete updated.rust_script
      updated[field] = value
    }
    onChange(JSON.stringify(updated, null, 2))
  }
  const mapBinding = typeof definition?.map_file === 'string' ? definition.map_file : ''
  const scriptBinding = typeof definition?.rust_script === 'string' ? definition.rust_script : ''

  return <div>
    <label htmlFor="bot-task-binding">Task
      <select id="bot-task-binding" value={binding === null ? '' : `task:${binding}`} disabled={disabled || loading || error !== null || jsonError !== null} onChange={event => changeTarget('task_id', event.target.value.slice('task:'.length))} aria-describedby="bot-task-binding-help">
        <option value="">未绑定</option>
        {invalidBinding && <option value={`task:${binding}`} disabled>{loading || error ? `${binding || '空 task_id'}（尚未核实）` : invalidLabel}</option>}
        <GroupOptions items={tasks.filter(task => task.taskType === 'normal')} groupOf={task => task.group}>{task => <option key={task.id} value={`task:${task.id}`}>{task.name} · {task.id}</option>}</GroupOptions>
      </select>
    </label>
    <label htmlFor="bot-map-binding">Map
      <select id="bot-map-binding" value={mapBinding} disabled={disabled || maps === null || jsonError !== null} onChange={event => changeTarget('map_file', event.target.value)}>
        <option value="">未绑定</option>
        {mapBinding && !maps?.files.includes(mapBinding) && <option value={mapBinding} disabled>{mapBinding}（Map 不存在或尚未核实）</option>}
        <GroupOptions items={maps?.files ?? []} groupOf={file => maps?.groups?.[file]}>{file => <option key={file} value={file}>{maps?.titles?.[file] || file} · {file}</option>}</GroupOptions>
      </select>
    </label>
    <label htmlFor="bot-script-binding">固定 Rust 能力
      <select id="bot-script-binding" value={scriptBinding} disabled={disabled || jsonError !== null} onChange={event => changeTarget('rust_script', event.target.value)}>
        <option value="">未绑定</option>
        <option value="create_blank_bot">空白 Bot 创建 · create_blank_bot</option>
        <option value="save_bot_guides">创建分支三指南合并保存 · save_bot_guides</option>
        {scriptBinding && !['create_blank_bot', 'save_bot_guides'].includes(scriptBinding) && <option value={scriptBinding} disabled>{scriptBinding}（未知能力）</option>}
      </select>
    </label>
    <p id="bot-task-binding-help">仅引用普通（normal）Task、Map 或固定 Rust 能力，不复制配置，三者互斥。可保持未绑定并保存；执行前必须绑定有效目标。选择与下方 JSON 中的 task_id / map_file / rust_script 同步；Map 的 manager Task 由调用节点指定。</p>
    {scriptBinding && <p>脚本仅在 Map 节点执行；使用命名文件材料，不消费 node.input，不创建 Chat，固定自动报告且无需关闭会话。</p>}
    {['task_id', 'map_file', 'rust_script'].filter(key => definition?.[key] != null).length > 1 && <p>task_id、map_file 与 rust_script 互斥；冲突绑定不会保存或按优先级执行。</p>}
    {mapError && <p role="status">{mapError} 当前引用和 JSON 草稿未改变。</p>}
    {loading && <p role="status">正在读取 Task 列表…</p>}
    {error && <p role="status">{error} 当前引用和 JSON 草稿未改变。</p>}
    {jsonError && <p>{jsonError}</p>}
    {!loading && !error && invalidBinding && <p>当前引用无效：{invalidLabel}。不会自动替换；可保留保存，或主动选择其他 Task／未绑定。</p>}
  </div>
}
