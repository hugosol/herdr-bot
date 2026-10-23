import { useEffect, useState, type FormEvent } from 'react'

import { ChatNameRuleEditor } from './ChatNameRuleEditor'
import {
  createBot,
  deleteBot,
  fetchBots,
  updateBot,
  type Bot,
  type BotInput,
  type BotList,
  type BotOptions,
  type BotResult,
} from './botApi'
import { INITIAL_CHAT_NAME, renderChatNameExample } from './chatNaming'

type Feedback = {
  kind: 'success' | 'error'
  text: string
}

type Dialog = { kind: 'create' } | { kind: 'edit'; id: string }

/** 把一次失败归一化成面向使用者的说明；未得到可用响应时明确说“无法确认”。 */
function describeFailure(result: BotResult, action: string): string {
  return result.kind === 'rejected'
    ? result.message
    : `无法确认 Bot${action}结果：请求没有得到可用响应。`
}

/** Bot 的展示值：默认值不显示成具体模型或级别。 */
function modelLabel(bot: Pick<Bot, 'model'>): string {
  return bot.model === 'default' ? '沿用 Agent 默认值' : bot.model
}

function thinkingLabel(bot: Pick<Bot, 'thinking'>): string {
  return bot.thinking === 'default' ? '沿用 Agent 默认值' : bot.thinking
}

function skillLabel(bot: Pick<Bot, 'skill'>): string {
  return bot.skill === null ? '无' : `/${bot.skill}`
}

/** 新增 Bot 的初始表单：agent 必须有固定选择，其余使用默认值。 */
function emptyDraft(): BotInput {
  return {
    name: '',
    chatName: INITIAL_CHAT_NAME,
    agent: 'omp',
    model: 'default',
    thinking: 'default',
    skill: null,
    prompt: '',
    extensions: [],
  }
}

/** 编辑时把已保存值放回表单。 */
function draftFrom(bot: Bot): BotInput {
  return {
    name: bot.name,
    chatName: bot.chatName,
    agent: bot.agent,
    model: bot.model,
    thinking: bot.thinking,
    skill: bot.skill,
    prompt: bot.prompt,
    extensions: [...bot.extensions],
  }
}

/**
 * Bot：`bot` 入口中的新增、查看、编辑与删除。
 *
 * Bot 由后端持久保存，电脑与手机读取同一份。页面不缓存列表：每次进入 bot 入口都重新读取，
 * 保存/删除成功后才用后端返回的完整列表更新界面；保存失败不表示已保存。
 */
export function BotsPanel({ active }: { active: boolean }) {
  const [data, setData] = useState<BotList | null>(null)
  const [loading, setLoading] = useState(false)
  const [listFailure, setListFailure] = useState<string | null>(null)
  const [feedback, setFeedback] = useState<Feedback | null>(null)
  const [dialog, setDialog] = useState<Dialog | null>(null)
  const [draft, setDraft] = useState<BotInput>(emptyDraft)
  const [formError, setFormError] = useState<string | null>(null)
  const [notice, setNotice] = useState<string | null>(null)
  const [saving, setSaving] = useState(false)
  const [step, setStep] = useState<1 | 2 | 3>(1)
  const [deleting, setDeleting] = useState<string | null>(null)

  // 进入这个顶层入口时读取后端共享列表；切走时取消尚未落地的读取结果。
  useEffect(() => {
    if (!active) return

    let cancelled = false
    setLoading(true)
    void fetchBots().then((result) => {
      if (cancelled) return
      setLoading(false)
      if (result.kind === 'ok') {
        setData({ bots: result.bots, options: result.options })
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

  async function openEditor(bot?: Bot) {
    setLoading(true)
    const result = await fetchBots()
    setLoading(false)
    if (result.kind !== 'ok') {
      setFeedback({ kind: 'error', text: describeFailure(result, '选项读取') })
      return
    }
    setData({ bots: result.bots, options: result.options })
    const current = bot ? result.bots.find((entry) => entry.id === bot.id) : undefined
    if (bot && !current) {
      setFeedback({ kind: 'error', text: '目标 Bot 不存在：可能已被删除。' })
      return
    }
    setDialog(current ? { kind: 'edit', id: current.id } : { kind: 'create' })
    setStep(1)
    setDraft(current ? draftFrom(current) : { ...emptyDraft(), agent: result.options.agents[0] })
    setFormError(null)
    setNotice(null)
  }

  function openCreate() { void openEditor() }
  function openEdit(bot: Bot) { void openEditor(bot) }

  function closeDialog() {
    setDialog(null)
    setFormError(null)
    setNotice(null)
  }

  function changeAgent(agent: BotInput['agent']) {
    // 切换 Agent 不继承模型或同名扩展；切回也不恢复隐藏选择。
    setDraft((current) => ({
      ...current,
      agent,
      model: 'default',
      thinking: 'default',
      extensions: [],
    }))
    setNotice(null)
  }

  function changeModel(model: BotInput['model']) {
    const supported =
      model === 'default' ? [] : (data?.options.thinking[draft.agent]?.[model] ?? [])
    const retained = draft.thinking === 'default' || supported.includes(draft.thinking)
    setDraft((current) => ({
      ...current,
      model,
      thinking: retained ? current.thinking : 'default',
    }))
    setNotice(
      retained
        ? null
        : `当前思考级别「${thinkingLabel(draft)}」不适用于 ${draft.agent} / ${model}，已重置为「沿用 Agent 默认值」。`,
    )
  }

  async function remove(bot: Bot) {
    setDeleting(bot.id)
    setFeedback(null)
    const result = await deleteBot(bot.id)
    setDeleting(null)
    if (result.kind === 'ok') {
      setData({ bots: result.bots, options: result.options })
      setListFailure(null)
      setFeedback({ kind: 'success', text: `已删除 Bot：${bot.name}` })
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
      setFormError('请填写 Bot 显示名。')
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
        ? await createBot(draft)
        : await updateBot(dialog.id, draft)
    setSaving(false)

    if (result.kind === 'ok') {
      setData({ bots: result.bots, options: result.options })
      setListFailure(null)
      setDialog(null)
      setFeedback({ kind: 'success', text: `已保存 Bot：${draft.name}` })
    } else {
      // 保存失败不改变列表，也不显示“已保存”。
      setFormError(describeFailure(result, '保存'))
      setFeedback(null)
    }
  }

  const modelUnavailable = data !== null &&
    !(draft.model in (data.options.thinking[draft.agent] ?? {}))

  const thinkingOptions: BotOptions['thinking'][string][string] =
    draft.model === 'default'
      ? []
      : (data?.options.thinking[draft.agent]?.[draft.model] ?? [])

  const thinkingUnavailable = draft.thinking !== 'default' && !thinkingOptions.includes(draft.thinking)
  const extensions = (data?.options.extensions ?? []).filter((entry) => entry.agent === draft.agent)
  const fixedExtensions = extensions.filter((entry) => entry.kind === 'fixed')
  const optionalExtensions = extensions.filter((entry) => entry.kind === 'optional')
  const unavailableExtensions = draft.extensions.filter((id) => !optionalExtensions.some((entry) => entry.id === id))
  const selectionErrors = [
    ...(modelUnavailable ? [`model「${draft.model}」不可用`] : []),
    ...(thinkingUnavailable ? [`thinking「${draft.thinking}」不可用`] : []),
    ...unavailableExtensions.map((id) => `扩展「${id}」不可用`),
  ]

  return (
    <section className="bots" data-testid="bots">
      <div className="bots__actions">
        <button
          type="button"
          className="zone-action zone-action--create"
          data-testid="bot-create-open"
          disabled={data === null}
          onClick={openCreate}
        >
          新增 Bot
        </button>
      </div>

      {feedback !== null && (
        <p
          className="zone-feedback"
          data-testid="bot-feedback"
          data-kind={feedback.kind}
          role={feedback.kind === 'error' ? 'alert' : 'status'}
        >
          {feedback.text}
        </p>
      )}

      {loading && data === null && (
        <p className="zones__loading" data-testid="bot-loading" role="status">
          正在读取 Bot……
        </p>
      )}

      {listFailure !== null && (
        <p className="zone-error" data-testid="bot-list-error" role="alert">
          {listFailure}
        </p>
      )}

      {data !== null &&
        (data.bots.length === 0 ? (
          <p className="zones__empty" data-testid="bot-empty">
            尚未保存任何 Bot。
          </p>
        ) : (
          <ul className="bot-list" data-testid="bot-list">
            {data.bots.map((bot) => (
              <li
                key={bot.id}
                className="bot-item"
                data-testid="bot-item"
                data-bot-id={bot.id}
              >
                <h2 className="bot-item__name">{bot.name}</h2>
                <dl className="bot-item__fields">
                  <div>
                    <dt>chat-name 示例</dt>
                    <dd>{renderChatNameExample(bot.chatName, bot.name, bot.agent)}</dd>
                  </div>
                  <div>
                    <dt>agent</dt>
                    <dd>{bot.agent}</dd>
                  </div>
                  <div>
                    <dt>model</dt>
                    <dd>{modelLabel(bot)}</dd>
                  </div>
                  <div>
                    <dt>thinking</dt>
                    <dd>{thinkingLabel(bot)}</dd>
                  </div>
                  <div>
                    <dt>Skill</dt>
                    <dd>{skillLabel(bot)}</dd>
                  </div>
                  <div>
                    <dt>固定扩展</dt>
                    <dd>{data.options.extensions?.filter((entry) => entry.agent === bot.agent && entry.kind === 'fixed').map((entry) => entry.label).join('、') || '无'}</dd>
                  </div>
                  <div>
                    <dt>可选扩展</dt>
                    <dd>{bot.extensions.map((id) => data.options.extensions?.find((entry) => entry.id === id)?.label ?? id).join('、') || '无'}</dd>
                  </div>
                </dl>
                <pre className="bot-item__prompt" data-testid="bot-prompt-view">
                  {bot.prompt}
                </pre>
                <div className="bot-item__actions">
                  <button
                    type="button"
                    className="zone-dialog__cancel"
                    data-testid="bot-edit"
                    disabled={saving || deleting !== null}
                    onClick={() => openEdit(bot)}
                  >
                    编辑
                  </button>
                  <button
                    type="button"
                    className="zone-dialog__cancel"
                    data-testid="bot-delete"
                    disabled={saving || deleting !== null}
                    onClick={() => void remove(bot)}
                  >
                    删除
                  </button>
                </div>
              </li>
            ))}
          </ul>
        ))}

      {dialog !== null && data !== null && (
        <div className="zone-modal">
          <form
            className="zone-dialog bot-dialog"
            data-testid="bot-dialog"
            role="dialog"
            aria-modal="true"
            aria-label={dialog.kind === 'create' ? '新增 Bot' : '编辑 Bot'}
            onSubmit={(event) => void submit(event)}
          >
            <h2 className="zone-dialog__title">
              {dialog.kind === 'create' ? '新增 Bot' : '编辑 Bot'}
            </h2>

            <p data-testid="bot-step" role="status">
              {step} / 3 · {step === 1 ? 'Bot 预设' : step === 2 ? '扩展配置' : '确认保存'}
            </p>
            <ol className="bot-steps" aria-label="Bot 编辑步骤">
              {['Bot 预设', '扩展配置', '确认保存'].map((label, index) => (
                <li key={label} aria-current={step === index + 1 ? 'step' : undefined}>{label}</li>
              ))}
            </ol>

            {selectionErrors.length > 0 && (
              <p className="zone-error" data-testid="bot-selection-error" role="alert">
                {selectionErrors.join('；')}；请显式修正选择后再保存或创建。仅查看或切换步骤不会修正已保存 Bot。
              </p>
            )}

            {step === 1 && <>
            <section className="bot-preview-card" aria-label="Chat 名称实时示例">
              <span className="bot-preview-card__label">Chat 名称 · 实时示例</span>
              <output
                className="bot-preview"
                data-testid="chat-name-preview"
                aria-live="polite"
              >
                {renderChatNameExample(draft.chatName, draft.name, draft.agent) || '（空规则）'}
              </output>
              <small className="bot-preview-card__hint">
                示例 zone：alpha；示例短标识：a7f3c2。发起 Chat 时由后端使用真实值。
              </small>
            </section>

            <div className="bot-compose">
              <div className="bot-compose__primary">
                <ChatNameRuleEditor
                  value={draft.chatName}
                  onChange={(chatName) => setDraft((current) => ({ ...current, chatName }))}
                  disabled={saving}
                />
              </div>

              <fieldset className="bot-settings">
                <legend>Agent 与 Bot 设置</legend>

                <label className="zone-form__label" htmlFor="bot-name">
                  Bot 显示名
                </label>
                <input
                  id="bot-name"
                  className="zone-form__input"
                  data-testid="bot-name"
                  type="text"
                  value={draft.name}
                  onChange={(event) =>
                    setDraft((current) => ({ ...current, name: event.target.value }))
                  }
                  autoComplete="off"
                  disabled={saving}
                />
                <p className="bot-hint">
                  必填；只是 Bot 的显示名，不是新 Chat 的固定名字。
                </p>

                <label className="zone-form__label" htmlFor="bot-agent">
                  agent
                </label>
                <select
                  id="bot-agent"
                  className="zone-form__input zone-form__select"
                  data-testid="bot-agent"
                  value={draft.agent}
                  onChange={(event) => changeAgent(event.target.value as BotInput['agent'])}
                  disabled={saving}
                >
                  {data.options.agents.map((agent) => (
                    <option key={agent} value={agent}>
                      {data.options.labels?.agents?.[agent] ?? agent}
                    </option>
                  ))}
                </select>

                <label className="zone-form__label" htmlFor="bot-model">
                  model
                </label>
                <select
                  id="bot-model"
                  className="zone-form__input zone-form__select"
                  data-testid="bot-model"
                  value={draft.model}
                  onChange={(event) => changeModel(event.target.value as BotInput['model'])}
                  disabled={saving}
                >
                  {modelUnavailable && <option value={draft.model} disabled>{draft.model}（不可用）</option>}
                  {data.options.models.filter((model) => model in (data.options.thinking[draft.agent] ?? {})).map((model) => (
                    <option key={model} value={model}>
                      {data.options.labels?.models?.[model] ?? (model === 'default' ? '沿用 Agent 默认值' : model)}
                    </option>
                  ))}
                </select>

                <label className="zone-form__label" htmlFor="bot-thinking">
                  thinking
                </label>
                <select
                  id="bot-thinking"
                  className="zone-form__input zone-form__select"
                  data-testid="bot-thinking"
                  value={draft.thinking}
                  onChange={(event) =>
                    setDraft((current) => ({
                      ...current,
                      thinking: event.target.value as BotInput['thinking'],
                    }))
                  }
                  disabled={saving}
                >
                  <option value="default">沿用 Agent 默认值</option>
                  {thinkingUnavailable && <option value={draft.thinking} disabled>{draft.thinking}（不可用）</option>}
                  {thinkingOptions.map((thinking) => (
                    <option key={thinking} value={thinking}>
                      {thinking}
                    </option>
                  ))}
                </select>

                {notice !== null && (
                  <p className="zone-feedback" data-testid="thinking-notice" role="status">
                    {notice}
                  </p>
                )}

                <label className="zone-form__label" htmlFor="bot-skill">
                  Skill
                </label>
                <select
                  id="bot-skill"
                  className="zone-form__input zone-form__select"
                  data-testid="bot-skill"
                  value={draft.skill ?? ''}
                  onChange={(event) =>
                    setDraft((current) => ({
                      ...current,
                      skill:
                        event.target.value === ''
                          ? null
                          : (event.target.value as NonNullable<BotInput['skill']>),
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

                <label className="zone-form__label" htmlFor="bot-prompt">
                  固定 prompt（可多行、可为空）
                </label>
                <textarea
                  id="bot-prompt"
                  className="zone-form__input bot-prompt"
                  data-testid="bot-prompt"
                  value={draft.prompt}
                  onChange={(event) =>
                    setDraft((current) => ({ ...current, prompt: event.target.value }))
                  }
                  rows={5}
                  disabled={saving}
                />
              </fieldset>
            </div>
            </>}

            {step === 2 && <section className="bot-extensions" aria-label="扩展配置">
              <section data-testid="bot-fixed-extensions">
                <h3>固定扩展（始终加载，不可取消）</h3>
                <p>{fixedExtensions.map((entry) => entry.label).join('、') || '无'}</p>
              </section>
              <fieldset disabled={saving}>
                <legend>可选扩展（可不选或多选）</legend>
                {optionalExtensions.map((entry) => (
                  <label key={entry.id} className="bot-extension">
                    <input type="checkbox" data-testid={`extension-${entry.id}`}
                      checked={draft.extensions.includes(entry.id)}
                      onChange={(event) => setDraft((current) => ({
                        ...current,
                        extensions: event.target.checked
                          ? [...current.extensions, entry.id]
                          : current.extensions.filter((id) => id !== entry.id),
                      }))}
                    />
                    {entry.label}
                  </label>
                ))}
                {unavailableExtensions.map((id) => (
                  <label key={id} className="bot-extension">
                    <input type="checkbox" data-testid={`extension-${id}`} checked
                      onChange={() => setDraft((current) => ({
                        ...current,
                        extensions: current.extensions.filter((selected) => selected !== id),
                      }))}
                    />
                    {id}（不可用）
                  </label>
                ))}
              </fieldset>
              <p>扩展自动发现：关闭。创建时仅加载对应 Agent 的固定项与本 Bot 所选项。</p>
            </section>}

            {step === 3 && <section data-testid="bot-summary" aria-label="确认保存摘要">
              <h3>确认保存</h3>
              <dl className="bot-item__fields">
                <div><dt>Bot 显示名</dt><dd data-testid="summary-name">{draft.name}</dd></div>
                <div><dt>chat-name 命名规则</dt><dd data-testid="summary-chatName">{draft.chatName}</dd></div>
                <div><dt>Chat 名称示例</dt><dd>{renderChatNameExample(draft.chatName, draft.name, draft.agent)}</dd></div>
                <div><dt>agent</dt><dd data-testid="summary-agent">{data.options.labels?.agents?.[draft.agent] ?? draft.agent}</dd></div>
                <div><dt>model</dt><dd data-testid="summary-model">{data.options.labels?.models?.[draft.model] ?? modelLabel(draft)}</dd></div>
                <div><dt>thinking</dt><dd data-testid="summary-thinking">{thinkingLabel(draft)}</dd></div>
                <div><dt>Skill</dt><dd data-testid="summary-skill">{draft.skill === null ? '无' : (data.options.labels?.skills?.[draft.skill] ?? skillLabel(draft))}</dd></div>
                <div><dt>固定扩展（始终加载，不可取消）</dt><dd data-testid="summary-fixed">{fixedExtensions.map((entry) => entry.label).join('、') || '无'}</dd></div>
                <div><dt>可选扩展</dt><dd data-testid="summary-optional">{draft.extensions.map((id) => extensions.find((entry) => entry.id === id)?.label ?? id).join('、') || '无'}</dd></div>
                <div><dt>扩展自动发现</dt><dd data-testid="summary-discovery">关闭</dd></div>
              </dl>
              <h4>固定 prompt</h4>
              <pre className="bot-item__prompt" data-testid="summary-prompt">{draft.prompt}</pre>
              <p>仅在点击保存并收到成功确认后更新共享 Bot；摘要不表示扩展已实际加载。</p>
            </section>}

            {formError !== null && (
              <p
                className="zone-feedback"
                data-testid="bot-form-error"
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
                data-testid="bot-cancel"
                disabled={saving}
                onClick={closeDialog}
              >
                取消
              </button>
              {step !== 1 && <button type="button" className="zone-dialog__cancel"
                data-testid="bot-back" disabled={saving}
                onClick={() => setStep(step === 3 ? 2 : 1)}>上一步</button>}
              {step === 3 ? <button
                type="submit"
                className="zone-register"
                data-testid="save-bot"
                disabled={saving}
              >
                {saving ? '正在保存……' : '保存'}
              </button> : <button type="submit" className="zone-register"
                data-testid="bot-next" disabled={saving}>下一步</button>}
            </div>
          </form>
        </div>
      )}
    </section>
  )
}
