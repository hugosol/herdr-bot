import { useEffect, useState } from 'react'

import { isAwaitingOnline, sortByTier } from './sessionOrdering'
import type { AgentState, KeepAlive, SessionFeed, SessionView } from './sessionFeed'
import {
  COUNTDOWN_ACTIVE,
  NOT_ELIGIBLE,
  UNKNOWN_SESSION,
  cancelShutdown,
  setMonitoring,
} from './shutdownApi'
import { useSessionFeedSnapshot } from './useSessionFeed'

/** agent 状态在界面上的中文名。 */
const STATE_LABELS: Record<AgentState, string> = {
  idle: '空闲',
  working: '工作中',
  'awaiting-input': '等待用户输入',
}

/** keepAlive 灯与离线标记的文案。 */
const KEEPALIVE_LABELS: Record<KeepAlive, string> = {
  online: '在线',
  offline: '离线',
}

/**
 * 一次监听命令的本地结果。
 *
 * 只有「已被接受」「未确认」与「明确拒绝」三种归宿需要在这里留下说明：目标选中值是否已经成立由
 * **后端权威集合**决定（按钮的选中态跟着观察流走），本地绝不乐观翻转，也不把没有回应记成成功。
 */
type MonitorStatus = { kind: 'accepted' } | { kind: 'refused'; code: string } | { kind: 'unconfirmed' }

/** 全局清空入口的本地结果：同样只有拿到服务端确认才算这次命令被接受。 */
type ClearStatus = 'idle' | 'clearing' | 'cleared' | 'refused' | 'unconfirmed'

/** 会话名回退：`<cwd 目录名> · <sessionId 前 8 位>`。 */
function sessionDisplayName(session: SessionView): string {
  const named = session.sessionName?.trim()
  if (named) return named
  const dir = session.cwd.split(/[\\/]/).filter(Boolean).pop() ?? session.cwd
  return `${dir} · ${session.sessionId.slice(0, 8)}`
}

type SessionCardProps = {
  session: SessionView
  /** 是否属于置顶等待组：在线的等待用户输入会话，卡片用普通等待状态样式。 */
  waiting: boolean
  /** 完成后关机监听控件要渲染的全部信息；`null` 表示这个会话当前不提供监听按钮。 */
  monitoring: {
    selected: boolean
    note: string | null
    status: MonitorStatus | null
    busy: boolean
    onToggle: (selected: boolean) => void
  } | null
}

function SessionCard({ session, waiting, monitoring }: SessionCardProps) {
  const keepAlive = session.online ? 'online' : 'offline'
  const state = session.state ?? 'unknown'
  // 离线的登记仍在列表里，但它的 agent 状态只是**最后上报值**：与「过期」一起呈现，
  // 不能被读成当前完成证据（等待用户输入也不等于现在轮次结束）。
  const lastReported = !session.online && session.state !== undefined
  return (
    <article
      className={`session-card${session.online ? '' : ' is-offline'}${waiting ? ' is-waiting' : ''}`}
      data-testid="session-card"
      data-keepalive={keepAlive}
      data-state={state}
      data-waiting={waiting ? 'true' : undefined}
    >
      <header className="session-card__head">
        <span
          className={`session-card__dot session-card__dot--${keepAlive}`}
          data-testid="keepalive-dot"
          data-keepalive={keepAlive}
          role="img"
          aria-label={KEEPALIVE_LABELS[keepAlive]}
          title={KEEPALIVE_LABELS[keepAlive]}
        />
        <strong className="session-card__name" data-testid="session-name">
          {sessionDisplayName(session)}
        </strong>
        {session.online ? null : (
          <span className="session-card__offline" data-testid="offline-tag">
            已离线
          </span>
        )}
      </header>
      <p
        className="session-card__state"
        data-testid="session-state"
        data-state={state}
        data-reported={session.online ? 'current' : 'last-reported'}
      >
        {session.state !== undefined ? STATE_LABELS[session.state] : '状态同步中…'}
      </p>
      {lastReported ? (
        <span className="session-card__last-reported" data-testid="last-reported-tag">
          最后上报
        </span>
      ) : null}
      <p className="session-card__meta">
        <span className="session-card__cwd" data-testid="session-cwd">
          {session.cwd}
        </span>
        {session.model?.trim() ? (
          <span className="session-card__model" data-testid="session-model">
            {session.model}
          </span>
        ) : null}
      </p>

      {monitoring === null ? null : (
        <div className="session-card__monitoring">
          <button
            type="button"
            className={`monitor-button${monitoring.selected ? ' is-selected' : ''}`}
            data-testid="monitor-button"
            data-selected={monitoring.selected ? 'true' : 'false'}
            aria-pressed={monitoring.selected}
            disabled={monitoring.busy}
            onClick={() => monitoring.onToggle(!monitoring.selected)}
          >
            {monitoring.selected ? '完成后关机：已选择' : '完成后关机'}
          </button>
          {monitoring.note === null ? null : (
            <p className="monitor-note" data-testid="monitor-note" role="status">
              {monitoring.note}
            </p>
          )}
          {monitoring.status === null ? null : (
            <p
              className="monitor-status"
              data-testid="monitor-status"
              data-outcome={monitoring.status.kind}
              data-code={monitoring.status.kind === 'refused' ? monitoring.status.code : undefined}
              role="status"
            >
              {monitoring.status.kind === 'unconfirmed'
                ? '未确认：没有收到服务端的答复，无法确认这次监听命令是否生效；页面不会自动重发。'
                : monitoring.status.kind === 'accepted'
                  ? '服务端已接受这次监听命令；这次命令的归宿是「已接受」。选中态以观察流为准，尚未更新的展示保持不变。'
                  : monitoring.status.code === NOT_ELIGIBLE
                    ? '服务端拒绝：该会话当前不在线或不在工作中，不能开始监听。'
                    : monitoring.status.code === COUNTDOWN_ACTIVE
                      ? '服务端拒绝：已有活动倒计时，期间不接受新增监听。'
                      : monitoring.status.code === UNKNOWN_SESSION
                        ? '服务端拒绝：该会话尚未注册。'
                        : '服务端明确拒绝这次监听命令。'}
            </p>
          )}
        </div>
      )}
    </article>
  )
}

/**
 * 「会话列表」的卡片流：每个会话一张卡，等待用户输入的会话聚成置顶组。
 *
 * 组件不直接依赖全局 `EventSource`——数据源在 `SessionFeed` 契约后面（真实 SSE 客户端在
 * `sseSessionFeed.ts`，运行时由 `main.tsx` 注入），测试用假数据源或假 `EventSource` 驱动同一段代码。
 *
 * 「完成后关机」按钮的选中态**只**来自后端权威集合（观察流的 `monitoredSessionIds`），提交的是明确的
 * 目标选中值，因此本地不做乐观翻转；命令回执只反馈本次命令的归宿（已被接受／未确认／明确拒绝），
 * 不拿来填补选中态，也不自动重发。
 */
export function SessionsPanel({ feed }: { feed: SessionFeed }) {
  const snapshot = useSessionFeedSnapshot(feed)
  const [status, setStatus] = useState<Record<string, MonitorStatus>>({})
  // 正在提交命令的会话：只禁用那一个按钮，其他会话照常可用。
  const [busy, setBusy] = useState<string | null>(null)
  const [clearing, setClearing] = useState<ClearStatus>('idle')

  // 业务快照只有一个来源：共享观察流。命令回执只反馈这次命令的归宿（见 `status`／`clearing`），
  // 不参与展示——独立通道的迟到回执因此不可能覆盖较新的观察结果。
  const shutdown = snapshot.shutdown
  const ordered = sortByTier(snapshot.sessions)
  const waiting = ordered.filter(isAwaitingOnline)
  const rest = ordered.filter((session) => !isAwaitingOnline(session))

  // 观察流下一次给出关机状态时，「已接受」这条归宿说明就被权威事实取代：它只是回执，不是业务状态。
  useEffect(() => {
    setStatus((current) => {
      if (!Object.values(current).some((value) => value.kind === 'accepted')) return current
      const next: Record<string, MonitorStatus> = {}
      for (const [sessionId, value] of Object.entries(current)) {
        if (value.kind !== 'accepted') next[sessionId] = value
      }
      return next
    })
  }, [snapshot.shutdown])

  const monitored = new Set(shutdown.monitoredSessionIds)
  // 活动倒计时期间不接受新增监听，界面因此也不提供这个入口。
  const locked = shutdown.phase === 'counting-down' || shutdown.phase === 'executing'

  /** 某个会话的监听控件信息；返回 `null` 表示当前不提供按钮。 */
  function monitoringFor(session: SessionView) {
    const selected = monitored.has(session.sessionId)
    if (locked) return null
    // 在线且工作中的会话可以新增监听；已经在集合里的会话即便进入等待用户输入也保留可撤销的按钮。
    if (!selected && !(session.online && session.state === 'working')) return null

    const stillWaitingForOthers =
      selected &&
      session.state === 'awaiting-input' &&
      [...monitored].some((id) => {
        if (id === session.sessionId) return false
        const other = snapshot.sessions.find((candidate) => candidate.sessionId === id)
        return other !== undefined && other.state !== 'awaiting-input'
      })

    return {
      selected,
      note: stillWaitingForOthers
        ? '已等待用户输入，正在等待其他被监听会话；它们都等待后才会开始关机倒计时。'
        : null,
      status: status[session.sessionId] ?? null,
      busy: busy === session.sessionId,
      onToggle: (next: boolean) => void toggleMonitoring(session.sessionId, next),
    }
  }

  async function toggleMonitoring(sessionId: string, selected: boolean) {
    setBusy(sessionId)
    // 新的监听动作之后，上一次清空的说明不再代表当前状态。
    setClearing('idle')
    const result = await setMonitoring(sessionId, selected)
    setBusy(null)
    if (result.kind === 'accepted') {
      // 这次命令被服务端接受：只反馈归宿，选中态仍等观察流的权威集合，绝不凭本地翻转。
      setStatus((current) => ({ ...current, [sessionId]: { kind: 'accepted' } }))
      return
    }
    setStatus((current) => ({
      ...current,
      [sessionId]:
        result.kind === 'rejected' ? { kind: 'refused', code: result.code } : { kind: 'unconfirmed' },
    }))
  }

  async function clearMonitoring() {
    setClearing('clearing')
    const result = await cancelShutdown()
    // 只有服务端确认取消生效才算这次命令被接受；没有回应只说未确认，且不自动重发。
    setClearing(result.kind === 'cancelled' ? 'cleared' : result.kind)
  }

  return (
    <div className="session-board" data-testid="session-board">
      {/* 降级只在会话列表内呈现：断线时保留最后一帧快照，并说明它可能已经过期。 */}
      {snapshot.connection === 'interrupted' ? (
        <div className="session-stale" role="status" data-testid="connection-banner">
          <strong>与 omp-bot 的连接已中断</strong>
          <span>下面是最后一次收到的快照，可能已经过期；连接恢复后会自动对齐。</span>
        </div>
      ) : null}

      {/* 全局清空入口：只在有监听且没有倒计时时出现（倒计时会消耗整组监听），
          因此它是「取消关机并清空监听」这一全局意图，而不是某个会话的撤销，也不是结果页的返回。 */}
      {monitored.size === 0 ? null : (
        <section className="monitoring-bar" data-testid="monitoring-bar" aria-label="完成后关机监听">
          <p className="monitoring-bar__summary" data-testid="monitoring-summary">
            已完成监听的会话：{monitored.size} 个。全部成员都等待用户输入后，后端会开始关机倒计时。
          </p>
          <button
            type="button"
            className="monitoring-bar__clear"
            data-testid="monitoring-clear"
            disabled={clearing === 'clearing'}
            onClick={() => void clearMonitoring()}
          >
            {clearing === 'clearing' ? '正在清空…' : '取消关机并清空监听'}
          </button>
        </section>
      )}

      {/* 清空的结果说明单独占一处：集合清空后入口本身会收起，但「服务端已确认」这条归宿要留下。 */}
      {clearing === 'cleared' ? (
        <p
          className="monitoring-bar__status"
          data-testid="monitoring-clear-status"
          data-outcome="cleared"
          role="status"
        >
          服务端已确认这次取消；这次命令的归宿是「已接受」。监听集合与关机安排以观察流为准。
        </p>
      ) : null}
      {clearing === 'refused' ? (
        <p
          className="monitoring-bar__status"
          data-testid="monitoring-clear-status"
          data-outcome="refused"
          role="status"
        >
          取消没有生效：服务端明确拒绝，电脑仍可能按当前安排关闭。
        </p>
      ) : null}
      {clearing === 'unconfirmed' ? (
        <p
          className="monitoring-bar__status"
          data-testid="monitoring-clear-status"
          data-outcome="unconfirmed"
          role="status"
        >
          清空未确认，电脑仍可能关机：没有收到服务端的确认，页面不会自动重发。
        </p>
      ) : null}

      {snapshot.sessions.length === 0 ? (
        <div className="session-empty" data-testid="sessions-empty">
          <strong>面板已就绪，尚无会话接入</strong>
          <span>在 omp 里执行 /connect-bot 后，会话会出现在这里。</span>
        </div>
      ) : (
        <>
          {waiting.length === 0 ? null : (
            <section
              className="session-group session-group--waiting"
              data-testid="waiting-group"
              aria-labelledby="waiting-group-heading"
            >
              <h2 className="session-group__heading" id="waiting-group-heading" data-testid="waiting-group-heading">
                等待用户输入
              </h2>
              {waiting.map((session) => (
                <SessionCard
                  key={session.sessionId}
                  session={session}
                  waiting
                  monitoring={monitoringFor(session)}
                />
              ))}
            </section>
          )}

          {rest.map((session) => (
            <SessionCard
              key={session.sessionId}
              session={session}
              waiting={false}
              monitoring={monitoringFor(session)}
            />
          ))}
        </>
      )}
    </div>
  )
}
