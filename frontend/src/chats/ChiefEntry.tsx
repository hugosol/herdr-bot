import { useState } from 'react'
import { ChatCreationDialog } from './ChatCreationDialog'

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
      const response = await fetch('/api/chief', { cache: 'no-store' })
      if (!response.ok) throw new Error('unconfirmed authority')
      const body = await response.json()
      if (!body || !Object.hasOwn(body, 'current')) throw new Error('unconfirmed authority')
      if (body.current === null) {
        setCreating(true)
      } else if (typeof body.current.sessionId === 'string') {
        const resumed = await fetch('/api/chats/resume', {
          method: 'POST', headers: { 'content-type': 'application/json' },
          body: JSON.stringify({ sessionId: body.current.sessionId }),
        })
        const result = await resumed.json()
        if (typeof result?.message !== 'string' || (resumed.ok && !['located', 'restored'].includes(result.outcome))) {
          throw new Error('unconfirmed navigation')
        }
        setFeedback({ failed: !resumed.ok, message: result.message })
      } else if (typeof body.current.phase === 'string') {
        setFeedback({ failed: true, message: '当前 chief 正在启动或启动结果尚未确认；不会另建 Chat。' })
      } else throw new Error('unconfirmed authority')
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
