import { useEffect, useState } from 'react'
import type { Bot } from './schedulerApi'
import { fetchTasks, type TaskResult } from '../tasks/taskApi'

const documents = [
  { key: 'when', title: 'when' },
  { key: 'plan_guide', title: 'plan_guide' },
  { key: 'node_guide', title: 'node_guide' },
] as const

export function BotPreview({ bot }: { bot: Bot }) {
  const [section, setSection] = useState('when')
  const [taskLookup, setTaskLookup] = useState<{ bot: Bot; result: TaskResult } | null>(null)

  useEffect(() => {
    if (bot.task_id == null) return
    let cancelled = false
    void fetchTasks().then(result => {
      if (!cancelled) setTaskLookup({ bot, result })
    })
    return () => { cancelled = true }
  }, [bot])

  const result = taskLookup?.bot === bot ? taskLookup.result : null
  const task = result?.kind === 'ok' ? result.tasks.find(task => task.id === bot.task_id) : undefined
  const taskName = bot.task_id == null ? '尚未绑定 Task'
    : result === null ? '正在读取 Task 名称…'
      : result.kind !== 'ok' ? `Task 名称读取失败${result.kind === 'rejected' ? `：${result.message}` : '，请刷新重试。'}`
        : task ? task.name || '（Task 未命名）' : `绑定的 Task 不存在（${bot.task_id}）`

  return <>
    <p className="scheduler-filename">{bot.id}</p>
    <h3 className="scheduler-text">{bot.name}</h3>
    <dl aria-label="绑定 Task"><dt>Task</dt><dd className="scheduler-text">{taskName}</dd></dl>
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
