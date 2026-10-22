import { useEffect, useState, type FormEvent } from 'react'

import { BotNameRuleEditor } from './BotNameRuleEditor'
import {
  createBotTemplate,
  deleteBotTemplate,
  fetchBotTemplates,
  updateBotTemplate,
  type BotTemplate,
  type BotTemplateInput,
  type BotTemplateList,
  type BotTemplateOptions,
  type BotTemplateResult,
} from './botTemplateApi'
import { INITIAL_BOT_NAME, renderBotNameExample } from './botTemplateNaming'

type Feedback = {
  kind: 'success' | 'error'
  text: string
}

type Dialog = { kind: 'create' } | { kind: 'edit'; id: string }

/** 把一次失败归一化成面向使用者的说明；未得到可用响应时明确说“无法确认”。 */
function describeFailure(result: BotTemplateResult, action: string): string {
  return result.kind === 'rejected'
    ? result.message
    : `无法确认模板${action}结果：请求没有得到可用响应。`
}

/** 模板的展示值：默认值不显示成具体模型或级别。 */
function modelLabel(template: Pick<BotTemplate, 'model'>): string {
  return template.model === 'default' ? '沿用 Agent 默认值' : template.model
}

function thinkingLabel(template: Pick<BotTemplate, 'thinking'>): string {
  return template.thinking === 'default' ? '沿用 Agent 默认值' : template.thinking
}

function skillLabel(template: Pick<BotTemplate, 'skill'>): string {
  return template.skill === null ? '无' : `/${template.skill}`
}

/** 新增模板的初始表单：agent 必须有固定选择，其余使用默认值。 */
function emptyDraft(): BotTemplateInput {
  return {
    name: '',
    botName: INITIAL_BOT_NAME,
    agent: 'omp',
    model: 'default',
    thinking: 'default',
    skill: null,
    prompt: '',
    extensions: [],
  }
}

/** 编辑时把已保存值放回表单。 */
function draftFrom(template: BotTemplate): BotTemplateInput {
  return {
    name: template.name,
    botName: template.botName,
    agent: template.agent,
    model: template.model,
    thinking: template.thinking,
    skill: template.skill,
    prompt: template.prompt,
    extensions: [...template.extensions],
  }
}

/**
 * Bot 模板：`bot` 入口中的新增、查看、编辑与删除。
 *
 * 模板由后端持久保存，电脑与手机读取同一份。页面不缓存列表：每次进入 bot 入口都重新读取，
 * 保存/删除成功后才用后端返回的完整列表更新界面；保存失败不表示已保存。
 */
export function BotTemplatesPanel({ active }: { active: boolean }) {
  const [data, setData] = useState<BotTemplateList | null>(null)
  const [loading, setLoading] = useState(false)
  const [listFailure, setListFailure] = useState<string | null>(null)
  const [feedback, setFeedback] = useState<Feedback | null>(null)
  const [dialog, setDialog] = useState<Dialog | null>(null)
  const [draft, setDraft] = useState<BotTemplateInput>(emptyDraft)
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
    void fetchBotTemplates().then((result) => {
      if (cancelled) return
      setLoading(false)
      if (result.kind === 'ok') {
        setData({ templates: result.templates, options: result.options })
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

  async function openEditor(template?: BotTemplate) {
    setLoading(true)
    const result = await fetchBotTemplates()
    setLoading(false)
    if (result.kind !== 'ok') {
      setFeedback({ kind: 'error', text: describeFailure(result, '选项读取') })
      return
    }
    setData({ templates: result.templates, options: result.options })
    const current = template ? result.templates.find((entry) => entry.id === template.id) : undefined
    if (template && !current) {
      setFeedback({ kind: 'error', text: '目标模板不存在：可能已被删除。' })
      return
    }
    setDialog(current ? { kind: 'edit', id: current.id } : { kind: 'create' })
    setStep(1)
    setDraft(current ? draftFrom(current) : { ...emptyDraft(), agent: result.options.agents[0] })
    setFormError(null)
    setNotice(null)
  }

  function openCreate() { void openEditor() }
  function openEdit(template: BotTemplate) { void openEditor(template) }

  function closeDialog() {
    setDialog(null)
    setFormError(null)
    setNotice(null)
  }

  function changeAgent(agent: BotTemplateInput['agent']) {
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

  function changeModel(model: BotTemplateInput['model']) {
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

  async function remove(template: BotTemplate) {
    setDeleting(template.id)
    setFeedback(null)
    const result = await deleteBotTemplate(template.id)
    setDeleting(null)
    if (result.kind === 'ok') {
      setData({ templates: result.templates, options: result.options })
      setListFailure(null)
      setFeedback({ kind: 'success', text: `已删除模板：${template.name}` })
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
      setFormError('请填写模板显示名。')
      return
    }
    if (draft.botName.trim() === '') {
      setFormError('请填写 bot-name 命名规则。')
      return
    }
    if (/[\r\n]/.test(draft.botName)) {
      setFormError('bot-name 必须是单行规则：固定文字与占位符不能包含换行。')
      return
    }

    setSaving(true)
    setFormError(null)
    const result =
      dialog.kind === 'create'
        ? await createBotTemplate(draft)
        : await updateBotTemplate(dialog.id, draft)
    setSaving(false)

    if (result.kind === 'ok') {
      setData({ templates: result.templates, options: result.options })
      setListFailure(null)
      setDialog(null)
      setFeedback({ kind: 'success', text: `已保存模板：${draft.name}` })
    } else {
      // 保存失败不改变列表，也不显示“已保存”。
      setFormError(describeFailure(result, '保存'))
      setFeedback(null)
    }
  }

  const modelUnavailable = data !== null &&
    !(draft.model in (data.options.thinking[draft.agent] ?? {}))

  const thinkingOptions: BotTemplateOptions['thinking'][string][string] =
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
    <section className="bot-templates" data-testid="bot-templates">
      <div className="bot-templates__actions">
        <button
          type="button"
          className="zone-action zone-action--create"
          data-testid="template-create-open"
          disabled={data === null}
          onClick={openCreate}
        >
          新增模板
        </button>
      </div>

      {feedback !== null && (
        <p
          className="zone-feedback"
          data-testid="template-feedback"
          data-kind={feedback.kind}
          role={feedback.kind === 'error' ? 'alert' : 'status'}
        >
          {feedback.text}
        </p>
      )}

      {loading && data === null && (
        <p className="zones__loading" data-testid="template-loading" role="status">
          正在读取 Bot 模板……
        </p>
      )}

      {listFailure !== null && (
        <p className="zone-error" data-testid="template-list-error" role="alert">
          {listFailure}
        </p>
      )}

      {data !== null &&
        (data.templates.length === 0 ? (
          <p className="zones__empty" data-testid="template-empty">
            尚未保存任何 Bot 模板。
          </p>
        ) : (
          <ul className="bot-template-list" data-testid="bot-template-list">
            {data.templates.map((template) => (
              <li
                key={template.id}
                className="bot-template-item"
                data-testid="bot-template-item"
                data-template-id={template.id}
              >
                <h2 className="bot-template-item__name">{template.name}</h2>
                <dl className="bot-template-item__fields">
                  <div>
                    <dt>bot-name 示例</dt>
                    <dd>{renderBotNameExample(template.botName, template.name, template.agent)}</dd>
                  </div>
                  <div>
                    <dt>agent</dt>
                    <dd>{template.agent}</dd>
                  </div>
                  <div>
                    <dt>model</dt>
                    <dd>{modelLabel(template)}</dd>
                  </div>
                  <div>
                    <dt>thinking</dt>
                    <dd>{thinkingLabel(template)}</dd>
                  </div>
                  <div>
                    <dt>Skill</dt>
                    <dd>{skillLabel(template)}</dd>
                  </div>
                  <div>
                    <dt>固定扩展</dt>
                    <dd>{data.options.extensions?.filter((entry) => entry.agent === template.agent && entry.kind === 'fixed').map((entry) => entry.label).join('、') || '无'}</dd>
                  </div>
                  <div>
                    <dt>可选扩展</dt>
                    <dd>{template.extensions.map((id) => data.options.extensions?.find((entry) => entry.id === id)?.label ?? id).join('、') || '无'}</dd>
                  </div>
                </dl>
                <pre className="bot-template-item__prompt" data-testid="template-prompt-view">
                  {template.prompt}
                </pre>
                <div className="bot-template-item__actions">
                  <button
                    type="button"
                    className="zone-dialog__cancel"
                    data-testid="template-edit"
                    disabled={saving || deleting !== null}
                    onClick={() => openEdit(template)}
                  >
                    编辑
                  </button>
                  <button
                    type="button"
                    className="zone-dialog__cancel"
                    data-testid="template-delete"
                    disabled={saving || deleting !== null}
                    onClick={() => void remove(template)}
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
            className="zone-dialog bot-template-dialog"
            data-testid="template-dialog"
            role="dialog"
            aria-modal="true"
            aria-label={dialog.kind === 'create' ? '新增 Bot 模板' : '编辑 Bot 模板'}
            onSubmit={(event) => void submit(event)}
          >
            <h2 className="zone-dialog__title">
              {dialog.kind === 'create' ? '新增 Bot 模板' : '编辑 Bot 模板'}
            </h2>

            <p data-testid="template-step" role="status">
              {step} / 3 · {step === 1 ? '模板预设' : step === 2 ? '扩展配置' : '确认保存'}
            </p>
            <ol className="bot-template-steps" aria-label="模板编辑步骤">
              {['模板预设', '扩展配置', '确认保存'].map((label, index) => (
                <li key={label} aria-current={step === index + 1 ? 'step' : undefined}>{label}</li>
              ))}
            </ol>

            {selectionErrors.length > 0 && (
              <p className="zone-error" data-testid="template-selection-error" role="alert">
                {selectionErrors.join('；')}；请显式修正选择后再保存或创建。仅查看或切换步骤不会修正已保存模板。
              </p>
            )}

            {step === 1 && <>
            <section className="bot-template-preview-card" aria-label="Bot 名称实时示例">
              <span className="bot-template-preview-card__label">Bot 名称 · 实时示例</span>
              <output
                className="bot-template-preview"
                data-testid="bot-name-preview"
                aria-live="polite"
              >
                {renderBotNameExample(draft.botName, draft.name, draft.agent) || '（空规则）'}
              </output>
              <small className="bot-template-preview-card__hint">
                示例 zone：alpha；示例短标识：a7f3c2。创建 Bot 时由后端使用真实值。
              </small>
            </section>

            <div className="bot-template-compose">
              <div className="bot-template-compose__primary">
                <BotNameRuleEditor
                  value={draft.botName}
                  onChange={(botName) => setDraft((current) => ({ ...current, botName }))}
                  disabled={saving}
                />
              </div>

              <fieldset className="bot-template-settings">
                <legend>Agent 与模板设置</legend>

                <label className="zone-form__label" htmlFor="template-name">
                  模板显示名
                </label>
                <input
                  id="template-name"
                  className="zone-form__input"
                  data-testid="template-name"
                  type="text"
                  value={draft.name}
                  onChange={(event) =>
                    setDraft((current) => ({ ...current, name: event.target.value }))
                  }
                  autoComplete="off"
                  disabled={saving}
                />
                <p className="bot-template-hint">
                  必填；只是模板的显示名，不是新 Bot 的固定名字。
                </p>

                <label className="zone-form__label" htmlFor="template-agent">
                  agent
                </label>
                <select
                  id="template-agent"
                  className="zone-form__input zone-form__select"
                  data-testid="template-agent"
                  value={draft.agent}
                  onChange={(event) => changeAgent(event.target.value as BotTemplateInput['agent'])}
                  disabled={saving}
                >
                  {data.options.agents.map((agent) => (
                    <option key={agent} value={agent}>
                      {data.options.labels?.agents?.[agent] ?? agent}
                    </option>
                  ))}
                </select>

                <label className="zone-form__label" htmlFor="template-model">
                  model
                </label>
                <select
                  id="template-model"
                  className="zone-form__input zone-form__select"
                  data-testid="template-model"
                  value={draft.model}
                  onChange={(event) => changeModel(event.target.value as BotTemplateInput['model'])}
                  disabled={saving}
                >
                  {modelUnavailable && <option value={draft.model} disabled>{draft.model}（不可用）</option>}
                  {data.options.models.filter((model) => model in (data.options.thinking[draft.agent] ?? {})).map((model) => (
                    <option key={model} value={model}>
                      {data.options.labels?.models?.[model] ?? (model === 'default' ? '沿用 Agent 默认值' : model)}
                    </option>
                  ))}
                </select>

                <label className="zone-form__label" htmlFor="template-thinking">
                  thinking
                </label>
                <select
                  id="template-thinking"
                  className="zone-form__input zone-form__select"
                  data-testid="template-thinking"
                  value={draft.thinking}
                  onChange={(event) =>
                    setDraft((current) => ({
                      ...current,
                      thinking: event.target.value as BotTemplateInput['thinking'],
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

                <label className="zone-form__label" htmlFor="template-skill">
                  Skill
                </label>
                <select
                  id="template-skill"
                  className="zone-form__input zone-form__select"
                  data-testid="template-skill"
                  value={draft.skill ?? ''}
                  onChange={(event) =>
                    setDraft((current) => ({
                      ...current,
                      skill:
                        event.target.value === ''
                          ? null
                          : (event.target.value as NonNullable<BotTemplateInput['skill']>),
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

                <label className="zone-form__label" htmlFor="template-prompt">
                  固定 prompt（可多行、可为空）
                </label>
                <textarea
                  id="template-prompt"
                  className="zone-form__input bot-template-prompt"
                  data-testid="template-prompt"
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

            {step === 2 && <section className="bot-template-extensions" aria-label="扩展配置">
              <section data-testid="template-fixed-extensions">
                <h3>固定扩展（始终加载，不可取消）</h3>
                <p>{fixedExtensions.map((entry) => entry.label).join('、') || '无'}</p>
              </section>
              <fieldset disabled={saving}>
                <legend>可选扩展（可不选或多选）</legend>
                {optionalExtensions.map((entry) => (
                  <label key={entry.id} className="bot-template-extension">
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
                  <label key={id} className="bot-template-extension">
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
              <p>扩展自动发现：关闭。创建时仅加载对应 Agent 的固定项与本模板所选项。</p>
            </section>}

            {step === 3 && <section data-testid="template-summary" aria-label="确认保存摘要">
              <h3>确认保存</h3>
              <dl className="bot-template-item__fields">
                <div><dt>模板显示名</dt><dd data-testid="summary-name">{draft.name}</dd></div>
                <div><dt>bot-name 命名规则</dt><dd data-testid="summary-botName">{draft.botName}</dd></div>
                <div><dt>Bot 名称示例</dt><dd>{renderBotNameExample(draft.botName, draft.name, draft.agent)}</dd></div>
                <div><dt>agent</dt><dd data-testid="summary-agent">{data.options.labels?.agents?.[draft.agent] ?? draft.agent}</dd></div>
                <div><dt>model</dt><dd data-testid="summary-model">{data.options.labels?.models?.[draft.model] ?? modelLabel(draft)}</dd></div>
                <div><dt>thinking</dt><dd data-testid="summary-thinking">{thinkingLabel(draft)}</dd></div>
                <div><dt>Skill</dt><dd data-testid="summary-skill">{draft.skill === null ? '无' : (data.options.labels?.skills?.[draft.skill] ?? skillLabel(draft))}</dd></div>
                <div><dt>固定扩展（始终加载，不可取消）</dt><dd data-testid="summary-fixed">{fixedExtensions.map((entry) => entry.label).join('、') || '无'}</dd></div>
                <div><dt>可选扩展</dt><dd data-testid="summary-optional">{draft.extensions.map((id) => extensions.find((entry) => entry.id === id)?.label ?? id).join('、') || '无'}</dd></div>
                <div><dt>扩展自动发现</dt><dd data-testid="summary-discovery">关闭</dd></div>
              </dl>
              <h4>固定 prompt</h4>
              <pre className="bot-template-item__prompt" data-testid="summary-prompt">{draft.prompt}</pre>
              <p>仅在点击保存并收到成功确认后更新共享模板；摘要不表示扩展已实际加载。</p>
            </section>}

            {formError !== null && (
              <p
                className="zone-feedback"
                data-testid="template-form-error"
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
                data-testid="template-cancel"
                disabled={saving}
                onClick={closeDialog}
              >
                取消
              </button>
              {step !== 1 && <button type="button" className="zone-dialog__cancel"
                data-testid="template-back" disabled={saving}
                onClick={() => setStep(step === 3 ? 2 : 1)}>上一步</button>}
              {step === 3 ? <button
                type="submit"
                className="zone-register"
                data-testid="save-template"
                disabled={saving}
              >
                {saving ? '正在保存……' : '保存'}
              </button> : <button type="submit" className="zone-register"
                data-testid="template-next" disabled={saving}>下一步</button>}
            </div>
          </form>
        </div>
      )}
    </section>
  )
}
