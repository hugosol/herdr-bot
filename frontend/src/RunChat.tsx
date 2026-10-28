import { ResumeChat } from './ResumeChat'
import type { ChatReference } from './workflowApi'

export function SavedChat({ chat, label }: { chat: ChatReference | null; label: string }) {
  return <section aria-label={label}>
    {chat ? <>
      <dl>{Object.entries(chat).map(([key, value]) => <div key={key}><dt>{key}</dt><dd>{value}</dd></div>)}</dl>
      <ResumeChat key={chat.sessionId} sessionId={chat.sessionId} />
    </> : <p>尚无已保存 Chat 关联；不会隐式创建执行。</p>}
  </section>
}
