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

type HistoryPage = { sessions: HistorySession[]; page: number; total: number; totalPages: number }
type HistoryState = { kind: 'loading' } | ({ kind: 'loaded' } & HistoryPage) | { kind: 'error' }

function ResumeChat({ sessionId }: { sessionId: string }) {
  const [pending, setPending] = useState(false)
  const [result, setResult] = useState<{ failed: boolean; message: string } | null>(null)
  async function resume() {
    if (pending) return
    setPending(true)
    setResult(null)
    try {
      const response = await fetch('/api/chats/resume', {
        method: 'POST', headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ sessionId }),
      })
      const body: unknown = await response.json()
      if (typeof body !== 'object' || body === null || !('message' in body) || typeof body.message !== 'string') {
        throw new Error('unconfirmed response')
      }
      if (response.ok && (!('outcome' in body) || !['located', 'not-found'].includes(String(body.outcome)))) {
        throw new Error('unconfirmed outcome')
      }
      setResult({ failed: !response.ok, message: body.message })
    } catch {
      setResult({ failed: true, message: '无法确认 Resume 结果（网络或响应异常）；尚未确认定位或恢复。请检查 Herdr 后再重试。' })
    } finally { setPending(false) }
  }
  return <>
    <button type="button" className="history-resume" disabled={pending} onClick={() => void resume()}>{pending ? '正在定位…' : 'Resume'}</button>
    {result ? <p role={result.failed ? 'alert' : 'status'}>{result.message}</p> : null}
  </>
}

function HistoryPanel() {
  const [page, setPage] = useState(1)
  const [state, setState] = useState<HistoryState>({ kind: 'loading' })
  const navigate = (target: number) => {
    setState({ kind: 'loading' })
    setPage(target)
  }
  useEffect(() => {
    const controller = new AbortController()
    void fetch(`/api/sessions/history?page=${page}`, { signal: controller.signal, cache: 'no-store' })
      .then(async (response) => {
        if (!response.ok) throw new Error(`HTTP ${response.status}`)
        const body = await response.json() as HistoryPage
        if (!controller.signal.aborted) setState({ kind: 'loaded', ...body })
      })
      .catch(() => { if (!controller.signal.aborted) setState({ kind: 'error' }) })
    return () => controller.abort()
  }, [page])

  return <section aria-label="历史会话" className="history-panel">
    {state.kind === 'loading' ? <p role="status">正在读取历史会话…</p> : null}
    {state.kind === 'error' ? <p role="alert">读取历史会话失败，未确认历史数据。请退出后重新进入历史视图。</p> : null}
    {state.kind === 'loaded' && state.sessions.length === 0 ? <p>尚无历史会话。成功接入的对话会收录在这里。</p> : null}
    {state.kind === 'loaded' && state.sessions.length > 0 ? <table className="history-table" aria-label="历史会话目录">
      <thead><tr><th>名称</th><th>agent 类型</th><th>cwd</th><th>首次收录时间</th><th>操作</th></tr></thead>
      <tbody>{state.sessions.map((session) => <tr key={session.sessionId}>
        <td data-label="名称">{session.sessionName ?? session.sessionId}</td>
        <td data-label="agent 类型">{session.agentType ?? '—'}</td>
        <td data-label="cwd">{session.cwd ?? '—'}</td>
        <td data-label="首次收录时间"><time dateTime={new Date(session.firstRecordedAt).toISOString()}>{new Date(session.firstRecordedAt).toLocaleString('zh-CN')}</time></td>
        <td data-label="操作"><ResumeChat sessionId={session.sessionId} /></td>
      </tr>)}</tbody>
    </table> : null}
    {state.kind === 'loaded' ? <nav className="history-pagination" aria-label="历史分页">
      <p aria-live="polite">第 {state.page} / {state.totalPages} 页 · 共 {state.total} 条</p>
      <div className="history-pagination__controls">
        <button type="button" disabled={state.page === 1} onClick={() => navigate(1)}>首页</button>
        <button type="button" disabled={state.page === 1} onClick={() => navigate(state.page - 1)}>上一页</button>
        <button type="button" disabled={state.page === state.totalPages} onClick={() => navigate(state.page + 1)}>下一页</button>
        <button type="button" disabled={state.page === state.totalPages} onClick={() => navigate(state.totalPages)}>末页</button>
      </div>
    </nav> : null}
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
