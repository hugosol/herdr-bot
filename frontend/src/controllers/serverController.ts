/**
 * 操作接口客户端：页面与接口同源，请求地址直接使用相对路径。
 *
 * 脚本目标与工作目录由服务端配置决定，请求只提交“要执行哪个固定操作”，
 * 不能携带命令、路径或额外参数。
 */

/** 页面能提交的两个固定操作名；执行目标和参数不在客户端一侧。 */
export type OperationName = 'start' | 'stop'

/** 结果类别：脚本成功退出为 `completed`，无法启动或失败退出为 `failed`。 */
export type OperationOutcome = 'completed' | 'failed'

/** 失败子类别；“无法启动”与“运行后失败退出”因此可互相区分。 */
export type FailureKind = 'not-started' | 'failed-exit'

/** 一次已结束的调用：本次调用已结束的事实、脚本进程的原始观测结果与结果类别。 */
export type OperationResult = {
  operation: OperationName
  /** 本次调用是否已经结束。 */
  ended: boolean
  /** 脚本进程是否成功创建。 */
  started: boolean
  /** 脚本进程退出码；未能创建或非正常退出结束时为 null。 */
  exitCode: number | null
  outcome: OperationOutcome
  /** 失败子类别；仅当 `outcome` 为 `failed` 时出现。 */
  failure?: FailureKind
}

/**
 * 一次操作请求在页面一侧的可能归宿。
 *
 * - `ended`：脚本调用已结束，带结果类别（完成 / 失败）。
 * - `busy`：请求被拒绝为忙碌（HTTP 409 `operation-in-progress`），本次没有执行脚本，
 *   这不是脚本执行结果，也不会被排队补做。
 *
 * 其余情形（请求没得到可用响应）抛出异常：页面不能把它当成脚本结果。
 */
export type OperationCall = { kind: 'ended'; result: OperationResult } | { kind: 'busy' }

/** 忙碌拒绝的机器可读类别码（服务端 409 响应体里的 `code`）。 */
const BUSY_CODE = 'operation-in-progress'

export async function runOperation(name: OperationName): Promise<OperationCall> {
  const response = await fetch(`/api/operations/${name}`, { method: 'POST' })

  if (response.ok) {
    return { kind: 'ended', result: (await response.json()) as OperationResult }
  }

  // 409 + `operation-in-progress` 是拒绝：没有执行脚本，因此不能按结果类别显示。
  if (response.status === 409) {
    const rejection = (await response.json().catch(() => null)) as { code?: string } | null
    if (rejection?.code === BUSY_CODE) {
      return { kind: 'busy' }
    }
  }

  throw new Error(`操作请求失败：HTTP ${response.status}`)
}
