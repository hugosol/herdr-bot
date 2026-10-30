import { useEffect, useState } from 'react'
import { SessionsPanel } from './SessionsPanel'
import { ResumeChat } from './ResumeChat'
import type { SessionFeed } from './sessionFeed'

type HistorySession = {
  sessionId: string
  sessionName: string | null
  agentType: string | null
  cwd: string | null
  originalWorkspaceId?: string
  originalWorkspaceName?: string
  memo?: string
  firstRecordedAt: number
}

type HistoryPage = { sessions: HistorySession[]; page: number; total: number; totalPages: number }
type HistoryState = { kind: 'loading' } | ({ kind: 'loaded' } & HistoryPage) | { kind: 'error' }

function HistoryMemo({ session }: { session: HistorySession }) {
  const [memo, setMemo] = useState(session.memo)
  const [draft, setDraft] = useState(session.memo ?? '')
  const [editing, setEditing] = useState(false)
  const [pending, setPending] = useState(false)
  const [error, setError] = useState(false)

  async function save() {
    if (pending) return
    setPending(true)
    setError(false)
    try {
      const response = await fetch('/api/sessions/history/memo', {
        method: 'PUT', headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ sessionId: session.sessionId, memo: draft }),
      })
      if (!response.ok) throw new Error(`HTTP ${response.status}`)
      const body: unknown = await response.json()
      if (typeof body !== 'object' || body === null || !('sessionId' in body) || body.sessionId !== session.sessionId || !('memo' in body) || typeof body.memo !== 'string') {
        throw new Error('unconfirmed memo response')
      }
      setMemo(body.memo)
      setDraft(body.memo)
      setEditing(false)
    } catch {
      setError(true)
    } finally { setPending(false) }
  }

  return <div className="history-memo">
    {editing ? <>
      <label>
        <span>备忘内容</span>
        <textarea aria-label={`${session.sessionName ?? session.sessionId}的备忘`} value={draft} disabled={pending} onChange={(event) => setDraft(event.target.value)} rows={4} />
      </label>
      <div className="history-memo__controls">
        <button type="button" disabled={pending} onClick={() => void save()}>{pending ? '保存中…' : '保存'}</button>
        <button type="button" disabled={pending} onClick={() => { setEditing(false); setDraft(memo ?? ''); setError(false) }}>取消</button>
      </div>
      {error ? <p role="alert">保存备忘失败，草稿已保留。请重试。</p> : null}
    </> : <>
      {memo !== undefined ? <p className="history-memo__text">{memo}</p> : null}
      <button type="button" onClick={() => { setDraft(memo ?? ''); setError(false); setEditing(true) }}>{memo === undefined ? '新增备忘' : '编辑备忘'}</button>
    </>}
  </div>
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
      <thead><tr><th>名称</th><th>agent 类型</th><th>Workspace</th><th>首次收录时间</th><th>备忘</th><th>操作</th></tr></thead>
      <tbody>{state.sessions.map((session) => <tr key={session.sessionId}>
        <td data-label="名称">{session.sessionName ?? session.sessionId}</td>
        <td data-label="agent 类型">{session.agentType ?? '—'}</td>
        <td data-label="Workspace"><div>{session.originalWorkspaceId ? <>{session.originalWorkspaceName ? <span className="history-workspace-name">{session.originalWorkspaceName}</span> : null}<span>{session.originalWorkspaceId}</span></> : '未记录'}</div></td>
        <td data-label="首次收录时间"><time dateTime={new Date(session.firstRecordedAt).toISOString()}>{new Date(session.firstRecordedAt).toLocaleString('zh-CN')}</time></td>
        <td data-label="备忘"><HistoryMemo session={session} /></td>
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
