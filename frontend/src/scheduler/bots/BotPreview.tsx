import { useState, type ReactNode } from 'react'
import type { Bot } from '../../controllers/botController'
import { EditableTitle } from '../../app/EditableTitle'

const documents = [
  { key: 'when', title: 'when' },
  { key: 'description', title: 'description' },
  { key: 'input_rule', title: 'input_rule' },
  { key: 'output_rule', title: 'output_rule' },
] as const

export function BotPreview({ bot, target, active, disabled, onRename }: { bot: Bot; target: ReactNode; active: boolean; disabled?: boolean; onRename: (name: string) => Promise<void> }) {
  const [section, setSection] = useState('when')

  return <>
    <p className="scheduler-filename scheduler-reference">{bot.id}</p>
    <EditableTitle key={bot.id} value={bot.name} label="Bot 名称" heading="h3" className="scheduler-text" active={active} disabled={disabled} onSave={onRename} />
    {target}
    <nav className="scheduler-bot-sections" aria-label="Bot 内容分区">
      {documents.map(({ key, title }) => <button type="button" key={key} aria-pressed={section === key} onClick={() => setSection(key)}>{title}</button>)}
      <button type="button" aria-pressed={section === 'raw'} onClick={() => setSection('raw')}>json</button>
    </nav>
    {documents.map(({ key, title }) => <section key={key} aria-label={title} hidden={section !== key}>
      <pre className="scheduler-text scheduler-prose">{bot[key]}</pre>
    </section>)}
    {documents.some(({ key }) => section === key && bot[key] === '') && <p role="status">当前 {section}：空文本</p>}
    <div hidden={section !== 'raw'}><pre aria-label="Bot 原始定义">{JSON.stringify(bot, null, 2)}</pre></div>
    <p className="scheduler-help">四个字段用于能力选用，允许空文本；输入／输出摘要不是运行校验规则。仅保存定义，不启动 Chat。</p>
  </>
}
