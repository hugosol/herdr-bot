import { useEffect, useRef, useState, type KeyboardEvent } from 'react'

import { runOperation, type FailureKind, type OperationName, type OperationResult } from './api'
import { TasksPanel } from './TasksPanel'
import { FileListPanel } from './FileListPanel'
import { ChatPanel } from './ChatPanel'
import { SchedulerPanel } from './SchedulerPanel'
import { EMPTY_SESSION_FEED, type SessionFeed } from './sessionFeed'
import { ShutdownControl } from './ShutdownControl'
import { useSessionFeedSnapshot } from './useSessionFeed'
import type { ZoneWatchFeed } from './zoneWatch'

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

/**
 * herdr-bot 主界面的四个顶层入口：chat → task → zone → scheduler。
 * 打开或整页刷新永远从 chat 开始（见 `useState` 初值），没有恢复上次 tab 的持久化状态。
 */
const TAB_ORDER = ['sessions', 'tasks', 'files', 'scheduler'] as const

type TabKey = (typeof TAB_ORDER)[number]

const TAB_LABELS: Record<TabKey, string> = {
  sessions: 'chat',
  tasks: 'task',
  files: 'zone',
  scheduler: 'scheduler',
}

type Feedback = {
  kind: FeedbackKind
  text: string
  /** 失败子类别（无法启动 / 失败退出）；只有失败反馈带这个标识。 */
  failure?: FailureKind
}

/**
 * herdr-bot 主界面。
 *
 * `sessionFeed` 是注入给 `chat` 入口的数据源；不传时用空数据源（页面显示引导空态）。
 * 运行时由 `main.tsx` 注入真实 SSE 数据源；组件本身不依赖全局 `EventSource`，因此组件测试
 * 可以用假数据源直接驱动。
 */
export default function App({
  sessionFeed = EMPTY_SESSION_FEED,
  zoneWatchFeed,
}: {
  sessionFeed?: SessionFeed
  zoneWatchFeed?: ZoneWatchFeed
} = {}) {
  const [state, setState] = useState<PanelState>({ kind: 'idle' })
  // 每次打开或整页刷新都从 chat 开始：这里没有读取任何持久化的 tab 状态。
  const [tab, setTab] = useState<TabKey>('sessions')
  const [serverOpen, setServerOpen] = useState(false)
  const schedulerNavigationGuard = useRef<(() => boolean) | null>(null)
  // 四个 tab 都是原生按钮，都在 Tab 顺序里；方向键切换时把焦点一起移到新入口。
  const tabRefs = useRef<Partial<Record<TabKey, HTMLButtonElement | null>>>({})
  const serverBackRef = useRef<HTMLButtonElement>(null)
  const wasServerOpen = useRef(false)
  useEffect(() => {
    if (serverOpen) serverBackRef.current?.focus()
    else if (wasServerOpen.current) tabRefs.current[tab]?.focus()
    wasServerOpen.current = serverOpen
  }, [serverOpen, tab])
  // 关机流程整屏接管期间，底层“启动/停止”不可操作（关机本身与它们的执行状态无关）。
  const [shutdownTakeover, setShutdownTakeover] = useState(false)
  // 后端权威的关机状态与观察流健康度：与 chat 入口同一条观察流，因此同一次订阅就够。
  const { shutdown, connection } = useSessionFeedSnapshot(sessionFeed)

  function selectAdjacentTab(event: KeyboardEvent<HTMLDivElement>) {
    const index = TAB_ORDER.indexOf(tab)
    let nextIndex: number
    switch (event.key) {
      case 'ArrowRight':
        nextIndex = (index + 1) % TAB_ORDER.length
        break
      case 'ArrowLeft':
        nextIndex = (index - 1 + TAB_ORDER.length) % TAB_ORDER.length
        break
      case 'Home':
        nextIndex = 0
        break
      case 'End':
        nextIndex = TAB_ORDER.length - 1
        break
      default:
        return
    }

    // 方向键只属于顶层入口：阻止页面滚动，并同步选中态与焦点。
    event.preventDefault()
    const next = TAB_ORDER[nextIndex]
    if (next !== tab && tab === 'scheduler' && schedulerNavigationGuard.current?.() === false) return
    setTab(next)
    tabRefs.current[next]?.focus()
  }

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
    <main className={!serverOpen && (tab === 'files' || tab === 'tasks') ? 'panel panel--wide' : 'panel'}>
      <header className="app-header">
        <h1>herdr-bot</h1>
        <button type="button" data-testid="server-button" hidden={serverOpen} onClick={() => { if (tab !== 'scheduler' || schedulerNavigationGuard.current?.() !== false) setServerOpen(true) }}>
          server
        </button>
      </header>

      {/* 四 tab 外壳：chat → task → zone → scheduler，打开时默认停在 chat。
          四个 tab 都是原生按钮——Tab 键逐个聚焦、Enter/Space 触发点击；
          方向键在四个入口之间循环切换并同步焦点。左右滑动不参与顶层切换。 */}
      <div
        className="tabs"
        hidden={serverOpen}
        role="tablist"
        aria-label="herdr-bot 主界面分区"
        onKeyDown={selectAdjacentTab}
      >
        {TAB_ORDER.map((key) => (
          <button
            key={key}
            type="button"
            role="tab"
            id={`tab-${key}`}
            className={tab === key ? 'tab is-active' : 'tab'}
            data-testid={`tab-${key}`}
            aria-selected={tab === key}
            aria-controls={`panel-${key}`}
            ref={(node) => {
              tabRefs.current[key] = node
            }}
            onClick={() => { if (key === tab || tab !== 'scheduler' || schedulerNavigationGuard.current?.() !== false) setTab(key) }}
          >
            {TAB_LABELS[key]}
          </button>
        ))}
      </div>

      {/* 分区内容常驻、只切换可见性：切走再切回不会丢掉已提交操作的结果反馈，
          关机的整屏接管也不会被切 tab 打断。 */}
      <div
        id="panel-sessions"
        role="tabpanel"
        aria-labelledby="tab-sessions"
        className="tabpanel"
        data-testid="panel-sessions"
        hidden={serverOpen || tab !== 'sessions'}
      >
        <ChatPanel feed={sessionFeed} />
      </div>

      <div
        id="panel-tasks"
        role="tabpanel"
        aria-labelledby="tab-tasks"
        className="tabpanel"
        data-testid="panel-tasks"
        hidden={serverOpen || tab !== 'tasks'}
      >
        <TasksPanel active={!serverOpen && tab === 'tasks'} />
      </div>

      <div
        id="panel-files"
        role="tabpanel"
        aria-labelledby="tab-files"
        className="tabpanel"
        data-testid="panel-files"
        hidden={serverOpen || tab !== 'files'}
      >
        <FileListPanel active={!serverOpen && tab === 'files'} watchFeed={zoneWatchFeed} />
      </div>

      <div
        id="panel-scheduler"
        role="tabpanel"
        aria-labelledby="tab-scheduler"
        className="tabpanel"
        data-testid="panel-scheduler"
        hidden={serverOpen || tab !== 'scheduler'}
      >
        <SchedulerPanel active={!serverOpen && tab === 'scheduler'} navigationGuard={schedulerNavigationGuard} />
      </div>

      <section
        aria-labelledby="server-title"
        data-testid="server-page"
        hidden={!serverOpen}
      >
        <h2 id="server-title">server</h2>
        <button
          type="button"
          data-testid="server-back"
          className="server-back"
          ref={serverBackRef}
          onClick={() => setServerOpen(false)}
        >
          返回 {TAB_LABELS[tab]}
        </button>
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
      </section>

      {/* 全局关机提醒不能位于 hidden 的内容页内；手动入口只在 server 页可见。 */}
      <ShutdownControl
        showButton={serverOpen}
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
