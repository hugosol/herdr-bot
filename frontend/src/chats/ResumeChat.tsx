import { useState } from 'react'
import { resumeChat } from './controller'

export function ResumeChat({ sessionId }: { sessionId: string }) {
  const [pending, setPending] = useState(false)
  const [result, setResult] = useState<{ failed: boolean; message: string; targets: { label: string; value: string }[] } | null>(null)
  async function resume() {
    if (pending) return
    setPending(true)
    setResult(null)
    try {
      const result = await resumeChat(sessionId)
      if (result.located) {
        setResult({ failed: false, message: '已切换到当前herdr焦点', targets: [] })
        return
      }
      const targets: { label: string; value: string }[] = []
      for (const [key, label] of [['workspaceId', 'Workspace'], ['tabId', 'Tab'], ['paneId', 'Pane']] as const) {
        const value = result.target[key]
        if (value !== undefined) targets.push({ label, value })
      }
      if (result.agentName !== undefined) {
        targets.push({ label: 'agent 启动名称（不代表已确认加载）', value: result.agentName })
      }
      setResult({ failed: result.failed, message: result.message, targets })
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
