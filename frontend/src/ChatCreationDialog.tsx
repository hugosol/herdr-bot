import { useEffect, useRef, useState, type FormEvent } from 'react'

import {
  createChat,
  fetchChatCreation,
  previewChat,
  type ChatCreation,
  type ChatPreview,
} from './chatCreationApi'
import { fetchBots, type Bot } from './botApi'
import { renderChatNameExample } from './chatNaming'

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
function failureOutcomeText(creation: ChatCreation): string {
  const failure = creation.failure
  if (failure === undefined) {
    return '本次创建没有完成；无法确认后续动作是否发生，不会自动重试。'
  }
  switch (failure.stage) {
    case 'workspace':
      return '本次没有新建 Tab，也没有启动 agent；不会自动重试或改换 Workspace。已保留表单，可直接再次创建，也可改选 Workspace 后创建。'
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
 * 从某个 zone 发起 Chat 的弹窗。
 *
 * 后端提供真实命名上下文；本次规则可在本地编辑预览，不修改 Bot。确认后页面轮询同一次创建：只有后端确认目标
 * 就绪且初始指示提交成功，才显示“创建成功，初始指示已提交”；成功与失败都留在弹窗里，由用户关闭，
 * 不自动切换会话列表。
 */
export function ChatCreationDialog({ zone, onClose }: { zone: string; onClose: () => void }) {
  const [bots, setBots] = useState<Bot[] | null>(null)
  const [botId, setBotId] = useState('')
  const [workspaceTarget, setWorkspaceTarget] = useState<'default' | 'current'>('current')
  const [input, setInput] = useState('')
  const [chatNameRule, setChatNameRule] = useState('')
  const [loadError, setLoadError] = useState<string | null>(null)
  const [preview, setPreview] = useState<ChatPreview | null>(null)
  const [previewing, setPreviewing] = useState(false)
  const [previewError, setPreviewError] = useState<string | null>(null)
  const [previewRevision, setPreviewRevision] = useState(0)
  const [submitting, setSubmitting] = useState(false)
  const [creation, setCreation] = useState<ChatCreation | null>(null)
  const [requestError, setRequestError] = useState<string | null>(null)
  const [observationError, setObservationError] = useState<string | null>(null)
  const previewSequence = useRef(0)

  // 弹窗打开时读取后端共享 Bot；没有 Bot 时明确说明，而不是让确认按钮假装可用。
  useEffect(() => {
    let cancelled = false
    void fetchBots().then((result) => {
      if (cancelled) return
      if (result.kind === 'ok') {
        setBots(result.bots)
        setBotId(result.bots[0]?.id ?? '')
        setLoadError(null)
      } else {
        setBots(null)
        setLoadError(
          result.kind === 'rejected'
            ? result.message
            : '无法读取 Bot：请求没有得到可用响应。',
        )
      }
    })
    return () => {
      cancelled = true
    }
  }, [])

  // Bot 切换或请求重试时读取真实上下文并重置本次规则；规则编辑不触发请求。
  useEffect(() => {
    if (botId === '') {
      setPreview(null)
      return
    }
    const sequence = previewSequence.current + 1
    previewSequence.current = sequence
    setPreviewing(true)
    setPreview(null)
    setChatNameRule('')
    setPreviewError(null)
    void previewChat(zone, botId).then((result) => {
      if (previewSequence.current !== sequence) return
      setPreviewing(false)
      if (result.kind === 'ok') {
        setPreview(result.value)
        setChatNameRule(result.value.chatNameRule)
      } else {
        setPreview(null)
        setPreviewError(
          result.kind === 'rejected'
            ? result.message
            : '无法确认本次 Chat 名称：请求没有得到可用响应。',
        )
      }
    })
  }, [zone, botId, previewRevision])

  // 已接受的创建由后端继续；页面只观察同一次创建，直到成功或失败。
  useEffect(() => {
    if (creation === null || creation.stage === 'succeeded' || creation.stage === 'failed') {
      return
    }
    let cancelled = false
    const timer = setTimeout(() => {
      void fetchChatCreation(creation.id).then((result) => {
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

  const name = preview === null ? '' : renderChatNameExample(
    chatNameRule,
    preview.botName,
    preview.agent,
    { zoneName: preview.zoneName, chatId: preview.chatId },
  )
  const validName = chatNameRule.trim() !== '' && !/[\r\n]/.test(chatNameRule) &&
    name.trim() !== '' && !/[\r\n]/.test(name)

  const workspaceFailed = creation?.stage === 'failed' && creation.failure?.stage === 'workspace'
  const running = creation !== null && creation.stage !== 'succeeded' && creation.stage !== 'failed'
  const finished = creation !== null && !running && !workspaceFailed

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    if (preview === null || previewing || !validName || submitting || (creation !== null && !workspaceFailed)) return

    setSubmitting(true)
    setRequestError(null)
    setObservationError(null)
    const result = await createChat({
      zone,
      botId: preview.botId,
      chatId: preview.chatId,
      expectedName: name,
      chatNameRule,
      input,
      workspaceTarget,
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
      // 预览可能已过期（Bot 被编辑）：丢弃旧名称，重新从后端取一次。
      setPreview(null)
      setPreviewRevision((revision) => revision + 1)
    }
  }

  const canSubmit =
    preview !== null && validName && !previewing && !submitting &&
    (creation === null || workspaceFailed) && bots !== null

  return (
    <div className="zone-modal">
      <form
        className="zone-dialog chat-dialog"
        data-testid="chat-creation-dialog"
        role="dialog"
        aria-modal="true"
        aria-label={`从 ${zone} 发起 Chat`}
        onSubmit={(event) => void submit(event)}
      >
        <h2 className="zone-dialog__title">发起 Chat</h2>
        <p className="zone-dialog__hint">
          新 Chat 会在所选 Herdr Workspace 中新建独立 Tab；默认使用服务端配置，当前按 zone 目录名匹配。
          无论选择哪项，都以所选 zone 的目录作为工作目录。
          创建前自动检查 Bot 的固定及所选扩展入口存在、为文件且可读；关闭扩展自动发现，选择仅来自 Bot。
          文件检查不执行扩展或探测模型、余额及账号，不代表扩展或 Skill 已初始化成功，也不保证检查后文件不再变化。
        </p>

        <p className="chat-zone" data-testid="chat-zone">
          zone：<code>{zone}</code>
        </p>

        <label className="zone-form__label" htmlFor="bot">
          Bot
        </label>
        <select
          id="bot"
          className="zone-form__input zone-form__select"
          data-testid="bot"
          value={botId}
          onChange={(event) => setBotId(event.target.value)}
          disabled={bots === null || submitting || running || finished}
        >
          {bots !== null && bots.length === 0 && <option value="">没有可用的 Bot</option>}
          {(bots ?? []).map((bot) => (
            <option key={bot.id} value={bot.id}>
              {bot.name}（{bot.agent}）
            </option>
          ))}
        </select>

        <label className="zone-form__label" htmlFor="chat-workspace-target">
          Herdr Workspace
        </label>
        <select
          id="chat-workspace-target"
          className="zone-form__input zone-form__select"
          value={workspaceTarget}
          onChange={(event) =>
            setWorkspaceTarget(event.target.value === 'current' ? 'current' : 'default')
          }
          disabled={submitting || running || finished}
          aria-describedby="chat-workspace-hint"
        >
          <option value="default">默认</option>
          <option value="current">当前</option>
        </select>
        <p id="chat-workspace-hint" className="zone-dialog__hint">
          当前按 zone 目录名匹配（区分大小写），不是 Herdr 当前聚焦的 Workspace，也不是浏览器所在子目录。
          未找到或匹配到多个 Workspace 时创建失败，不会改用默认 Workspace。
        </p>

        <label className="zone-form__label" htmlFor="chat-input">
          初始 input（可为空）
        </label>
        <textarea
          id="chat-input"
          className="zone-form__input chat-input"
          data-testid="chat-input"
          rows={4}
          value={input}
          onChange={(event) => setInput(event.target.value)}
          disabled={submitting || running || finished}
          spellCheck={false}
        />

        <label className="zone-form__label" htmlFor="chat-name-rule">本次 Chat 名称（可编辑规则）</label>
        <input
          id="chat-name-rule"
          className="zone-form__input"
          value={chatNameRule}
          onChange={(event) => setChatNameRule(event.target.value)}
          disabled={preview === null || previewing || submitting || running || finished}
          spellCheck={false}
          aria-describedby="chat-name-hint"
        />
        <p id="chat-name-hint" className="zone-dialog__hint">
          可修改固定文字，保留的占位符会自动转换；仅影响本次创建，不修改 Bot。
        </p>
        <span className="zone-form__label">最终名称预览</span>
        <output
          className="chat-name-preview"
          data-testid="chat-name-preview"
          aria-live="polite"
          data-state={previewing ? 'loading' : preview === null ? 'unavailable' : 'ready'}
        >
          {previewing
            ? '正在生成名称……'
            : preview !== null
              ? name || '请输入名称规则'
              : previewError !== null
                ? '名称暂不可用'
                : '选择 Bot 后显示'}
        </output>
        {preview !== null && !previewing && !validName && (
          <p className="zone-feedback" role="alert">名称必须是非空单行文字。</p>
        )}

        {loadError !== null && (
          <p className="zone-feedback" data-testid="chat-load-error" data-kind="error" role="alert">
            {loadError}
          </p>
        )}
        {previewError !== null && (
          <p className="zone-feedback" data-testid="chat-preview-error" data-kind="error" role="alert">
            {previewError}
          </p>
        )}
        {requestError !== null && (
          <p className="zone-feedback" data-testid="chat-request-error" data-kind="error" role="alert">
            {requestError}
          </p>
        )}
        {observationError !== null && (
          <p
            className="zone-feedback"
            data-testid="chat-observation-error"
            data-kind="error"
            role="alert"
          >
            {observationError}
          </p>
        )}

        {running && creation !== null && (
          <p className="chat-stage" data-testid="chat-create-stage" role="status">
            {creation.stage === 'starting'
              ? STAGE_LABELS.starting
              : STAGE_LABELS.submitting}
          </p>
        )}

        {creation !== null && (
          <dl className="chat-result" data-testid="chat-result">
            <div>
              <dt>Chat</dt>
              <dd data-testid="chat-result-name">{creation.name}</dd>
            </div>
            <div>
              <dt>zone</dt>
              <dd data-testid="chat-result-zone">{creation.zone}</dd>
            </div>
            {creation.tabId !== undefined && (
              <div>
                <dt>Herdr Tab</dt>
                <dd data-testid="chat-result-tab">{creation.tabId}</dd>
              </div>
            )}
            {creation.paneId !== undefined && (
              <div>
                <dt>Herdr Pane</dt>
                <dd data-testid="chat-result-pane">{creation.paneId}</dd>
              </div>
            )}
            {creation.agentName !== undefined && (
              <div>
                <dt>agent 控制别名</dt>
                <dd data-testid="chat-result-agent">{creation.agentName}</dd>
              </div>
            )}
          </dl>
        )}

        {creation?.stage === 'succeeded' && (
          <p
            className="zone-feedback"
            data-testid="chat-create-success"
            data-kind="success"
            role="status"
          >
            创建成功，初始指示已提交
          </p>
        )}

        {creation?.stage === 'failed' && (
          <div
            className="zone-feedback"
            data-testid="chat-create-failure"
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
            data-testid="chat-create-close"
            onClick={onClose}
          >
            关闭
          </button>
          <button
            type="submit"
            className="zone-register"
            data-testid="chat-create-submit"
            disabled={!canSubmit}
          >
            {submitting ? '正在提交……' : '创建'}
          </button>
        </div>
      </form>
    </div>
  )
}
