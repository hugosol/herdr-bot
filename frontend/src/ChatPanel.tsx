import { useEffect, useState } from 'react'
import { SessionsPanel } from './SessionsPanel'
import type { SessionFeed } from './sessionFeed'

type HistorySession = {
  sessionId: string
  sessionName: string | null
  agentType: string | null
  cwd: string | null
  firstRecordedAt: number
}

type HistoryState = { kind: 'loading' } | { kind: 'loaded'; sessions: HistorySession[] } | { kind: 'error' }

function HistoryPanel() {
  const [state, setState] = useState<HistoryState>({ kind: 'loading' })
  useEffect(() => {
    const controller = new AbortController()
    void fetch('/api/sessions/history', { signal: controller.signal, cache: 'no-store' })
      .then(async (response) => {
        if (!response.ok) throw new Error(`HTTP ${response.status}`)
        const body = await response.json() as { sessions: HistorySession[] }
        if (!controller.signal.aborted) setState({ kind: 'loaded', sessions: body.sessions })
      })
      .catch(() => { if (!controller.signal.aborted) setState({ kind: 'error' }) })
    return () => controller.abort()
  }, [])

  return <section aria-label="历史会话" className="history-panel">
    {state.kind === 'loading' ? <p role="status">正在读取历史会话…</p> : null}
    {state.kind === 'error' ? <p role="alert">读取历史会话失败，未确认历史数据。请退出后重新进入历史视图。</p> : null}
    {state.kind === 'loaded' && state.sessions.length === 0 ? <p>尚无历史会话。成功接入的对话会收录在这里。</p> : null}
    {state.kind === 'loaded' && state.sessions.length > 0 ? <table className="history-table" aria-label="历史会话目录">
      <thead><tr><th>名称</th><th>agent 类型</th><th>cwd</th><th>首次收录时间</th></tr></thead>
      <tbody>{state.sessions.map((session) => <tr key={session.sessionId}>
        <td data-label="名称">{session.sessionName ?? session.sessionId}</td>
        <td data-label="agent 类型">{session.agentType ?? '—'}</td>
        <td data-label="cwd">{session.cwd ?? '—'}</td>
        <td data-label="首次收录时间"><time dateTime={new Date(session.firstRecordedAt).toISOString()}>{new Date(session.firstRecordedAt).toLocaleString('zh-CN')}</time></td>
      </tr>)}</tbody>
    </table> : null}
  </section>
}

/** The current feed keeps running independently; switching views never writes session events. */
export function ChatPanel({ feed }: { feed: SessionFeed }) {
  const [history, setHistory] = useState(false)
  return <div className={`chat-views${history ? ' chat-views--history' : ''}`} data-testid="chat-views">
    <header className="chat-views__header">
      <h2>{history ? '历史会话' : '当前会话'}</h2>
      <button type="button" className="chat-views__toggle" aria-label={history ? '查看当前会话' : '查看历史会话'} onClick={() => setHistory(!history)}>
        <span aria-hidden="true">{history ? '◀' : '▶'}</span>
        <span>{history ? '当前' : '历史'}</span>
      </button>
    </header>
    {history ? <HistoryPanel /> : <SessionsPanel feed={feed} />}
  </div>
}
