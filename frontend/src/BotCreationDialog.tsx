import { useEffect, useRef, useState, type FormEvent } from 'react'

import {
  createBot,
  fetchBotCreation,
  previewBot,
  type BotCreation,
  type BotPreview,
} from './botCreationApi'
import { fetchBotTemplates, type BotTemplate } from './botTemplateApi'

const STAGE_LABELS: Record<'starting' | 'submitting', string> = {
  starting: '正在启动',
  submitting: '正在提交初始指示',
}

const FAILURE_STAGE_LABELS: Record<string, string> = {
  workspace: '查找 Workspace',
  tab: '新建 Tab',
  start: '启动 agent',
  submit: '提交初始指示',
}

/**
 * 失败后的现场说明。
 *
 * 明确被拒绝与超时/无法确认是两种不同事实：只有前者能说提交失败；后者只能说明投递结果
 * 未确认，不能表述成确定未发送。已经取得的对象保留，页面不提供自动重发或清理。
 */
function failureOutcomeText(creation: BotCreation): string {
  const failure = creation.failure
  if (failure === undefined) {
    return '本次创建没有完成；无法确认后续动作是否发生，不会自动重试。'
  }
  switch (failure.stage) {
    case 'workspace':
      return '本次没有新建 Tab，也没有启动 agent；不会自动重试或改换 Workspace。'
    case 'tab':
      return '没有继续启动 agent；已经产生的对象如果存在会保留，不会自动删除或重建。'
    case 'start':
      return '现场已存在：Tab 与 Pane 已创建并保留，初始指示尚未提交；不会自动重发。'
    case 'submit':
      return failure.code === 'herdr-command-failed'
        ? '现场已存在：agent 已就绪，但初始指示的提交被明确拒绝；不会自动重发。'
        : '现场已存在：agent 已就绪；初始指示的投递结果未确认，不能断定它一定没有发送；不会自动重发。'
  }
  return '本次创建没有完成；不会自动重试。'
}

/**
 * 从某个 zone 创建 Bot 的弹窗。
 *
 * 名称只显示后端预览的只读结果，没有手工覆盖入口。确认后页面轮询同一次创建：只有后端确认目标
 * 就绪且初始指示提交成功，才显示“创建成功，初始指示已提交”；成功与失败都留在弹窗里，由用户关闭，
 * 不自动切换会话列表。
 */
export function BotCreationDialog({ zone, onClose }: { zone: string; onClose: () => void }) {
  const [templates, setTemplates] = useState<BotTemplate[] | null>(null)
  const [templateId, setTemplateId] = useState('')
  const [input, setInput] = useState('')
  const [loadError, setLoadError] = useState<string | null>(null)
  const [preview, setPreview] = useState<BotPreview | null>(null)
  const [previewing, setPreviewing] = useState(false)
  const [previewError, setPreviewError] = useState<string | null>(null)
  const [previewRevision, setPreviewRevision] = useState(0)
  const [submitting, setSubmitting] = useState(false)
  const [creation, setCreation] = useState<BotCreation | null>(null)
  const [requestError, setRequestError] = useState<string | null>(null)
  const [observationError, setObservationError] = useState<string | null>(null)
  const previewSequence = useRef(0)

  // 弹窗打开时读取后端共享模板；没有模板时明确说明，而不是让确认按钮假装可用。
  useEffect(() => {
    let cancelled = false
    void fetchBotTemplates().then((result) => {
      if (cancelled) return
      if (result.kind === 'ok') {
        setTemplates(result.templates)
        setTemplateId(result.templates[0]?.id ?? '')
        setLoadError(null)
      } else {
        setTemplates(null)
        setLoadError(
          result.kind === 'rejected'
            ? result.message
            : '无法读取 Bot 模板：请求没有得到可用响应。',
        )
      }
    })
    return () => {
      cancelled = true
    }
  }, [])

  // 模板选定后由后端生成名称与本次短实例标识；页面不自己拼名称。
  useEffect(() => {
    if (templateId === '') {
      setPreview(null)
      return
    }
    const sequence = previewSequence.current + 1
    previewSequence.current = sequence
    setPreviewing(true)
    setPreviewError(null)
    void previewBot(zone, templateId).then((result) => {
      if (previewSequence.current !== sequence) return
      setPreviewing(false)
      if (result.kind === 'ok') {
        setPreview(result.value)
      } else {
        setPreview(null)
        setPreviewError(
          result.kind === 'rejected'
            ? result.message
            : '无法确认本次 Bot 名称：请求没有得到可用响应。',
        )
      }
    })
  }, [zone, templateId, previewRevision])

  // 已接受的创建由后端继续；页面只观察同一次创建，直到成功或失败。
  useEffect(() => {
    if (creation === null || creation.stage === 'succeeded' || creation.stage === 'failed') {
      return
    }
    let cancelled = false
    const timer = setTimeout(() => {
      void fetchBotCreation(creation.id).then((result) => {
        if (cancelled) return
        if (result.kind === 'ok') {
          setCreation(result.value)
        } else {
          setObservationError(
            result.kind === 'rejected'
              ? result.message
              : '无法确认创建状态：请求没有得到可用响应。',
          )
        }
      })
    }, 250)
    return () => {
      cancelled = true
      clearTimeout(timer)
    }
  }, [creation])

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    if (preview === null || submitting || creation !== null) return

    setSubmitting(true)
    setRequestError(null)
    const result = await createBot({
      zone,
      templateId: preview.templateId,
      botId: preview.botId,
      expectedName: preview.name,
      input,
    })
    setSubmitting(false)
    if (result.kind === 'ok') {
      setCreation(result.value)
    } else {
      setRequestError(
        result.kind === 'rejected'
          ? result.message
          : '无法确认创建请求结果：请求没有得到可用响应。',
      )
      // 预览可能已过期（模板被编辑）：丢弃旧名称，重新从后端取一次。
      setPreview(null)
      setPreviewRevision((revision) => revision + 1)
    }
  }

  const running = creation !== null && creation.stage !== 'succeeded' && creation.stage !== 'failed'
  const finished = creation !== null && !running
  const canSubmit =
    preview !== null && !previewing && !submitting && !running && templates !== null

  return (
    <div className="zone-modal">
      <form
        className="zone-dialog bot-dialog"
        data-testid="bot-creation-dialog"
        role="dialog"
        aria-modal="true"
        aria-label={`从 ${zone} 创建 Bot`}
        onSubmit={(event) => void submit(event)}
      >
        <h2 className="zone-dialog__title">创建 Bot</h2>
        <p className="zone-dialog__hint">
          新 Bot 会在服务端配置的 Herdr Workspace 中新建独立 Tab，并以所选 zone 的目录作为工作目录。
        </p>

        <p className="bot-zone" data-testid="bot-zone">
          zone：<code>{zone}</code>
        </p>

        <label className="zone-form__label" htmlFor="bot-template">
          模板
        </label>
        <select
          id="bot-template"
          className="zone-form__input"
          data-testid="bot-template"
          value={templateId}
          onChange={(event) => setTemplateId(event.target.value)}
          disabled={templates === null || running || finished}
        >
          {templates !== null && templates.length === 0 && <option value="">没有可用的模板</option>}
          {(templates ?? []).map((template) => (
            <option key={template.id} value={template.id}>
              {template.name}（{template.agent}）
            </option>
          ))}
        </select>

        <label className="zone-form__label" htmlFor="bot-input">
          初始 input（可为空）
        </label>
        <textarea
          id="bot-input"
          className="zone-form__input bot-input"
          data-testid="bot-input"
          rows={4}
          value={input}
          onChange={(event) => setInput(event.target.value)}
          disabled={running || finished}
          spellCheck={false}
        />

        <span className="zone-form__label">本次 Bot 名称（只读预览）</span>
        <output
          className="bot-name-preview"
          data-testid="bot-name-preview"
          aria-live="polite"
          data-state={previewing ? 'loading' : preview === null ? 'unavailable' : 'ready'}
        >
          {previewing
            ? '正在生成名称……'
            : preview !== null
              ? preview.name
              : previewError !== null
                ? '名称暂不可用'
                : '选择模板后显示'}
        </output>
        <p className="zone-dialog__hint">
          名称由模板的 bot-name 规则与本次短实例标识决定，不能在弹窗中手工覆盖。
        </p>

        {loadError !== null && (
          <p className="zone-feedback" data-testid="bot-load-error" data-kind="error" role="alert">
            {loadError}
          </p>
        )}
        {previewError !== null && (
          <p className="zone-feedback" data-testid="bot-preview-error" data-kind="error" role="alert">
            {previewError}
          </p>
        )}
        {requestError !== null && (
          <p className="zone-feedback" data-testid="bot-request-error" data-kind="error" role="alert">
            {requestError}
          </p>
        )}
        {observationError !== null && (
          <p
            className="zone-feedback"
            data-testid="bot-observation-error"
            data-kind="error"
            role="alert"
          >
            {observationError}
          </p>
        )}

        {running && creation !== null && (
          <p className="bot-stage" data-testid="bot-create-stage" role="status">
            {creation.stage === 'starting'
              ? STAGE_LABELS.starting
              : STAGE_LABELS.submitting}
          </p>
        )}

        {creation !== null && (
          <dl className="bot-result" data-testid="bot-result">
            <div>
              <dt>Bot</dt>
              <dd data-testid="bot-result-name">{creation.name}</dd>
            </div>
            <div>
              <dt>zone</dt>
              <dd data-testid="bot-result-zone">{creation.zone}</dd>
            </div>
            {creation.tabId !== undefined && (
              <div>
                <dt>Herdr Tab</dt>
                <dd data-testid="bot-result-tab">{creation.tabId}</dd>
              </div>
            )}
            {creation.paneId !== undefined && (
              <div>
                <dt>Herdr Pane</dt>
                <dd data-testid="bot-result-pane">{creation.paneId}</dd>
              </div>
            )}
            {creation.agentName !== undefined && (
              <div>
                <dt>agent 控制别名</dt>
                <dd data-testid="bot-result-agent">{creation.agentName}</dd>
              </div>
            )}
          </dl>
        )}

        {creation?.stage === 'succeeded' && (
          <p
            className="zone-feedback"
            data-testid="bot-create-success"
            data-kind="success"
            role="status"
          >
            创建成功，初始指示已提交
          </p>
        )}

        {creation?.stage === 'failed' && (
          <div
            className="zone-feedback"
            data-testid="bot-create-failure"
            data-kind="error"
            role="alert"
          >
            <p>
              创建失败（{FAILURE_STAGE_LABELS[creation.failure?.stage ?? ''] ?? '未知阶段'}）：
              {creation.failure?.message ?? '后端没有给出可读说明。'}
            </p>
            <p>{failureOutcomeText(creation)}</p>
          </div>
        )}

        <div className="zone-dialog__actions">
          <button
            type="button"
            className="zone-dialog__cancel"
            data-testid="bot-create-close"
            onClick={onClose}
          >
            关闭
          </button>
          <button
            type="submit"
            className="zone-register"
            data-testid="bot-create-submit"
            disabled={!canSubmit}
          >
            {submitting ? '正在提交……' : '创建'}
          </button>
        </div>
      </form>
    </div>
  )
}
