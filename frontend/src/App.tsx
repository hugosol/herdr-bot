import { useState } from 'react'

import { runOperation, type FailureKind, type OperationName, type OperationResult } from './api'
import { ShutdownControl } from './ShutdownControl'

type PanelState =
  | { kind: 'idle' }
  | { kind: 'running'; operation: OperationName }
  | { kind: 'ended'; result: OperationResult }
  | { kind: 'busy'; operation: OperationName }
  | { kind: 'unreachable' }

/** 两个固定操作在界面上的名字；只说明“正在执行哪一个”，不代表任何执行目标。 */
const OPERATION_LABELS: Record<OperationName, string> = {
  start: '启动',
  stop: '停止',
}

/**
 * 结果反馈的类别。它只描述脚本调用本身：调用是否结束、成功还是失败，
 * 或者本次请求是否因“已有脚本调用在执行”而未被接受（忙碌）。
 */
type FeedbackKind = 'idle' | 'completed' | 'failed' | 'busy' | 'unreachable'

type Feedback = {
  kind: FeedbackKind
  text: string
  /** 失败子类别（无法启动 / 失败退出）；只有失败反馈带这个标识。 */
  failure?: FailureKind
}

export default function App() {
  const [state, setState] = useState<PanelState>({ kind: 'idle' })
  // 关机流程整屏接管期间，底层“启动/停止”不可操作（关机本身与它们的执行状态无关）。
  const [shutdownTakeover, setShutdownTakeover] = useState(false)

  async function submit(operation: OperationName) {
    setState({ kind: 'running', operation })
    try {
      const call = await runOperation(operation)
      setState(call.kind === 'busy' ? { kind: 'busy', operation } : { kind: 'ended', result: call.result })
    } catch {
      // 请求没得到响应：不能把它当成脚本失败，也不能自动重试。
      setState({ kind: 'unreachable' })
    }
  }

  // 只有页面自己发起的请求还在进行中时才算执行中：这期间两个操作按钮一起禁用，
  // 执行中提示与请求结束后的结果反馈各自占一处，不互相覆盖成同一条反馈。
  const pending = state.kind === 'running' ? state.operation : null
  const feedback = feedbackFor(state)

  return (
    <main className="panel">
      <h1>omp-bot 控制面板</h1>

      <p className="scope-note">
        本页面只反馈脚本调用情况，不表示后台程序已经启动、已经就绪或已经彻底停止。
      </p>

      <div className="operations">
        <button
          type="button"
          className="operation"
          data-testid="start-button"
          onClick={() => submit('start')}
          disabled={pending !== null || shutdownTakeover}
        >
          启动
        </button>
        <button
          type="button"
          className="operation"
          data-testid="stop-button"
          onClick={() => submit('stop')}
          disabled={pending !== null || shutdownTakeover}
        >
          停止
        </button>
      </div>

      <ShutdownControl onTakeoverChange={setShutdownTakeover} />

      {pending !== null && (
        <p className="pending" data-testid="pending-hint" role="status">
          正在执行{OPERATION_LABELS[pending]}脚本……本次操作尚未结束。
        </p>
      )}

      {/* 结果反馈：只在请求结束后出现（执行中另由 pending 提示），因此与执行中提示不是同一类反馈。
          data-outcome 是机器可读的结果类别，data-failure 是失败子类别。 */}
      {feedback !== null && (
        <p
          className="status"
          data-testid="operation-result"
          data-outcome={feedback.kind}
          data-failure={feedback.failure}
          role="status"
        >
          {feedback.text}
        </p>
      )}
    </main>
  )
}

/**
 * 结果反馈：把一次请求的归宿翻译成消费者可见的类别与文字。
 *
 * 只描述脚本调用本身——本次调用结束了没有、脚本执行成功还是失败、请求是否因忙碌未被接受；
 * 不推断也不声称后台程序或服务是否可用。
 *
 * 返回 `null` 表示请求尚未结束：此时页面只有执行中提示，没有结果反馈。
 */
function feedbackFor(state: PanelState): Feedback | null {
  switch (state.kind) {
    case 'idle':
      return { kind: 'idle', text: '尚未提交操作。' }
    case 'running':
      return null
    case 'ended':
      return endedFeedback(state.result)
    case 'busy': {
      const label = OPERATION_LABELS[state.operation]
      return {
        kind: 'busy',
        text: `${label}操作未执行：操作进行中，已有脚本调用正在执行。本页面不会自动重试，也不会在这次调用结束后补做。`,
      }
    }
    case 'unreachable':
      return { kind: 'unreachable', text: '无法确认本次调用结果：操作请求没有得到响应。' }
  }
}

/** 已结束的调用：按结果类别给出“执行完成”或“执行失败”，失败时再说明是哪一类失败。 */
function endedFeedback(result: OperationResult): Feedback {
  const label = OPERATION_LABELS[result.operation]

  if (result.outcome === 'completed') {
    return {
      kind: 'completed',
      text: `${label}脚本调用已结束：脚本执行完成（退出码 ${result.exitCode ?? '未知'}）。`,
    }
  }

  if (result.failure === 'not-started') {
    return {
      kind: 'failed',
      failure: 'not-started',
      text: `${label}脚本调用失败：脚本没能启动，本次调用没有执行脚本。`,
    }
  }

  return {
    kind: 'failed',
    failure: 'failed-exit',
    text:
      result.exitCode === null
        ? `${label}脚本调用失败：脚本已经结束，但没有取得退出码。`
        : `${label}脚本调用失败：脚本以退出码 ${result.exitCode} 结束。`,
  }
}
