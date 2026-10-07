import { useState } from 'react'

import { runOperation, type FailureKind, type OperationName, type OperationResult } from './api'
import { SessionsPanel } from './SessionsPanel'
import { EMPTY_SESSION_FEED, type SessionFeed } from './sessionFeed'
import { ShutdownControl } from './ShutdownControl'
import { useSessionFeedSnapshot } from './useSessionFeed'

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

/** omp-bot 主界面的两个分区：控制面板承载固定操作，会话列表呈现各会话的 keepAlive 与 agent 状态。 */
type TabKey = 'control' | 'sessions'

type Feedback = {
  kind: FeedbackKind
  text: string
  /** 失败子类别（无法启动 / 失败退出）；只有失败反馈带这个标识。 */
  failure?: FailureKind
}

/**
 * omp-bot 主界面。
 *
 * `sessionFeed` 是注入给「会话列表」的数据源；不传时用空数据源（页面显示引导空态）。
 * 运行时由 `main.tsx` 注入真实 SSE 数据源；组件本身不依赖全局 `EventSource`，因此组件测试
 * 可以用假数据源直接驱动。
 */
export default function App({ sessionFeed = EMPTY_SESSION_FEED }: { sessionFeed?: SessionFeed } = {}) {
  const [state, setState] = useState<PanelState>({ kind: 'idle' })
  // 分区选择只属于这个组件：单页无路由，不做 roving tabindex——两个 tab 都是原生按钮，
  // Tab 键依次聚焦、Enter/Space 触发点击即可切换。
  const [tab, setTab] = useState<TabKey>('control')
  // 关机流程整屏接管期间，底层“启动/停止”不可操作（关机本身与它们的执行状态无关）。
  const [shutdownTakeover, setShutdownTakeover] = useState(false)
  // 后端权威的关机状态与观察流健康度：与「会话列表」同一条观察流，因此同一次订阅就够。
  const { shutdown, connection } = useSessionFeedSnapshot(sessionFeed)

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
      <h1>omp-bot</h1>

      {/* 双 tab 外壳：整页恰两个分区，打开时默认停在「控制面板」。
          两个 tab 是原生按钮——Tab 键逐个聚焦、Enter/Space 切换；
          不实现方向键与 roving tabindex（明确偏离 WAI-ARIA APG，依赖原生行为）。 */}
      <div className="tabs" role="tablist" aria-label="omp-bot 主界面分区">
        <button
          type="button"
          role="tab"
          id="tab-control"
          className={tab === 'control' ? 'tab is-active' : 'tab'}
          data-testid="tab-control"
          aria-selected={tab === 'control'}
          aria-controls="panel-control"
          onClick={() => setTab('control')}
        >
          控制面板
        </button>
        <button
          type="button"
          role="tab"
          id="tab-sessions"
          className={tab === 'sessions' ? 'tab is-active' : 'tab'}
          data-testid="tab-sessions"
          aria-selected={tab === 'sessions'}
          aria-controls="panel-sessions"
          onClick={() => setTab('sessions')}
        >
          会话列表
        </button>
      </div>

      {/* 分区内容常驻、只切换可见性：切走再切回不会丢掉已提交操作的结果反馈，
          关机的整屏接管也不会被切 tab 打断。 */}
      <div
        id="panel-control"
        role="tabpanel"
        aria-labelledby="tab-control"
        className="tabpanel"
        data-testid="panel-control"
        hidden={tab !== 'control'}
      >
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
      </div>

      <div
        id="panel-sessions"
        role="tabpanel"
        aria-labelledby="tab-sessions"
        className="tabpanel"
        data-testid="panel-sessions"
        hidden={tab !== 'sessions'}
      >
        <SessionsPanel feed={sessionFeed} />
      </div>

      {/* 全局关机提醒不能位于 hidden 的 tab 容器内；手动入口仍只在控制面板可见。 */}
      <ShutdownControl
        showButton={tab === 'control'}
        shutdown={shutdown}
        connection={connection}
        onTakeoverChange={setShutdownTakeover}
      />
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
