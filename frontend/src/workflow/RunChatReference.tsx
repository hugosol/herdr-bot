import { ResumeChat } from '../chats/ResumeChat'
import type { ChatReference } from './workflowApi'

export function RunChatReference({ chat, label }: { chat: ChatReference | null; label: string }) {
  return <section aria-label={label} className="workflow-saved-chat">
    {chat ? <>
      <div className="workflow-chat-link"><p>{label} · <code>{chat.sessionId}</code></p><ResumeChat key={chat.sessionId} sessionId={chat.sessionId} /></div>
      <details><summary>保存的 Chat 关联</summary><dl>{Object.entries(chat).filter(([key]) => key !== 'sessionId').map(([key, value]) => <div key={key}><dt>{key}</dt><dd>{value}</dd></div>)}</dl></details>
    </> : <p>尚无已保存 Chat 关联；不会隐式创建执行。</p>}
  </section>
}
