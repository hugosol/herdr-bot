import { useState } from 'react'
import { ResumeChat } from '../chats/ResumeChat'
import { locateRunChat, type ChatReference } from '../controllers/workflowController'

function LocateChat({ runId, executionId }: { runId: string; executionId?: string }) {
  const [pending, setPending] = useState(false)
  const [result, setResult] = useState<{ failed: boolean; message: string } | null>(null)
  async function locate() {
    setPending(true)
    setResult(null)
    try {
      setResult({ failed: false, message: await locateRunChat(runId, executionId) })
    } catch (error) {
      setResult({ failed: true, message: error instanceof Error ? error.message : '无法确认定位结果。' })
    } finally { setPending(false) }
  }
  return <>
    <button type="button" disabled={pending} onClick={() => void locate()}>{pending ? '正在定位…' : '定位'}</button>
    {result && <p role={result.failed ? 'alert' : 'status'}>{result.message}</p>}
  </>
}

export function RunChatReference({ chat, label, runId, executionId }: { chat: ChatReference | null; label: string; runId: string; executionId?: string }) {
  return <section aria-label={label} className="workflow-saved-chat">
    {chat ? <>
      <div className="workflow-chat-link"><p>{label} · sessionId: <code>{chat.sessionId}</code></p><LocateChat key={`${runId}:${executionId ?? ''}:${chat.sessionId}`} runId={runId} executionId={executionId} /><ResumeChat key={chat.sessionId} sessionId={chat.sessionId} /></div>
      <details><summary>保存的 Chat 关联</summary><dl>{Object.entries(chat).filter(([key]) => key !== 'sessionId').map(([key, value]) => <div key={key}><dt>{key}</dt><dd>{value}</dd></div>)}</dl></details>
    </> : <p>尚无已保存 Chat 关联；不会隐式创建执行。</p>}
  </section>
}
