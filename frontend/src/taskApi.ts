/**
 * Task 接口客户端：页面与接口同源，请求地址直接使用相对路径。
 *
 * Task 由后端持久保存，电脑与手机读取同一份；页面不缓存 Task，也不自行生成固定选项。
 * 能力事实（agent / model / thinking / skill）由后端公开，保存失败时页面不能表示已保存。
 */

export type TaskType = 'normal' | 'manager' | 'node'
export type TaskAgent = 'omp' | 'pi'
export type TaskModel = string
export type TaskThinking = string
export type TaskSkill = string

/** 一个已保存的 Task；字段与后端存储一致。 */
export type Task = {
  id: string
  name: string
  chatName: string
  agent: TaskAgent
  taskType: TaskType
  model: TaskModel
  thinking: TaskThinking
  skill: TaskSkill | null
  prompt: string
  extensions: string[]
}

/** 新增或编辑时提交的字段（标识由后端维护）。 */
export type TaskInput = Omit<Task, 'id'>

/** 当前元数据选项；`thinking[agent][model]` 是显式级别，`default` 模型对应空数组。 */
export type TaskOptions = {
  agents: TaskAgent[]
  models: TaskModel[]
  thinking: Record<string, Record<string, TaskThinking[]>>
  skills: TaskSkill[]
  labels?: Record<string, Record<string, string>>
  extensions?: { id: string; label: string; agent: TaskAgent; kind: 'fixed' | 'optional'; taskType: TaskType | null; entrypoint: string }[]
}

export type TaskList = {
  tasks: Task[]
  options: TaskOptions
}

/**
 * 一次 Task 请求在页面一侧的可能归宿。
 *
 * - `ok`：后端返回了当前完整列表；只有这时才能更新界面或显示成功。
 * - `rejected`：后端给出了明确的失败类别与说明（字段非法、存储失败等）。
 * - `unreachable`：没有得到可用响应（网络中断或响应无法解析），不能当成成功或失败。
 */
export type TaskResult =
  | { kind: 'ok'; tasks: Task[]; options: TaskOptions }
  | { kind: 'rejected'; code: string; message: string }
  | { kind: 'unreachable' }

function isStringArray(value: unknown): value is string[] {
  return Array.isArray(value) && value.every((entry) => typeof entry === 'string')
}

function isTask(value: unknown): value is Task {
  if (typeof value !== 'object' || value === null) return false
  const task = value as Record<string, unknown>
  return (
    typeof task.id === 'string' &&
    typeof task.name === 'string' &&
    typeof task.chatName === 'string' &&
    typeof task.agent === 'string' &&
    ['normal', 'manager', 'node'].includes(task.taskType as string) &&
    typeof task.model === 'string' &&
    typeof task.thinking === 'string' &&
    (task.skill === null || typeof task.skill === 'string') &&
    typeof task.prompt === 'string' &&
    isStringArray(task.extensions)
  )
}

function isOptions(value: unknown): value is TaskOptions {
  if (typeof value !== 'object' || value === null) return false
  const options = value as Record<string, unknown>
  return (
    isStringArray(options.agents) &&
    isStringArray(options.models) &&
    isStringArray(options.skills) &&
    typeof options.thinking === 'object' &&
    options.thinking !== null
  )
}

function isTaskList(value: unknown): value is TaskList {
  if (typeof value !== 'object' || value === null) return false
  const payload = value as Record<string, unknown>
  return (
    Array.isArray(payload.tasks) &&
    payload.tasks.every(isTask) &&
    isOptions(payload.options)
  )
}

function isError(value: unknown): value is { code: string; message: string } {
  return (
    typeof value === 'object' &&
    value !== null &&
    'code' in value &&
    typeof value.code === 'string' &&
    'message' in value &&
    typeof value.message === 'string'
  )
}

/**
 * 发出一次 Task 请求并归一化答复。
 *
 * 只有 HTTP 成功且响应形状正确才算 `ok`；明确的错误码与说明原样交给界面；
 * 其余情形一律是 `unreachable`，不能显示成成功。
 */
async function taskRequest(
  input: RequestInfo | URL,
  init?: RequestInit,
): Promise<TaskResult> {
  let response: Response
  try {
    response = await fetch(input, init)
  } catch {
    return { kind: 'unreachable' }
  }

  let payload: unknown
  try {
    payload = await response.json()
  } catch {
    payload = null
  }

  if (response.ok) {
    return isTaskList(payload)
      ? {
          kind: 'ok',
          tasks: payload.tasks,
          options: payload.options,
        }
      : { kind: 'unreachable' }
  }

  return isError(payload)
    ? { kind: 'rejected', code: payload.code, message: payload.message }
    : { kind: 'unreachable' }
}

function jsonInit(method: string, body: TaskInput): RequestInit {
  return {
    method,
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  }
}

/** 读取当前 Task 列表与固定选项；每次进入该 tab 都重新读取后端。 */
export function fetchTasks(): Promise<TaskResult> {
  return taskRequest('/api/tasks')
}

/** 新增一个 Task；成功只意味着后端已经持久化。 */
export function createTask(input: TaskInput): Promise<TaskResult> {
  return taskRequest('/api/tasks', jsonInit('POST', input))
}

/** 编辑一个 Task；标识不变，成功只意味着后端已经持久化。 */
export function updateTask(
  id: string,
  input: TaskInput,
): Promise<TaskResult> {
  return taskRequest(`/api/tasks/${encodeURIComponent(id)}`, jsonInit('PUT', input))
}

/** 删除一个 Task；只移除 Task，不改变任何已经发起的 Chat。 */
export function deleteTask(id: string): Promise<TaskResult> {
  return taskRequest(`/api/tasks/${encodeURIComponent(id)}`, {
    method: 'DELETE',
  })
}
