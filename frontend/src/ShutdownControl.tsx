import { useEffect, useState } from 'react'

import { SHUTDOWN_BUTTON_VISIBLE } from './config'
import type { ConnectionStatus } from './sessionFeed'
import { NOT_CANCELLABLE, cancelShutdown, startShutdown } from './shutdownApi'
import { remainingSeconds, type ShutdownView } from './shutdownView'

type ShutdownControlProps = {
  /** 手动关机按钮只在控制面板显示；全局提醒不受 tab 可见性影响。 */
  showButton: boolean
  /** 后端权威的关机状态（共享观察流的首帧与 `shutdown` 事件）。 */
  shutdown: ShutdownView
  /** 观察流健康度：中断时本地显示的剩余时间可能已经过期。 */
  connection: ConnectionStatus
  /** 整屏接管是否进行中；页面据此让底层“启动/停止”不可操作。 */
  onTakeoverChange: (takeover: boolean) => void
}

/**
 * 一次命令的本地状态。
 *
 * 它只记录「我们发过什么、得到了什么答复」，绝不冒充后端状态：后端状态只有一个来源（观察流与命令
 * 回执），无回应只是「未确认」，不是接受、不是拒绝，也不是取消成功。
 */
type Command =
  | { kind: 'idle' }
  | { kind: 'starting' }
  | { kind: 'start-unconfirmed' }
  | { kind: 'start-refused'; code: string }
  | { kind: 'cancelling' }
  | { kind: 'cancel-unconfirmed' }
  | { kind: 'cancel-refused'; code: string }

/**
 * 关机按钮与整屏接管（确认页 / 后端倒计时 / 结果）。
 *
 * 倒计时由**后端**拥有（ADR 0005）：这里只按服务端的截止时间与服务端时间显示剩余秒数，本地归零不发
 * 任何关机请求，也不声称电脑已经关闭；取消窗口同样由后端裁定，因此本地显示归零时取消入口仍然可用。
 * 取消只有拿到服务端「取消已生效」的证据才算成功；没有回应时保留提醒并明确「电脑仍可能关机」，
 * 且绝不自动重发。确认页与结果页的「返回」都是纯前端事件。
 */
export function ShutdownControl({ showButton, shutdown, connection, onTakeoverChange }: ShutdownControlProps) {
  const [confirming, setConfirming] = useState(false)
  const [command, setCommand] = useState<Command>({ kind: 'idle' })
  // 命令回执里的权威状态：开始被接受、取消被确认时立刻用它显示，直到观察流给出更新的一份。
  const [bridge, setBridge] = useState<ShutdownView | null>(null)
  // 已经被本页关掉的终局（`阶段:安排标识`）：返回是纯前端事件，不影响后端与其他前端。
  const [dismissed, setDismissed] = useState<string | null>(null)
  // 显示用的“现在”；剩余时间按服务端时钟换算，本机时钟偏差不影响它。
  const [now, setNow] = useState(() => Date.now())

  const view = bridge ?? shutdown

  // 观察流给出新状态后，命令回执就不再顶替它：后端状态始终是唯一权威。
  useEffect(() => {
    setBridge(null)
  }, [shutdown])

  const counting = view.phase === 'counting-down' || view.phase === 'executing'
  const seconds = remainingSeconds(view, now)

  // 倒计时期间刷新显示；没有安排时不留任何定时器。
  useEffect(() => {
    setNow(Date.now())
    if (!counting) return
    const timer = window.setInterval(() => setNow(Date.now()), 500)
    return () => window.clearInterval(timer)
  }, [counting, view.serverTime, view.deadline])

  const key = `${view.phase}:${view.arrangementId ?? ''}`
  const finished =
    (view.phase === 'issued' || view.phase === 'failed' || view.phase === 'cancelled') &&
    dismissed !== key

  let screen = 'none'
  if (confirming) screen = 'confirm'
  else if (finished) screen = view.phase
  // 执行已经开始：到期已经裁定，取消不再可能成功，界面也不再提供它。
  else if (view.phase === 'executing') screen = 'executing'
  else if (view.phase === 'counting-down') screen = seconds > 0 ? 'counting' : 'waiting'
  else if (command.kind === 'starting') screen = 'starting'
  else if (command.kind === 'cancelling') screen = 'cancelling'
  else if (command.kind === 'start-unconfirmed') screen = 'start-unconfirmed'
  else if (command.kind === 'start-refused') screen = 'start-refused'
  else if (command.kind === 'cancel-unconfirmed') screen = 'cancel-unconfirmed'
  else if (command.kind === 'cancel-refused') screen = 'cancel-refused'

  const visible = screen !== 'none'
  useEffect(() => {
    onTakeoverChange(visible)
  }, [visible, onTakeoverChange])

  async function confirmShutdown() {
    setConfirming(false)
    setCommand({ kind: 'starting' })
    const result = await startShutdown()
    if (result.kind === 'accepted') {
      setBridge(result.shutdown)
      setCommand({ kind: 'idle' })
    } else if (result.kind === 'rejected') {
      setCommand({ kind: 'start-refused', code: result.code })
    } else {
      setCommand({ kind: 'start-unconfirmed' })
    }
  }

  async function cancelCountdown() {
    setCommand({ kind: 'cancelling' })
    const result = await cancelShutdown()
    if (result.kind === 'cancelled') {
      // 服务端确认取消生效：立刻按它给的权威状态显示成功，直到观察流给出更新的一份。
      setBridge(result.shutdown)
      setCommand({ kind: 'idle' })
    } else if (result.kind === 'refused') {
      setCommand({ kind: 'cancel-refused', code: result.code })
    } else {
      setCommand({ kind: 'cancel-unconfirmed' })
    }
  }

  const cancelUnconfirmed = command.kind === 'cancel-unconfirmed'
  const cancelRefused = command.kind === 'cancel-refused'
  const stale = connection === 'interrupted'

  /** 取消入口：只有倒计时阶段才有；执行已经开始之后不再提供。 */
  const cancelButton = (screen === 'counting' || screen === 'waiting') && (
    <button
      type="button"
      className="shutdown-action shutdown-action--ghost shutdown-cancel"
      data-testid="shutdown-cancel"
      onClick={cancelCountdown}
      disabled={command.kind === 'cancelling'}
    >
      {command.kind === 'cancelling' ? '正在取消…' : '取消关机'}
    </button>
  )

  /** 取消未确认与取消被拒的提示：无论显示哪一屏，它们都不允许被读成“取消成功”。 */
  const cancelNotes = (
    <>
      {cancelUnconfirmed && (
        <p className="shutdown-warning" data-testid="shutdown-warning">
          取消未确认，电脑仍可能关机。没有收到服务端的确认，无法确认取消是否生效；状态可能已经过期。
        </p>
      )}
      {cancelRefused && (
        <p className="shutdown-warning" data-testid="shutdown-refusal">
          {command.kind === 'cancel-refused' && command.code === NOT_CANCELLABLE
            ? '取消没有生效：服务端已经开始执行关机，电脑仍可能关闭。'
            : '服务端明确拒绝这次取消，电脑仍可能按当前安排关闭。'}
        </p>
      )}
    </>
  )

  return (
    <>
      {showButton && SHUTDOWN_BUTTON_VISIBLE && (
        <button
          type="button"
          className="shutdown"
          data-testid="shutdown-button"
          onClick={() => setConfirming(true)}
        >
          关机
        </button>
      )}

      {visible && (
        <div
          className="shutdown-takeover"
          data-testid="shutdown-takeover"
          data-phase={screen}
          role="dialog"
          aria-modal="true"
          aria-label="关机"
        >
          {screen === 'confirm' && (
            <div className="shutdown-screen shutdown-screen--confirm" data-testid="shutdown-confirm">
              <p className="shutdown-kicker">关机</p>
              <h2 className="shutdown-title">确定要关闭电脑？</h2>
              <p className="shutdown-note">
                确认后由服务端开始倒计时，截止时间之前都可以取消；此前返回不会开始任何安排。
              </p>
              <div className="shutdown-actions">
                <button
                  type="button"
                  className="shutdown-action shutdown-action--ghost"
                  data-testid="shutdown-back"
                  onClick={() => setConfirming(false)}
                >
                  返回
                </button>
                <button
                  type="button"
                  className="shutdown-action shutdown-action--danger"
                  data-testid="shutdown-confirm-button"
                  onClick={confirmShutdown}
                >
                  关机
                </button>
              </div>
            </div>
          )}

          {(screen === 'counting' || screen === 'waiting') && (
            <div
              className="shutdown-screen shutdown-screen--counting"
              data-testid={screen === 'counting' ? 'shutdown-countdown' : 'shutdown-waiting'}
              data-phase={screen}
              data-seconds={seconds}
            >
              <div className="shutdown-countdown-body">
                <div className="shutdown-count" data-testid="shutdown-count">
                  {seconds}
                </div>
                <p className="shutdown-note">
                  {screen === 'counting'
                    ? '秒后关闭电脑'
                    : '本地倒计时已经归零，等待服务端确认执行结果；无法确认电脑是否已经关闭，请不要重复操作。'}
                </p>
                {stale && (
                  <p className="shutdown-warning" data-testid="shutdown-stale">
                    连接已中断：这里的剩余时间可能已经过期，请稍后重新打开页面确认。
                  </p>
                )}
                {cancelNotes}
              </div>
              {cancelButton}
            </div>
          )}

          {screen === 'executing' && (
            <div
              className="shutdown-screen shutdown-screen--outcome"
              data-testid="shutdown-executing"
              data-outcome="executing"
            >
              <div className="shutdown-outcome-body">
                <div className="shutdown-outcome">
                  <h2 className="shutdown-title">关机执行已经开始</h2>
                  <p className="shutdown-note">
                    服务端已经开始关闭电脑，取消已经来不及了；这一页不再提供取消。
                  </p>
                  {stale && (
                    <p className="shutdown-warning" data-testid="shutdown-stale">
                      连接已中断：这里可能不是最新状态，请稍后重新打开页面确认。
                    </p>
                  )}
                  {cancelNotes}
                </div>
              </div>
            </div>
          )}

          {(screen === 'issued' || screen === 'failed' || screen === 'cancelled') && (
            <div
              className="shutdown-screen shutdown-screen--outcome"
              data-testid="shutdown-result"
              data-outcome={screen}
            >
              <div className="shutdown-outcome-body">
                <div className="shutdown-outcome">
                  <h2 className="shutdown-title">
                    {screen === 'issued'
                      ? '关机指令已发出'
                      : screen === 'failed'
                        ? '关机没有成功'
                        : '取消成功'}
                  </h2>
                  <p className="shutdown-note">
                    {screen === 'issued'
                      ? '服务端已发出关机指令，电脑正在关闭。指令已发出不等于已经观察到断电。'
                      : screen === 'failed'
                        ? '服务端明确回报关机没有做成，电脑不会按这次安排关闭；不会自动重试，可以重新走一次关机流程。'
                        : '服务端确认这次安排已经撤销，电脑不会按它关闭。'}
                  </p>
                </div>
              </div>
              <button
                type="button"
                className="shutdown-action shutdown-action--ghost shutdown-cancel"
                data-testid="shutdown-result-back"
                onClick={() => {
                  setDismissed(key)
                  setCommand({ kind: 'idle' })
                }}
              >
                返回
              </button>
            </div>
          )}

          {(screen === 'starting' || screen === 'cancelling') && (
            <div
              className="shutdown-screen shutdown-screen--outcome"
              data-testid="shutdown-requesting"
              data-outcome="unconfirmed"
            >
              <div className="shutdown-outcome-body">
                <p className="shutdown-kicker">关机</p>
                <h2 className="shutdown-title">
                  {screen === 'starting' ? '正在开始关机安排…' : '正在取消…'}
                </h2>
                <p className="shutdown-note">等待服务端答复。</p>
              </div>
            </div>
          )}

          {screen === 'start-unconfirmed' && (
            <div
              className="shutdown-screen shutdown-screen--outcome"
              data-testid="shutdown-start-unconfirmed"
              data-outcome="unconfirmed"
            >
              <div className="shutdown-outcome-body">
                <div className="shutdown-outcome">
                  <h2 className="shutdown-title">开始意图未确认</h2>
                  <p className="shutdown-note" data-testid="shutdown-warning">
                    没有收到服务端的答复，无法确认关机安排是否已经开始；电脑仍可能关机。页面不会自动
                    重发，可以使用 返回 之后重新确认一次。
                  </p>
                </div>
              </div>
              <button
                type="button"
                className="shutdown-action shutdown-action--ghost shutdown-cancel"
                data-testid="shutdown-result-back"
                onClick={() => setCommand({ kind: 'idle' })}
              >
                返回
              </button>
            </div>
          )}

          {screen === 'start-refused' && (
            <div
              className="shutdown-screen shutdown-screen--outcome"
              data-testid="shutdown-start-refused"
              data-outcome="failed"
              data-code={command.kind === 'start-refused' ? command.code : undefined}
            >
              <div className="shutdown-outcome-body">
                <div className="shutdown-outcome">
                  <h2 className="shutdown-title">开始被拒绝</h2>
                  <p className="shutdown-note">服务端明确拒绝了这次开始意图，没有开始新的安排。</p>
                </div>
              </div>
              <button
                type="button"
                className="shutdown-action shutdown-action--ghost shutdown-cancel"
                data-testid="shutdown-result-back"
                onClick={() => setCommand({ kind: 'idle' })}
              >
                返回
              </button>
            </div>
          )}

          {screen === 'cancel-unconfirmed' && (
            // 没有回应：提醒与不确定性都必须留下，而且这里没有「返回」（返回很容易被当成取消成功）。
            <div
              className="shutdown-screen shutdown-screen--outcome"
              data-testid="shutdown-cancel-unconfirmed"
              data-outcome="unconfirmed"
            >
              <div className="shutdown-outcome-body">
                <div className="shutdown-outcome">
                  <h2 className="shutdown-title">取消未确认，电脑仍可能关机</h2>
                  <p className="shutdown-note" data-testid="shutdown-warning">
                    没有收到服务端的确认，无法确认取消是否生效；状态可能已经过期。可以再点一次取消，
                    但页面不会自动重发。
                  </p>
                </div>
              </div>
              <button
                type="button"
                className="shutdown-action shutdown-action--ghost shutdown-cancel"
                data-testid="shutdown-cancel"
                onClick={cancelCountdown}
              >
                再试一次取消
              </button>
            </div>
          )}

          {screen === 'cancel-refused' && (
            <div
              className="shutdown-screen shutdown-screen--outcome"
              data-testid="shutdown-cancel-refused"
              data-outcome="failed"
              data-code={command.kind === 'cancel-refused' ? command.code : undefined}
            >
              <div className="shutdown-outcome-body">
                <div className="shutdown-outcome">
                  <h2 className="shutdown-title">取消没有生效</h2>
                  <p className="shutdown-note" data-testid="shutdown-refusal">
                    {command.kind === 'cancel-refused' && command.code === NOT_CANCELLABLE
                      ? '服务端已经开始执行关机，这次取消不会生效；电脑仍可能关闭。'
                      : '服务端明确拒绝这次取消，电脑仍可能按当前安排关闭。'}
                  </p>
                </div>
              </div>
              <button
                type="button"
                className="shutdown-action shutdown-action--ghost shutdown-cancel"
                data-testid="shutdown-result-back"
                onClick={() => setCommand({ kind: 'idle' })}
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
