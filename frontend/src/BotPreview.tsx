import { useState } from 'react'
import type { Bot } from './schedulerApi'

const documents = [
  { key: 'when', title: 'when' },
  { key: 'plan_guide', title: 'plan_guide' },
  { key: 'node_guide', title: 'node_guide' },
] as const

export function BotPreview({ bot }: { bot: Bot }) {
  const [section, setSection] = useState('when')
  return <>
    <p className="scheduler-filename">{bot.id}</p>
    <h3 className="scheduler-text">{bot.name}</h3>
    <p className="scheduler-text">Task 引用：{bot.task_id ?? '尚未绑定 Task'}</p>
    <nav className="scheduler-bot-sections" aria-label="Bot 内容分区">
      {documents.map(({ key, title }) => <button type="button" key={key} aria-pressed={section === key} onClick={() => setSection(key)}>{title}</button>)}
      <button type="button" aria-pressed={section === 'raw'} onClick={() => setSection('raw')}>json</button>
    </nav>
    {documents.map(({ key, title }) => <section key={key} aria-label={title} hidden={section !== key}>
      <pre className="scheduler-text">{bot[key]}</pre>
    </section>)}
    <div hidden={section !== 'raw'}><pre aria-label="Bot 原始定义">{JSON.stringify(bot, null, 2)}</pre></div>
    <p>三份文档可独立阅读、允许空文本；Markdown 是指引，不是输入校验规则。仅保存定义，不启动 Chat。</p>
  </>
}
