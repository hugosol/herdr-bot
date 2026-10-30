import { useState } from 'react'

export function ResumeChat({ sessionId }: { sessionId: string }) {
  const [pending, setPending] = useState(false)
  const [result, setResult] = useState<{ failed: boolean; message: string; targets: { label: string; value: string }[] } | null>(null)
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
      if (response.ok && (!('outcome' in body) || !['located', 'restored'].includes(String(body.outcome)))) {
        throw new Error('unconfirmed outcome')
      }
      if (response.ok && 'outcome' in body && body.outcome === 'located') {
        setResult({ failed: false, message: '已切换到当前herdr焦点', targets: [] })
        return
      }
      const targets: { label: string; value: string }[] = []
      if ('target' in body && typeof body.target === 'object' && body.target !== null) {
        for (const [key, label] of [['workspaceId', 'Workspace'], ['tabId', 'Tab'], ['paneId', 'Pane']] as const) {
          if (key in body.target) {
            const value = (body.target as Record<string, unknown>)[key]
            if (typeof value === 'string' && value.length > 0) targets.push({ label, value })
          }
        }
      }
      if ('agentName' in body && typeof body.agentName === 'string' && body.agentName.length > 0) {
        targets.push({ label: 'agent 启动名称（不代表已确认加载）', value: body.agentName })
      }
      setResult({ failed: !response.ok, message: body.message, targets })
    } catch {
      setResult({ failed: true, message: '无法确认 Resume 结果（网络或响应异常）；尚未确认定位或恢复。请检查 Herdr 后再重试。', targets: [] })
    } finally { setPending(false) }
  }
  return <>
    <button type="button" className="history-resume" disabled={pending} onClick={() => void resume()}>{pending ? '正在定位／恢复…' : 'Resume'}</button>
    {result ? <>
      <p role={result.failed ? 'alert' : 'status'}>{result.message}</p>
      {result.targets.length > 0 ? <dl aria-label="Resume 目标">{result.targets.map(({ label, value }) => <div key={label}><dt>{label}</dt><dd>{value}</dd></div>)}</dl> : null}
    </> : null}
  </>
}
