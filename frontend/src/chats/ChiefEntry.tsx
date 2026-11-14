import { useState } from 'react'
import { ChatCreationDialog } from './ChatCreationDialog'
import { openChiefChat } from './controller'

/** Resolve on every click: another browser can acquire or remove the chief. */
export function ChiefEntry() {
  const [pending, setPending] = useState(false)
  const [creating, setCreating] = useState(false)
  const [feedback, setFeedback] = useState<{ failed: boolean; message: string } | null>(null)

  async function open() {
    if (pending) return
    setPending(true)
    setFeedback(null)
    try {
      const result = await openChiefChat()
      if (result.kind === 'absent') {
        setCreating(true)
      } else if (result.kind === 'resumed') {
        setFeedback({ failed: result.failed, message: result.message })
      } else {
        setFeedback({ failed: true, message: '当前 chief 正在启动或启动结果尚未确认；不会另建 Chat。' })
      }
    } catch {
      setFeedback({ failed: true, message: '无法确认当前 chief 或打开结果；不会另建 Chat。请检查服务后重试。' })
    } finally { setPending(false) }
  }

  return <>
    <button type="button" data-action="chief" disabled={pending} onClick={() => void open()} aria-label="chief bot">{pending ? '正在打开…' : 'chief bot'}</button>
    {feedback && <p className="chief-entry-feedback" role={feedback.failed ? 'alert' : 'status'}>{feedback.message}</p>}
    {creating && <ChatCreationDialog chief onClose={() => setCreating(false)} />}
  </>
}
