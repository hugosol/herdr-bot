import { useRef, useState } from 'react'

import { isAwaitingOnline, sortByTier } from './sessionOrdering'
import type { AgentState, KeepAlive, SessionFeed, SessionFeedSnapshot, SessionView } from './sessionFeed'
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

/** 会话名回退：`<cwd 目录名> · <sessionId 前 8 位>`。 */
function sessionDisplayName(session: SessionView): string {
  const named = session.sessionName?.trim()
  if (named) return named
  const dir = session.cwd.split(/[\\/]/).filter(Boolean).pop() ?? session.cwd
  return `${dir} · ${session.sessionId.slice(0, 8)}`
}

type SessionCardProps = {
  session: SessionView
  /** 是否属于置顶等待组：在线的等待用户输入会话，卡片据此高亮。 */
  waiting: boolean
  /** 是否刚发生 `working → awaiting-input`：等待组内额外高亮这一张。 */
  justEnded: boolean
}

function SessionCard({ session, waiting, justEnded }: SessionCardProps) {
  const keepAlive = session.online ? 'online' : 'offline'
  const state = session.state ?? 'unknown'
  const emphasis = `${waiting ? ' is-waiting' : ''}${justEnded ? ' is-just-ended' : ''}`
  return (
    <article
      className={`session-card${session.online ? '' : ' is-offline'}${emphasis}`}
      data-testid="session-card"
      data-keepalive={keepAlive}
      data-state={state}
      data-waiting={waiting ? 'true' : undefined}
      data-just-ended={justEnded ? 'true' : undefined}
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
      <p className="session-card__state" data-testid="session-state" data-state={state}>
        {session.state !== undefined ? STATE_LABELS[session.state] : '状态同步中…'}
      </p>
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
    </article>
  )
}

/** 判定「刚结束一轮」所需的基线：上一帧快照本身（判断前后是否都处于实况）与各会话的状态。 */
type FeedObservation = {
  snapshot: SessionFeedSnapshot | null
  states: Map<string, AgentState | undefined>
}

/**
 * 「会话列表」的卡片流：每个会话一张卡，等待用户输入的会话聚成置顶组；组头给出自上次
 * 点「知道了」以来刚结束轮次的会话数。
 *
 * 组件不直接依赖全局 `EventSource`——数据源在 `SessionFeed` 契约后面（真实 SSE 客户端在
 * `sseSessionFeed.ts`，运行时由 `main.tsx` 注入），测试用假数据源或假 `EventSource` 驱动同一段代码。
 */
export function SessionsPanel({ feed }: { feed: SessionFeed }) {
  const snapshot = useSessionFeedSnapshot(feed)

  // 「刚结束一轮」= 自上次点「知道了」以来观察到 `working → awaiting-input` 的会话集合。
  // 它是状态而不是纯派生值：点「知道了」要清空它，而清空必须能触发重渲染。
  const [endedRounds, setEndedRounds] = useState<ReadonlySet<string>>(() => new Set<string>())
  const observation = useRef<FeedObservation>({ snapshot: null, states: new Map() })

  // 提示只由真实迁移派生，且只派生一次：每来一帧推进一次观察，只有当上一帧与这一帧都处在
  // 实况（`connection === 'live'`）时才结算迁移。首帧（还没有基线）、断线帧与重连后的快照
  // 都只重建基线，于是 snapshot/重连不计 N、也不补发提示；同值帧状态没变，同样不结算。
  // 用 ref 而不是 effect：判定与渲染在同一次完成，StrictMode 的重复渲染不会重复记账。
  if (observation.current.snapshot !== snapshot) {
    const previous = observation.current
    observation.current = {
      snapshot,
      states: new Map(snapshot.sessions.map((session) => [session.sessionId, session.state])),
    }

    if (previous.snapshot?.connection === 'live' && snapshot.connection === 'live') {
      const ended = snapshot.sessions
        .filter(
          (session) => session.state === 'awaiting-input' && previous.states.get(session.sessionId) === 'working',
        )
        .map((session) => session.sessionId)
      if (ended.length > 0) {
        setEndedRounds((previousEnded) => new Set([...previousEnded, ...ended]))
      }
    }
  }

  const ordered = sortByTier(snapshot.sessions)
  const waiting = ordered.filter(isAwaitingOnline)
  const rest = ordered.filter((session) => !isAwaitingOnline(session))
  // 组头 N 与卡片高亮同源：都取「等待组 ∩ 刚结束」，因此 N 恒等于高亮卡片的张数，
  // 也只数在线会话——离线（含原等待者）不在等待组里，自然不计入。
  const freshWaiting = waiting.filter((session) => endedRounds.has(session.sessionId))

  /** 「知道了」只清强调：清空 N 与高亮；等待组与 `awaiting-input` 状态不属于强调，保持不变。 */
  function acknowledgeEndedRounds() {
    setEndedRounds((previousEnded) => (previousEnded.size === 0 ? previousEnded : new Set<string>()))
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
              <header className="session-group__head">
                <h2 className="session-group__heading" id="waiting-group-heading" data-testid="waiting-group-heading">
                  等待用户输入
                </h2>
                {/* N=0 时组头只显示「等待用户输入」：计数与「知道了」只在真有刚结束的轮次时出现。 */}
                {freshWaiting.length === 0 ? null : (
                  <>
                    <span className="session-group__note" data-testid="waiting-group-note">
                      {freshWaiting.length} 个刚结束轮次
                    </span>
                    <button
                      type="button"
                      className="session-group__dismiss"
                      data-testid="waiting-group-dismiss"
                      onClick={acknowledgeEndedRounds}
                    >
                      知道了
                    </button>
                  </>
                )}
              </header>
              {waiting.map((session) => (
                <SessionCard
                  key={session.sessionId}
                  session={session}
                  waiting
                  justEnded={freshWaiting.includes(session)}
                />
              ))}
            </section>
          )}

          {rest.map((session) => (
            <SessionCard key={session.sessionId} session={session} waiting={false} justEnded={false} />
          ))}
        </>
      )}
    </div>
  )
}
