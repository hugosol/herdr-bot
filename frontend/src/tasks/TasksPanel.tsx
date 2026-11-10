import { useEffect, useMemo, useRef, useState, type FormEvent } from 'react'

import { ChatNameRuleEditor } from './ChatNameRuleEditor'
import { TaskModelControls, changeTaskModel, modelCompatibility } from './TaskModelControls'
import { ChatCreationDialog } from '../chats/ChatCreationDialog'
import { NierTitleText } from '../app/NierTitleText'
import {
  createTask,
  deleteTask,
  fetchTasks,
  updateTask,
  updateTaskGroup,
  type Task,
  type TaskInput,
  type TaskList,
  type TaskOptions,
  type TaskResult,
} from './taskApi'
import { listBots, type Bot } from '../scheduler/schedulerApi'
import { EntityGroups, GroupButton } from '../app/entityGroups'
import { INITIAL_CHAT_NAME, renderChatNameExample } from './chatNaming'

type Feedback = {
  kind: 'success' | 'error'
  text: string
}

type Dialog = { kind: 'create' } | { kind: 'edit'; id: string }

/** 把一次失败归一化成面向使用者的说明；未得到可用响应时明确说“无法确认”。 */
function describeFailure(result: TaskResult, action: string): string {
  return result.kind === 'rejected'
    ? result.message
    : `无法确认 Task${action}结果：请求没有得到可用响应。`
}

/** Task 的展示值：默认值不显示成具体模型或级别。 */
function modelLabel(task: Pick<Task, 'model'>): string {
  return task.model === 'default' ? '沿用 Agent 默认值' : task.model
}

function thinkingLabel(task: Pick<Task, 'thinking'>): string {
  return task.thinking === 'default' ? '沿用 Agent 默认值' : task.thinking
}

function skillLabel(task: Pick<Task, 'skill'>): string {
  return task.skill === null ? '无' : `/${task.skill}`
}

function suiteIssue(task: Pick<Task, 'suite' | 'agent'>, options: TaskOptions): string | null {
  if (task.suite == null) return null
  const suite = options.suites?.find(suite => suite.id === task.suite)
  if (!suite) return `Suite「${task.suite}」不存在；请显式修正引用。`
  if (!suite.agents.includes(task.agent)) return `Suite「${task.suite}」不适用于 ${task.agent}。`
  const missing = suite.mcp.filter(id => !options.mcp?.includes(id))
  if (missing.length) return `Suite「${task.suite}」的集中 MCP 引用失效：${missing.join('、')}；请显式修正引用。`
  return null
}
function suiteLabel(task: Pick<Task, 'suite'>, options: TaskOptions): string {
  return task.suite == null ? '不选 Suite' : `${options.suites?.find(suite => suite.id === task.suite)?.name ?? task.suite} · ${task.suite}`
}

/** 新增 Task 的初始表单：agent 必须有固定选择，其余使用默认值。 */
function emptyDraft(): TaskInput {
  return {
    name: '',
    chatName: INITIAL_CHAT_NAME,
    agent: 'omp',
    taskType: 'normal',
    model: 'default',
    thinking: 'default',
    skill: null,
    prompt: '',
    suite: null,
  }
}

/** 编辑时把已保存值放回表单。 */
function draftFrom(task: Task): TaskInput {
  return {
    name: task.name,
    chatName: task.chatName,
    agent: task.agent,
    taskType: task.taskType,
    model: task.model,
    thinking: task.thinking,
    skill: task.skill,
    prompt: task.prompt,
    suite: task.suite ?? null,
  }
}

/**
 * Task：`task` 入口中的新增、查看、编辑与删除。
 *
 * Task 由后端持久保存，电脑与手机读取同一份。页面不缓存列表：每次进入 task 入口都重新读取，
 * 保存/删除成功后才用后端返回的完整列表更新界面；保存失败不表示已保存。
 */
export function TasksPanel({ active, editRequest, onEditRequestHandled, detailRequest, onBot }: {
  active: boolean
  editRequest: { id: string } | null
  onEditRequestHandled: () => void
  /** Fresh object per cross-link: discard the draft, refresh saved data, clear filters and reveal details. */
  detailRequest?: { id: string }
  /** Navigate to the sole Bot whose saved task_id references this Task. */
  onBot?: (id: string) => void
}) {
  const [data, setData] = useState<TaskList | null>(null)
  const [loading, setLoading] = useState(false)
  const [listFailure, setListFailure] = useState<string | null>(null)
  const [feedback, setFeedback] = useState<Feedback | null>(null)
  const [dialog, setDialog] = useState<Dialog | null>(null)
  const [draft, setDraft] = useState<TaskInput>(emptyDraft)
  const [formError, setFormError] = useState<string | null>(null)
  const [notice, setNotice] = useState<string | null>(null)
  const [saving, setSaving] = useState(false)
  const [step, setStep] = useState<1 | 2 | 3>(1)
  const [deleting, setDeleting] = useState<string | null>(null)
  const [deleteTarget, setDeleteTarget] = useState<Task | null>(null)
  const deleteDialog = useRef<HTMLDialogElement>(null)
  const editorDialog = useRef<HTMLDialogElement>(null)
  const editorTrigger = useRef<HTMLElement | null>(null)
  const [launchTaskId, setLaunchTaskId] = useState<string | null>(null)
  const [query, setQuery] = useState('')
  const [agentFilter, setAgentFilter] = useState('all')
  const [bots, setBots] = useState<Bot[] | null>(null)
  const [botFailure, setBotFailure] = useState<string | null>(null)
  const [botRefresh, setBotRefresh] = useState(0)
  const botsByTask = useMemo(() => {
    const links = new Map<string, Bot[]>()
    for (const bot of bots ?? []) {
      if (!bot.task_id) continue
      const linked = links.get(bot.task_id)
      if (linked) linked.push(bot)
      else links.set(bot.task_id, [bot])
    }
    return links
  }, [bots])
  const [detailId, setDetailId] = useState<string | null>(null)
  const targetDetail = useRef<HTMLDetailsElement>(null)
  const revealDetail = useRef(false)
  const editorRequest = useRef(0)

  useEffect(() => {
    if (!active) return
    const controller = new AbortController()
    setBots(null)
    setBotFailure(null)
    void listBots(controller.signal).then(catalog => {
      if (!controller.signal.aborted) setBots(catalog.bots)
    }).catch(error => {
      if (!controller.signal.aborted) setBotFailure(error instanceof Error ? error.message : '共享 Bot 目录读取失败。')
    })
    return () => controller.abort()
  }, [active, botRefresh])

  useEffect(() => {
    if (!detailRequest) return
    discardDraft()
    setDetailId(null)
    setQuery('')
    setAgentFilter('all')
    setLoading(true)
    let cancelled = false
    void fetchTasks().then(result => {
      if (cancelled) return
      setLoading(false)
      if (result.kind !== 'ok') {
        setData(null)
        setListFailure(describeFailure(result, '详情读取'))
        return
      }
      setData({ tasks: result.tasks, options: result.options })
      setListFailure(null)
      if (result.tasks.some(task => task.id === detailRequest.id)) {
        revealDetail.current = true
        setDetailId(detailRequest.id)
      } else setFeedback({ kind: 'error', text: '目标 Task 不存在：可能已被删除。' })
    })
    return () => { cancelled = true }
  }, [detailRequest])

  useEffect(() => {
    const element = targetDetail.current
    if (!active || !element || !revealDetail.current) return
    revealDetail.current = false
    element.open = true
    const group = element.closest('.entity-group')
    if (group instanceof HTMLDetailsElement) group.open = true
    element.querySelector('summary')?.focus({ preventScroll: true })
    element.scrollIntoView?.({ block: 'nearest' })
  }, [active, data, detailId])

  function discardDraft() {
    editorRequest.current += 1
    editorTrigger.current = null
    closeDialog()
    setDraft(emptyDraft())
    setLoading(false)
    setStep(1)
    setDeleteTarget(null)
    setLaunchTaskId(null)
    setFeedback(null)
  }

  useEffect(() => {
    const element = editorDialog.current
    if (dialog === null || element === null) return
    element.showModal()
    element.querySelector<HTMLInputElement>('#task-name')?.focus({ preventScroll: true })
    return () => {
      element.close()
      editorTrigger.current?.focus({ preventScroll: true })
    }
  }, [dialog])

  useEffect(() => {
    const element = deleteDialog.current
    if (deleteTarget === null || element === null) return
    // 由原生弹窗聚焦首个按钮；提前 autoFocus 会丢失用于关闭后恢复的触发按钮。
    element.showModal()
    return () => element.close()
  }, [deleteTarget])

  // 进入这个顶层入口时读取后端共享列表；切走时取消尚未落地的读取结果。
  useEffect(() => {
    if (!active) return

    let cancelled = false
    setLoading(true)
    void fetchTasks().then((result) => {
      if (cancelled) return
      setLoading(false)
      if (result.kind === 'ok') {
        setData({ tasks: result.tasks, options: result.options })
        setListFailure(null)
      } else {
        setData(null)
        setListFailure(describeFailure(result, '列表读取'))
      }
    })
    return () => {
      cancelled = true
    }
  }, [active])

  useEffect(() => {
    if (!active || !editRequest) return
    void openEditor(editRequest.id)
    onEditRequestHandled()
  }, [active, editRequest])

  async function openEditor(taskId?: string) {
    const request = ++editorRequest.current
    editorTrigger.current = document.activeElement instanceof HTMLElement ? document.activeElement : null
    setLoading(true)
    const result = await fetchTasks()
    if (request !== editorRequest.current) return
    setLoading(false)
    if (result.kind !== 'ok') {
      setFeedback({ kind: 'error', text: describeFailure(result, '选项读取') })
      return
    }
    setData({ tasks: result.tasks, options: result.options })
    const current = taskId === undefined ? undefined : result.tasks.find((entry) => entry.id === taskId)
    if (taskId !== undefined && !current) {
      setFeedback({ kind: 'error', text: '目标 Task 不存在：可能已被删除。' })
      return
    }
    setDialog(current ? { kind: 'edit', id: current.id } : { kind: 'create' })
    setStep(1)
    setDraft(current ? draftFrom(current) : { ...emptyDraft(), agent: result.options.agents[0] })
    setFormError(null)
    setNotice(null)
  }

  function openCreate() { void openEditor() }
  function openEdit(task: Task) { void openEditor(task.id) }

  function closeDialog() {
    setDialog(null)
    setFormError(null)
    setNotice(null)
  }

  function changeAgent(agent: TaskInput['agent']) {
    // 切换 Agent 不继承模型或同名扩展；切回也不恢复隐藏选择。
    setDraft((current) => ({
      ...current,
      agent,
      model: 'default',
      thinking: 'default',
      }))
    setNotice(null)
  }

  function changeType(taskType: TaskInput['taskType']) {
    setDraft((current) => ({ ...current, taskType }))
    setNotice(null)
  }

  function changeModel(model: TaskInput['model']) {
    if (data === null) return
    const next = changeTaskModel(draft, model, data.options)
    setDraft(current => ({ ...current, model: next.model, thinking: next.thinking }))
    setNotice(next.notice)
  }

  function closeDeleteDialog() {
    deleteDialog.current?.close()
    setDeleteTarget(null)
  }

  async function remove(task: Task) {
    closeDeleteDialog()
    setDeleting(task.id)
    setFeedback(null)
    const result = await deleteTask(task.id)
    setDeleting(null)
    if (result.kind === 'ok') {
      setData({ tasks: result.tasks, options: result.options })
      setListFailure(null)
      setFeedback({ kind: 'success', text: `已删除 Task：${task.name}` })
    } else {
      // 删除失败时列表保持此前从后端读到的值。
      setFeedback({ kind: 'error', text: describeFailure(result, '删除') })
    }
  }

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    if (dialog === null) return
    if (step !== 3) {
      setStep(step === 1 ? 2 : 3)
      return
    }

    if (draft.name.trim() === '') {
      setFormError('请填写 Task 显示名。')
      return
    }
    if (draft.chatName.trim() === '') {
      setFormError('请填写 chat-name 命名规则。')
      return
    }
    if (/[\r\n]/.test(draft.chatName)) {
      setFormError('chat-name 必须是单行规则：固定文字与占位符不能包含换行。')
      return
    }

    setSaving(true)
    setFormError(null)
    const result =
      dialog.kind === 'create'
        ? await createTask(draft)
        : await updateTask(dialog.id, draft)
    setSaving(false)

    if (result.kind === 'ok') {
      setData({ tasks: result.tasks, options: result.options })
      setListFailure(null)
      setDialog(null)
      setFeedback({ kind: 'success', text: `已保存 Task：${draft.name}` })
    } else {
      // 保存失败不改变列表，也不显示“已保存”。
      setFormError(describeFailure(result, '保存'))
      setFeedback(null)
    }
  }

  const { modelUnavailable, thinkingUnavailable } = data === null
    ? { modelUnavailable: false, thinkingUnavailable: false }
    : modelCompatibility(draft, data.options)
  const compatibleSuites = (data?.options.suites ?? []).filter(suite => suite.agents.includes(draft.agent))
  const selectedSuiteIssue = data ? suiteIssue(draft, data.options) : null
  const selectionErrors = [
    ...(selectedSuiteIssue ? [selectedSuiteIssue] : []),
    ...(modelUnavailable ? [`model「${draft.model}」不可用`] : []),
    ...(thinkingUnavailable ? [`thinking「${draft.thinking}」不可用`] : []),
  ]

  const normalizedQuery = query.trim().toLocaleLowerCase()
  const visibleTasks = (data?.tasks ?? []).filter(task =>
    (agentFilter === 'all' || task.agent === agentFilter) &&
    (!normalizedQuery || [task.name, task.agent, task.taskType, task.model, task.skill ?? '', task.suite ?? '',
      data?.options.suites?.find(suite => suite.id === task.suite)?.name ?? '']
      .some(value => value.toLocaleLowerCase().includes(normalizedQuery))))

  return (
    <section className="tasks" data-testid="tasks" aria-labelledby="tasks-title">
      <header className="tasks__header">
        <div>
          <h1 id="tasks-title"><NierTitleText>Task</NierTitleText></h1>
          <p>可复用的对话预设，连接 Agent 与场景工具集。</p>
        </div>
        <button
          type="button"
          className="tasks__create"
          data-testid="task-create-open"
          disabled={data === null}
          onClick={openCreate}
        >
          新增 Task
        </button>
      </header>

      {data !== null && <div className="tasks__toolbar">
        <label className="tasks__search">
          <span>搜索 Task</span>
          <input type="search" value={query} onChange={event => setQuery(event.target.value)}
            placeholder="名称、模型或 Suite" />
        </label>
        <label className="tasks__filter">
          <span>Agent</span>
          <select value={agentFilter} onChange={event => setAgentFilter(event.target.value)}>
            <option value="all">全部 Agent</option>
            {data.options.agents.map(agent => <option key={agent} value={agent}>{data.options.labels?.agents?.[agent] ?? agent}</option>)}
          </select>
        </label>
        <p className="tasks__count" role="status">{visibleTasks.length} / {data.tasks.length} 个 Task</p>
        {(query || agentFilter !== 'all') && <button className="tasks__reset" type="button"
          onClick={() => { setQuery(''); setAgentFilter('all') }}>清除筛选</button>}
      </div>}

      {feedback !== null && (
        <p
          className="zone-feedback"
          data-testid="task-feedback"
          data-kind={feedback.kind}
          role={feedback.kind === 'error' ? 'alert' : 'status'}
        >
          {feedback.text}
        </p>
      )}

      {loading && data === null && (
        <p className="zones__loading" data-testid="task-loading" role="status">
          正在读取 Task……
        </p>
      )}

      {botFailure !== null && <p className="zone-error" role="alert" data-testid="task-bot-error">
        无法读取关联 Bot：{botFailure} <button type="button" className="zone-dialog__cancel" onClick={() => setBotRefresh(value => value + 1)}>重试</button>
      </p>}

      {listFailure !== null && (
        <p className="zone-error" data-testid="task-list-error" role="alert">
          {listFailure}
        </p>
      )}

      {data !== null &&
        (data.tasks.length === 0 ? (
          <p className="zones__empty" data-testid="task-empty">
            尚未保存任何 Task。
          </p>
        ) : (
          <div data-testid="task-list">
            {visibleTasks.length === 0 && <p className="tasks__empty">没有匹配的 Task。试试其他关键词，或清除筛选。</p>}
            {<EntityGroups items={visibleTasks} groupOf={task => task.group} listClass="task-list">{(task) => (
              <li
                key={task.id}
                className="task-item"
                data-testid="task-item"
                data-task-id={task.id}
              >
                <div className="task-item__overview">
                  <div className="task-item__identity">
                    <h2 className="task-item__name">{task.name}</h2>
                    <p className="task-item__meta"><span className="task-item__agent">{task.agent}</span><span>{task.taskType === 'manager' ? '流程管理者' : '普通 Task'}</span></p>
                  </div>
                  <div className="task-item__model"><span className="task-item__label">模型 / 思考</span><p>{modelLabel(task)}</p><small>思考：{thinkingLabel(task)}</small></div>
                  <div className="task-item__suite"><span className="task-item__label">Suite</span><p>{suiteLabel(task, data.options)}</p></div>
                  <div className="task-item__actions">
                    {(() => {
                      const linked = botsByTask.get(task.id) ?? []
                      const unique = linked.length === 1 ? linked[0] : null
                      const label = botFailure ? 'Bot 读取失败' : bots === null ? '正在读取 Bot…' : linked.length > 1 ? '关联冲突' : unique ? `Bot · ${unique.name}` : '无关联 Bot'
                      return <button type="button" className="zone-dialog__cancel task-item__bot" data-testid="task-linked-bot"
                        disabled={bots === null || botFailure !== null || unique === null || !onBot || saving || deleting !== null}
                        onClick={() => { if (unique && onBot) { discardDraft(); onBot(unique.id) } }}>{label}</button>
                    })()}
                    <button type="button" className="zone-dialog__cancel task-item__launch chat-launch-button" data-testid="task-create-chat"
                      disabled={saving || deleting !== null} onClick={() => setLaunchTaskId(task.id)}>发起 Chat</button>
                    <GroupButton name={task.name} group={task.group} names={data.tasks.map(item => item.group)} disabled={saving || deleting !== null} onSave={async group => { const result = await updateTaskGroup(task.id, group); if (result.kind !== 'ok') throw new Error(describeFailure(result, '分组保存')); setData({ tasks: result.tasks, options: result.options }) }} />
                    <button type="button" className="zone-dialog__cancel task-item__edit" data-testid="task-edit"
                      disabled={saving || deleting !== null} onClick={() => openEdit(task)}>编辑</button>
                    <button type="button" className="zone-dialog__cancel task-item__delete" data-testid="task-delete"
                      disabled={saving || deleting !== null} onClick={() => setDeleteTarget(task)}>{deleting === task.id ? '删除中…' : '删除'}</button>
                  </div>
                </div>
                {suiteIssue(task, data.options) && <p className="zone-error" role="alert">{suiteIssue(task, data.options)}</p>}
                <details className="task-item__details" ref={detailId === task.id ? targetDetail : undefined}>
                  <summary>查看配置与提示词</summary>
                <dl className="task-item__fields">
                  <div><dt>Task 类型</dt><dd>{task.taskType}</dd></div>
                  <div><dt>Suite</dt><dd className="task-item__reference">{suiteLabel(task, data.options)}</dd></div>
                  <div>
                    <dt>chat-name 示例</dt>
                    <dd className="task-item__reference">{renderChatNameExample(task.chatName, task.name, task.agent)}</dd>
                  </div>
                  <div>
                    <dt>agent</dt>
                    <dd>{task.agent}</dd>
                  </div>
                  <div>
                    <dt>model</dt>
                    <dd className="task-item__resource">{modelLabel(task)}</dd>
                  </div>
                  <div>
                    <dt>thinking</dt>
                    <dd>{thinkingLabel(task)}</dd>
                  </div>
                  <div>
                    <dt>Skill</dt>
                    <dd className="task-item__resource">{skillLabel(task)}</dd>
                  </div>
                </dl>
                <pre className="task-item__prompt" data-testid="task-prompt-view">
                  {task.prompt || '未设置固定 prompt。'}
                </pre>
                </details>
              </li>
            )}</EntityGroups>}
          </div>
        ))}

      {launchTaskId !== null && <ChatCreationDialog initialTaskId={launchTaskId} onClose={() => setLaunchTaskId(null)} />}

      {deleteTarget !== null && (
        <dialog
          ref={deleteDialog}
          className="zone-dialog task-delete-dialog"
          data-testid="task-delete-dialog"
          aria-labelledby="task-delete-question"
          onCancel={(event) => {
            event.preventDefault()
            closeDeleteDialog()
          }}
        >
          <h2 id="task-delete-question" className="zone-dialog__title">
            是否删除 Task「{deleteTarget.name}」？
          </h2>
          <div className="zone-dialog__actions">
            <button
              type="button"
              className="zone-dialog__cancel"
              data-testid="task-delete-cancel"
              onClick={closeDeleteDialog}
            >
              取消
            </button>
            <button
              type="button"
              className="zone-dialog__cancel task-delete-confirm"
              data-testid="task-delete-confirm"
              onClick={() => void remove(deleteTarget)}
            >
              删除
            </button>
          </div>
        </dialog>
      )}

      {dialog !== null && data !== null && (
        <dialog ref={editorDialog} className="task-editor-shell"
          aria-label={dialog.kind === 'create' ? '新增 Task' : '编辑 Task'}
          onCancel={event => { event.preventDefault(); if (!saving) closeDialog() }}>
          <form
            className="zone-dialog task-dialog"
            data-testid="task-dialog"
            onSubmit={(event) => void submit(event)}
          >
            {dialog.kind === 'edit' && (() => { const task = data.tasks.find(item => item.id === dialog.id); return task && <GroupButton name={task.name} group={task.group} names={data.tasks.map(item => item.group)} disabled={saving} onSave={async group => { const result = await updateTaskGroup(task.id, group); if (result.kind !== 'ok') throw new Error(describeFailure(result, '分组保存')); setData({ tasks: result.tasks, options: result.options }) }} /> })()}
            <h2 className="zone-dialog__title">
              {dialog.kind === 'create' ? '新增 Task' : '编辑 Task'}
            </h2>

            <p data-testid="task-step" role="status">
              {step} / 3 · {step === 1 ? 'Task 预设' : step === 2 ? '扩展配置' : '确认保存'}
            </p>
            <ol className="task-steps" aria-label="Task 编辑步骤">
              {['Task 预设', '扩展配置', '确认保存'].map((label, index) => (
                <li key={label} aria-current={step === index + 1 ? 'step' : undefined}>{label}</li>
              ))}
            </ol>

            {selectionErrors.length > 0 && (
              <p className="zone-error" data-testid="task-selection-error" role="alert">
                {selectionErrors.join('；')}；请显式修正选择后再保存或创建。仅查看或切换步骤不会修正已保存 Task。
              </p>
            )}

            {step === 1 && <>
            <section className="task-preview-card" aria-label="Chat 名称实时示例">
              <span className="task-preview-card__label">Chat 名称 · 实时示例</span>
              <output
                className="task-preview"
                data-testid="chat-name-preview"
                aria-live="polite"
              >
                {renderChatNameExample(draft.chatName, draft.name, draft.agent) || '（空规则）'}
              </output>
              <small className="task-preview-card__hint">
                示例 zone：alpha；示例短标识：a7f3c2。发起 Chat 时由后端使用真实值。
              </small>
            </section>

            <div className="task-compose">
              <fieldset className="task-settings">
                <legend>Agent 与 Task 设置</legend>

                <label className="zone-form__label" htmlFor="task-name">
                  Task 显示名
                </label>
                <input
                  id="task-name"
                  className="zone-form__input"
                  data-testid="task-name"
                  type="text"
                  value={draft.name}
                  onChange={(event) =>
                    setDraft((current) => ({ ...current, name: event.target.value }))
                  }
                  autoComplete="off"
                  disabled={saving}
                />
                <p className="task-hint">
                  必填；只是 Task 的显示名，不是新 Chat 的固定名字。
                </p>

                <label className="zone-form__label" htmlFor="task-type">Task 类型</label>
                <select id="task-type" data-testid="task-type" className="zone-form__input zone-form__select"
                  value={draft.taskType} onChange={(event) => changeType(event.target.value as TaskInput['taskType'])} disabled={saving}>
                  <option value="normal">普通（normal）</option>
                  <option value="manager">流程管理者（manager）</option>
                </select>
                <p className="task-hint">普通 Task 支持独立启动和 Bot 节点绑定；manager 专用于流程管理。两种类型均支持 omp 和 pi，节点角色仅在实际流程执行时加载。</p>

                <label className="zone-form__label" htmlFor="task-agent">
                  agent
                </label>
                <select
                  id="task-agent"
                  className="zone-form__input zone-form__select"
                  data-testid="task-agent"
                  value={draft.agent}
                  onChange={(event) => changeAgent(event.target.value as TaskInput['agent'])}
                  disabled={saving}
                >
                  {data.options.agents.map((agent) => (
                    <option key={agent} value={agent}>
                      {data.options.labels?.agents?.[agent] ?? agent}
                    </option>
                  ))}
                </select>

                <TaskModelControls value={draft} options={data.options} disabled={saving}
                  onModel={changeModel}
                  onThinking={thinking => setDraft(current => ({ ...current, thinking }))} />

                {notice !== null && (
                  <p className="zone-feedback" data-testid="thinking-notice" role="status">
                    {notice}
                  </p>
                )}

                <label className="zone-form__label" htmlFor="task-skill">
                  Skill
                </label>
                <select
                  id="task-skill"
                  className="zone-form__input zone-form__select"
                  data-testid="task-skill"
                  value={draft.skill ?? ''}
                  onChange={(event) =>
                    setDraft((current) => ({
                      ...current,
                      skill:
                        event.target.value === ''
                          ? null
                          : (event.target.value as NonNullable<TaskInput['skill']>),
                    }))
                  }
                  disabled={saving}
                >
                  <option value="">无</option>
                  {data.options.skills.map((skill) => (
                    <option key={skill} value={skill}>
                      {data.options.labels?.skills?.[skill] ?? `/${skill}`}
                    </option>
                  ))}
                </select>

                <label className="zone-form__label" htmlFor="task-prompt">
                  固定 prompt（可多行、可为空）
                </label>
                <textarea
                  id="task-prompt"
                  className="zone-form__input task-prompt"
                  data-testid="task-prompt"
                  value={draft.prompt}
                  onChange={(event) =>
                    setDraft((current) => ({ ...current, prompt: event.target.value }))
                  }
                  rows={5}
                  disabled={saving}
                />
              </fieldset>
              <div className="task-compose__primary">
                <ChatNameRuleEditor
                  value={draft.chatName}
                  onChange={(chatName) => setDraft((current) => ({ ...current, chatName }))}
                  disabled={saving}
                />
              </div>
            </div>
            </>}

            {step === 2 && <section aria-label="扩展配置">
              <label htmlFor="task-suite">Suite（至多一个）</label>
              <select id="task-suite" data-testid="task-suite" className="zone-form__input" disabled={saving}
                value={draft.suite ?? ''} onChange={event => setDraft(current => ({ ...current, suite: event.target.value || null }))}>
                <option value="">不选 Suite</option>
                {draft.suite != null && !compatibleSuites.some(suite => suite.id === draft.suite) && <option value={draft.suite} disabled>{draft.suite}（不可用，保留引用）</option>}
                {compatibleSuites.map(suite => <option key={suite.id} value={suite.id}>{suite.name} · {suite.id}</option>)}
              </select>
              {selectedSuiteIssue && <p className="zone-error" data-testid="task-suite-issue" role="alert">{selectedSuiteIssue}</p>}
              <p>Suite 不替代初始 Skill、固定 prompt、模型或命名规则。切换 Agent 不会自动更换 Suite。</p>
              <p>扩展自动发现：关闭。创建时仅加载对应 Agent 的固定项与所选 Suite 声明。</p>
            </section>}

            {step === 3 && <section data-testid="task-summary" aria-label="确认保存摘要">
              <h3>确认保存</h3>
              <dl className="task-item__fields">
                <div><dt>Task 显示名</dt><dd data-testid="summary-name">{draft.name}</dd></div>
                <div><dt>chat-name 命名规则</dt><dd data-testid="summary-chatName">{draft.chatName}</dd></div>
                <div><dt>Chat 名称示例</dt><dd>{renderChatNameExample(draft.chatName, draft.name, draft.agent)}</dd></div>
                <div><dt>Task 类型</dt><dd data-testid="summary-taskType">{draft.taskType}</dd></div>
                <div><dt>Suite</dt><dd data-testid="summary-suite">{suiteLabel(draft, data.options)}</dd></div>
                <div><dt>agent</dt><dd data-testid="summary-agent">{data.options.labels?.agents?.[draft.agent] ?? draft.agent}</dd></div>
                <div><dt>model</dt><dd data-testid="summary-model">{data.options.labels?.models?.[draft.model] ?? modelLabel(draft)}</dd></div>
                <div><dt>thinking</dt><dd data-testid="summary-thinking">{thinkingLabel(draft)}</dd></div>
                <div><dt>Skill</dt><dd data-testid="summary-skill">{draft.skill === null ? '无' : (data.options.labels?.skills?.[draft.skill] ?? skillLabel(draft))}</dd></div>
                <div><dt>扩展自动发现</dt><dd data-testid="summary-discovery">关闭</dd></div>
              </dl>
              <h4>固定 prompt</h4>
              <pre className="task-item__prompt" data-testid="summary-prompt">{draft.prompt}</pre>
              <p>仅在点击保存并收到成功确认后更新共享 Task；摘要不表示扩展已实际加载。</p>
            </section>}

            {formError !== null && (
              <p
                className="zone-feedback"
                data-testid="task-form-error"
                data-kind="error"
                role="alert"
              >
                {formError}
              </p>
            )}

            <div className="zone-dialog__actions">
              <button
                type="button"
                className="zone-dialog__cancel"
                data-testid="task-cancel"
                disabled={saving}
                onClick={closeDialog}
              >
                取消
              </button>
              {step !== 1 && <button type="button" className="zone-dialog__cancel"
                data-testid="task-back" disabled={saving}
                onClick={() => setStep(step === 3 ? 2 : 1)}>上一步</button>}
              {step === 3 ? <button
                type="submit"
                className="zone-register"
                data-testid="save-task"
                disabled={saving}
              >
                {saving ? '正在保存……' : '保存'}
              </button> : <button type="submit" className="zone-register"
                data-testid="task-next" disabled={saving}>下一步</button>}
            </div>
          </form>
        </dialog>
      )}
    </section>
  )
}
