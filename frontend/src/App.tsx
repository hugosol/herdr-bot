import { useEffect, useRef, useState, type KeyboardEvent, type TouchEvent } from 'react'
import './navigation.css'

import { runOperation, type FailureKind, type OperationName, type OperationResult } from './api'
import { TasksPanel } from './TasksPanel'
import { FileListPanel } from './FileListPanel'
import { ChatPanel } from './ChatPanel'
import { SchedulerPanel } from './SchedulerPanel'
import { RunDetails, selectedRunId } from './RunDetails'
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

const TAB_GROUPS = {
  work: ['sessions', 'files', 'workflow'],
  config: ['tasks', 'bot', 'scheduler'],
} as const
const TAB_ORDER = [...TAB_GROUPS.work, ...TAB_GROUPS.config]
type TabKey = (typeof TAB_ORDER)[number]
type NavigationGroup = keyof typeof TAB_GROUPS
const TAB_LABELS: Record<TabKey, string> = {
  sessions: 'chat', files: 'zone', workflow: 'workflow',
  tasks: 'task', bot: 'bot', scheduler: 'scheduler',
}
const TAB_ICONS: Record<TabKey, string> = {
  sessions: 'M4 4h16v12H9l-5 4V4Z',
  files: 'M3 6h7l2 2h9v12H3V6Z',
  workflow: 'm12 3 9 9-9 9-9-9 9-9Z',
  tasks: 'M8 6h13M8 12h13M8 18h13M3 6h1M3 12h1M3 18h1',
  bot: 'M5 7h14v13H5V7ZM12 3v4M8 12h1M15 12h1M9 16h6',
  scheduler: 'M4 4h16v16H4V4ZM4 9h16M9 9v11M15 9v11',
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
  const [initialRunId] = useState(() => {
    const navigation = performance.getEntriesByType?.('navigation')[0] as PerformanceNavigationTiming | undefined
    return navigation?.type === 'reload' ? null : selectedRunId()
  })
  const [runId, setRunId] = useState(initialRunId)
  const [tab, setTab] = useState<TabKey>(initialRunId ? 'workflow' : 'sessions')
  const [layout, setLayout] = useState<'compact' | 'expanded'>(() => new URL(window.location.href).searchParams.get('layout') === 'expanded' ? 'expanded' : 'compact')
  const [group, setGroup] = useState<NavigationGroup>('work')
  const lastTabs = useRef<Record<NavigationGroup, TabKey>>({ work: initialRunId ? 'workflow' : 'sessions', config: 'tasks' })
  const [navigationFade, setNavigationFade] = useState(false)
  const [serverOpen, setServerOpen] = useState(false)
  const [taskEditRequest, setTaskEditRequest] = useState<{ id: string } | null>(null)
  const schedulerNavigationGuard = useRef<(() => boolean) | null>(null)
  const acceptedHash = useRef(window.location.hash)
  const touchStart = useRef<{ id: number; x: number; y: number; horizontal: boolean } | null>(null)
  const [dragOffset, setDragOffset] = useState(0)
  const [dragging, setDragging] = useState(false)
  const navViewport = useRef<HTMLDivElement>(null)
  const schedulerTab = tab === 'scheduler' || tab === 'bot'
  const visibleTabs: readonly TabKey[] = layout === 'expanded' ? TAB_ORDER : TAB_GROUPS[group]

  useEffect(() => {
    const url = new URL(window.location.href)
    url.searchParams.set('layout', layout)
    window.history.replaceState(window.history.state, '', url)
  }, [layout])

  useEffect(() => {
    // Reload starts at Chat. Retire the old Run route so clicking that same
    // Run link later still produces a real hash navigation.
    const navigation = performance.getEntriesByType?.('navigation')[0] as PerformanceNavigationTiming | undefined
    if (navigation?.type !== 'reload' || !selectedRunId()) return
    const url = new URL(window.location.href)
    url.hash = 'workflow'
    window.history.replaceState(window.history.state, '', url)
    acceptedHash.current = window.location.hash
  }, [])

  useEffect(() => {
    if (!navigationFade) return
    const timer = window.setTimeout(() => setNavigationFade(false), 200)
    return () => window.clearTimeout(timer)
  }, [navigationFade, layout, group])

  function selectTab(next: TabKey, guarded = true) {
    if (next !== tab && guarded && schedulerTab && schedulerNavigationGuard.current?.() === false) return false
    const nextGroup = (TAB_GROUPS.work as readonly TabKey[]).includes(next) ? 'work' : 'config'
    if (tab === 'workflow' && next !== 'workflow' && selectedRunId()) {
      const url = new URL(window.location.href)
      url.hash = 'workflow'
      window.history.replaceState(window.history.state, '', url)
      acceptedHash.current = window.location.hash
    }
    lastTabs.current[nextGroup] = next
    setGroup(nextGroup)
    setTab(next)
    return true
  }

  useEffect(() => {
    const navigate = () => {
      if (window.location.hash === acceptedHash.current) return
      const id = selectedRunId()
      const workflowRoute = id !== null || window.location.hash === '#workflow'
      if (workflowRoute && !selectTab('workflow')) {
        const url = new URL(window.location.href)
        url.hash = acceptedHash.current
        window.history.replaceState(window.history.state, '', url)
        return
      }
      acceptedHash.current = window.location.hash
      setRunId(id)
      if (workflowRoute) setServerOpen(false)
    }
    window.addEventListener('hashchange', navigate)
    return () => window.removeEventListener('hashchange', navigate)
  })

  function cancelSwipe() {
    touchStart.current = null
    setDragging(false)
    setDragOffset(0)
  }

  function moveSwipe(event: TouchEvent<HTMLElement>) {
    const start = touchStart.current
    if (!start || layout !== 'compact') return
    if (event.touches.length !== 1 || event.touches[0].identifier !== start.id) { cancelSwipe(); return }
    const dx = event.touches[0].clientX - start.x
    const dy = event.touches[0].clientY - start.y
    if (!start.horizontal) {
      if (Math.max(Math.abs(dx), Math.abs(dy)) < 8) return
      if (Math.abs(dx) <= Math.abs(dy) * 1.5) { cancelSwipe(); return }
      start.horizontal = true
      setDragging(true)
    }
    const width = navViewport.current?.clientWidth ?? 0
    setDragOffset(group === 'work' ? Math.max(-width, Math.min(0, dx)) : Math.min(width, Math.max(0, dx)))
  }

  function finishSwipe(event: TouchEvent<HTMLElement>) {
    const start = touchStart.current
    cancelSwipe()
    if (!start?.horizontal || layout !== 'compact' || event.touches.length !== 0 || event.changedTouches.length !== 1) return
    event.preventDefault()
    const end = event.changedTouches[0]
    const width = navViewport.current?.clientWidth ?? 0
    if (end.identifier !== start.id || width === 0) return
    const dx = end.clientX - start.x
    if ((group === 'work' && dx <= -width / 3) || (group === 'config' && dx >= width / 3)) {
      selectTab(lastTabs.current[group === 'work' ? 'config' : 'work'])
    }
  }
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
    const index = visibleTabs.indexOf(tab)
    let nextIndex: number
    switch (event.key) {
      case 'ArrowDown':
      case 'ArrowRight':
        nextIndex = (index + 1) % visibleTabs.length
        break
      case 'ArrowUp':
      case 'ArrowLeft':
        nextIndex = (index - 1 + visibleTabs.length) % visibleTabs.length
        break
      case 'Home':
        nextIndex = 0
        break
      case 'End':
        nextIndex = visibleTabs.length - 1
        break
      default:
        return
    }

    // 方向键只属于顶层入口：阻止页面滚动，并同步选中态与焦点。
    event.preventDefault()
    const next = visibleTabs[nextIndex]
    if (selectTab(next)) tabRefs.current[next]?.focus()
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
    <div className={navigationFade ? 'app-layout navigation-fade' : 'app-layout'} data-layout={layout} data-group={group}>
      <header className="layout-header">
        <div className="layout-header-start">
          <button type="button" data-action="layout" aria-label={layout === 'compact' ? '展开导航' : '收起导航'} aria-expanded={layout === 'expanded'} onClick={() => { setNavigationFade(true); setLayout(layout === 'compact' ? 'expanded' : 'compact') }}>
            <span aria-hidden="true">☰</span><span className="layout-toggle-label">{layout === 'compact' ? '展开' : '收起'}</span>
          </button>
          <h1 className="layout-brand">herdr-bot</h1>
        </div>
        <button type="button" data-action="server" data-testid={serverOpen ? 'server-back' : 'server-button'} ref={serverBackRef} onClick={() => { if (serverOpen) setServerOpen(false); else if (!schedulerTab || schedulerNavigationGuard.current?.() !== false) setServerOpen(true) }}>{serverOpen ? '返回' : 'server'}</button>
      </header>
      <div className="layout-shell">
        <nav className="layout-nav" hidden={serverOpen} aria-label="主导航"
          onTouchStart={(event) => { cancelSwipe(); const touch = event.touches[0]; touchStart.current = layout === 'compact' && event.touches.length === 1 ? { id: touch.identifier, x: touch.clientX, y: touch.clientY, horizontal: false } : null }}
          onTouchMove={moveSwipe}
          onTouchCancel={cancelSwipe} onTouchEnd={finishSwipe}>
          <div className="layout-nav-viewport" ref={navViewport}>
          <div role="tablist" aria-label="herdr-bot 主界面分区" onKeyDown={selectAdjacentTab} className="layout-nav-track" data-dragging={dragging} style={layout === 'compact' ? { transform: `translateX(calc(${group === 'work' ? '0%' : '-100%'} + ${dragOffset}px))` } : undefined}>
            {(['work', 'config'] as const).map((navGroup) => <div key={navGroup} className="layout-nav-group" inert={layout === 'compact' && group !== navGroup} aria-hidden={layout === 'compact' && group !== navGroup ? true : undefined}>
              {TAB_GROUPS[navGroup].map((key) => <button key={key} type="button" role="tab" id={`tab-${key}`} className={tab === key ? 'layout-nav-item is-active' : 'layout-nav-item'} data-testid={`tab-${key}`} aria-selected={tab === key} aria-controls={key === 'bot' ? 'panel-scheduler' : `panel-${key}`} ref={(node) => { tabRefs.current[key] = node }} onClick={() => selectTab(key)}>
                <svg className="layout-nav-icon" aria-hidden="true" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round"><path d={TAB_ICONS[key]} /></svg><span>{TAB_LABELS[key]}</span>
              </button>)}
            </div>)}
          </div>
          </div>
        </nav>
        <main className={!serverOpen && (tab === 'files' || tab === 'tasks') ? 'panel panel--wide layout-content' : 'panel layout-content'}>

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
        <TasksPanel active={!serverOpen && tab === 'tasks'} editRequest={taskEditRequest} onEditRequestHandled={() => setTaskEditRequest(null)} />
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
        aria-labelledby={tab === 'bot' ? 'tab-bot' : 'tab-scheduler'}
        className="tabpanel"
        data-testid="panel-scheduler"
        hidden={serverOpen || !schedulerTab}
      >
        <SchedulerPanel active={!serverOpen && schedulerTab} view={tab === 'bot' ? 'bot' : 'map'}
          onViewChange={(view) => { selectTab(view === 'bot' ? 'bot' : 'scheduler', false) }} navigationGuard={schedulerNavigationGuard}
          onTask={(id) => { if (selectTab('tasks')) setTaskEditRequest(id ? { id } : null) }}
          onZones={() => { selectTab('files') }} />
      </div>

      <div id="panel-workflow" role="tabpanel" aria-labelledby="tab-workflow" className="tabpanel" hidden={serverOpen || tab !== 'workflow'}>
        <RunDetails id={runId} active={!serverOpen && tab === 'workflow'} />
      </div>

      <section
        aria-labelledby="server-title"
        data-testid="server-page"
        hidden={!serverOpen}
      >
        <h2 id="server-title">server</h2>
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
      </div>
    </div>
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
