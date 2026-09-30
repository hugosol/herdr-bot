import { useEffect, useRef, useState } from 'react'

import { SHUTDOWN_BUTTON_VISIBLE, SHUTDOWN_COUNTDOWN_SECONDS } from './config'
import { requestShutdown, type ShutdownOutcome } from './shutdownApi'

/**
 * 关机流程的相位。
 *
 * - `idle`：只有“关机”按钮。
 * - `confirm`：整屏确认页，问“确定要关闭电脑？”。
 * - `counting`：整屏倒计时接管，每秒递减。
 * - `requesting`：倒计时归零后，本次唯一的 `POST /api/shutdown` 已在路上，等答复。
 * - `sent` / `failed` / `unconfirmed`：这次请求的三个归宿（`ShutdownOutcome`），
 *   页面据此给出“已发出 / 明确失败 / 结果无法确认”三类反馈。
 */
type Phase = 'idle' | 'confirm' | 'counting' | 'requesting' | ShutdownOutcome

type ShutdownControlProps = {
  /** 整屏接管是否进行中；页面据此让底层“启动/停止”不可操作。 */
  onTakeoverChange: (takeover: boolean) => void
}

/** 三类结果的文案；消费者据此知道电脑到底会不会关。 */
const OUTCOME_SCREENS: Record<ShutdownOutcome, { title: string; note: string }> = {
  sent: {
    title: '关机指令已发出',
    note: '电脑正在关闭，稍后就会断电。',
  },
  failed: {
    title: '关机没有成功',
    note: '电脑不会关闭。可以稍后重新走一次关机流程。',
  },
  unconfirmed: {
    title: '结果无法确认',
    note: '没有收到服务端的响应，无法确认关机指令是否已发出；电脑可能正在关闭，请不要重复操作。',
  },
}

/** 相位是否已经是本次关机请求的结果（三类反馈之一）。 */
function isOutcome(phase: Phase): phase is ShutdownOutcome {
  return phase === 'sent' || phase === 'failed' || phase === 'unconfirmed'
}

/**
 * 关机按钮与整屏接管（确认页 / 倒计时 / 三类结果）。
 *
 * 关机与“启动/停止”互不干扰：按钮不受它们的执行状态影响，整屏接管期间则由页面把
 * 底层控件禁用。取消（确认页“返回”、倒计时“取消关机”、结果页“返回”）或刷新/关闭
 * 页面都回到初始状态；只有倒计时归零那一刻发出唯一一次 `POST /api/shutdown`，
 * 请求没得到响应时如实说“结果无法确认”，不当作失败，也不重发。
 */
export function ShutdownControl({ onTakeoverChange }: ShutdownControlProps) {
  const [phase, setPhase] = useState<Phase>('idle')
  const [seconds, setSeconds] = useState(SHUTDOWN_COUNTDOWN_SECONDS)
  // 本次关机流程是否已经发过请求：归零、重渲染或重挂载都不会让它发出第二次。
  const requested = useRef(false)

  useEffect(() => {
    if (phase !== 'counting') return

    if (seconds > 0) {
      const timer = window.setTimeout(() => setSeconds((value) => value - 1), 1000)
      return () => window.clearTimeout(timer)
    }

    // 归零：从这里发出本次流程唯一的一次请求。
    void issueShutdown()
  }, [phase, seconds])

  async function issueShutdown() {
    if (requested.current) return
    requested.current = true
    setPhase('requesting')
    // 三类归宿都在这里定下；没有响应只是“无法确认”，不会带出第二次请求。
    setPhase(await requestShutdown())
  }

  function begin() {
    setPhase('confirm')
    onTakeoverChange(true)
  }

  function confirm() {
    setSeconds(SHUTDOWN_COUNTDOWN_SECONDS)
    setPhase('counting')
  }

  function cancel() {
    // 手动重来的那次要能重新发请求：只有取消（或走完一整套流程后返回）才允许再次发送。
    requested.current = false
    setSeconds(SHUTDOWN_COUNTDOWN_SECONDS)
    setPhase('idle')
    onTakeoverChange(false)
  }

  return (
    <>
      {SHUTDOWN_BUTTON_VISIBLE && (
        <button type="button" className="shutdown" data-testid="shutdown-button" onClick={begin}>
          关机
        </button>
      )}

      {phase !== 'idle' && (
        <div
          className="shutdown-takeover"
          data-testid="shutdown-takeover"
          data-phase={phase}
          role="dialog"
          aria-modal="true"
          aria-label="关机"
        >
          {phase === 'confirm' && (
            <div className="shutdown-screen shutdown-screen--confirm" data-testid="shutdown-confirm">
              <p className="shutdown-kicker">关机</p>
              <h2 className="shutdown-title">确定要关闭电脑？</h2>
              <p className="shutdown-note">关机后需要重新开机。</p>
              <div className="shutdown-actions">
                <button
                  type="button"
                  className="shutdown-action shutdown-action--ghost"
                  data-testid="shutdown-back"
                  onClick={cancel}
                >
                  返回
                </button>
                <button
                  type="button"
                  className="shutdown-action shutdown-action--danger"
                  data-testid="shutdown-confirm-button"
                  onClick={confirm}
                >
                  关机
                </button>
              </div>
            </div>
          )}

          {phase === 'counting' && (
            <div
              className="shutdown-screen shutdown-screen--counting"
              data-testid="shutdown-countdown"
              data-seconds={seconds}
            >
              <div className="shutdown-countdown-body">
                <div className="shutdown-count" data-testid="shutdown-count">
                  {seconds}
                </div>
                <p className="shutdown-note">秒后关闭电脑</p>
              </div>
              <button
                type="button"
                className="shutdown-action shutdown-action--ghost shutdown-cancel"
                data-testid="shutdown-cancel"
                onClick={cancel}
              >
                取消关机
              </button>
            </div>
          )}

          {phase === 'requesting' && (
            // 请求已在路上，这里没有“取消”：发出去的请求收不回来，取消按钮只会误导。
            <div
              className="shutdown-screen shutdown-screen--outcome"
              data-testid="shutdown-requesting"
            >
              <div className="shutdown-outcome-body">
                <p className="shutdown-kicker">关机</p>
                <h2 className="shutdown-title">正在发出关机指令…</h2>
                <p className="shutdown-note">等待服务端答复。</p>
              </div>
            </div>
          )}

          {isOutcome(phase) && (
            <div
              className="shutdown-screen shutdown-screen--outcome"
              data-testid="shutdown-result"
              data-outcome={phase}
            >
              <div className="shutdown-outcome-body">
                <div className="shutdown-outcome">
                  <h2 className="shutdown-title">{OUTCOME_SCREENS[phase].title}</h2>
                  <p className="shutdown-note">{OUTCOME_SCREENS[phase].note}</p>
                </div>
              </div>
              <button
                type="button"
                className="shutdown-action shutdown-action--ghost shutdown-cancel"
                data-testid="shutdown-result-back"
                onClick={cancel}
              >
                返回
              </button>
            </div>
          )}
        </div>
      )}
    </>
  )
}
