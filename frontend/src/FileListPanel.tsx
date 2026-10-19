import { useCallback, useEffect, useRef, useState, type FormEvent } from 'react'

import { WorkspaceBrowser } from './WorkspaceBrowser'
import {
  fetchWorkspaceOverview,
  type WorkspaceSummary,
} from './gitApi'
import {
  fetchWorkspaces,
  registerWorkspace,
  unregisterWorkspace,
  type WorkspaceRegistration,
  type WorkspaceResult,
} from './workspaceApi'

type Feedback = {
  kind: 'success' | 'error'
  text: string
}

/** 把一次失败归一化成面向使用者的说明；未得到可用响应时明确说“无法确认”。 */
function describeFailure(result: WorkspaceResult, action: string): string {
  return result.kind === 'rejected'
    ? result.message
    : `无法确认 workspace ${action}结果：请求没有得到可用响应。`
}

/**
 * 文件列表：workspace 登记列表的入口。
 *
 * 路径指 omp-bot 服务所在电脑上的目录，不是手机或浏览器所在设备；页面没有客户端文件选择器。
 * 每次进入这个顶层入口都重新读取后端列表，因此另一台设备新增的登记会在这里出现；登记成功
 * 只表示后端已经持久化，取消登记只移除登记项。
 */
export function FileListPanel({ active }: { active: boolean }) {
  const [workspaces, setWorkspaces] = useState<WorkspaceRegistration[] | null>(null)
  const [listFailure, setListFailure] = useState<string | null>(null)
  const [loading, setLoading] = useState(false)
  const [path, setPath] = useState('')
  const [pending, setPending] = useState<'register' | `remove:${string}` | null>(null)
  const [feedback, setFeedback] = useState<Feedback | null>(null)
  // 当前进入的 workspace；顶层 tab 往返期间保持，返回列表时才清除。
  const [openWorkspace, setOpenWorkspace] = useState<string | null>(null)
  // 每个 workspace 的待提交项数；只有后端确认过的事实才进入这里。
  const [summaries, setSummaries] = useState<Map<string, WorkspaceSummary> | null>(null)
  const [overviewFailure, setOverviewFailure] = useState<string | null>(null)
  const overviewSequence = useRef(0)

  const loadOverview = useCallback(async () => {
    const sequence = overviewSequence.current + 1
    overviewSequence.current = sequence
    const result = await fetchWorkspaceOverview()
    if (overviewSequence.current !== sequence) return
    if (result.kind === 'ok') {
      setSummaries(new Map(result.value.map((summary) => [summary.path, summary])))
      setOverviewFailure(null)
    } else {
      // 读取失败时清除旧角标：不能继续把上一次的计数显示成当前事实。
      setSummaries(null)
      setOverviewFailure(
        result.kind === 'rejected'
          ? result.message
          : '无法确认待提交数量：请求没有得到可用响应。',
      )
    }
  }, [])

  // 进入文件列表时读取后端共享列表；切走时取消尚未落地的读取结果。
  useEffect(() => {
    if (!active) return

    let cancelled = false
    setLoading(true)
    void fetchWorkspaces().then((result) => {
      if (cancelled) return
      setLoading(false)
      if (result.kind === 'ok') {
        setWorkspaces(result.workspaces)
        setListFailure(null)
      } else {
        setWorkspaces(null)
        setListFailure(describeFailure(result, '列表读取'))
      }
    })
    void loadOverview()
    return () => {
      cancelled = true
    }
  }, [active, loadOverview])

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    const candidate = path.trim()
    if (candidate === '') {
      setFeedback({ kind: 'error', text: '请填写服务器上的目录路径。' })
      return
    }

    setPending('register')
    setFeedback(null)
    const result = await registerWorkspace(candidate)
    setPending(null)
    if (result.kind === 'ok') {
      setWorkspaces(result.workspaces)
      setListFailure(null)
      setPath('')
      setFeedback({ kind: 'success', text: `已登记：${candidate}` })
      void loadOverview()
    } else {
      setFeedback({ kind: 'error', text: describeFailure(result, '登记') })
    }
  }

  async function remove(registration: WorkspaceRegistration) {
    setPending(`remove:${registration.path}`)
    setFeedback(null)
    const result = await unregisterWorkspace(registration.path)
    setPending(null)
    if (result.kind === 'ok') {
      setWorkspaces(result.workspaces)
      setListFailure(null)
      setFeedback({
        kind: 'success',
        text: `已取消登记：${registration.path}（目录与文件未删除）。`,
      })
      void loadOverview()
    } else {
      setFeedback({ kind: 'error', text: describeFailure(result, '取消登记') })
    }
  }

  function badgeFor(path: string) {
    const summary = summaries?.get(path)
    if (summary === undefined) return null
    if (summary.error !== undefined) {
      return (
        <span
          className="workspace-badge workspace-badge--error"
          data-testid="workspace-badge"
          title={summary.error}
        >
          待提交状态不可用
        </span>
      )
    }
    if (
      !summary.repository ||
      summary.pendingCount === undefined ||
      summary.pendingCount === 0
    ) {
      return null
    }
    return (
      <span
        className="workspace-badge"
        data-testid="workspace-badge"
        data-conflict={summary.conflict ? 'true' : undefined}
      >
        {summary.pendingCount} 项待提交
      </span>
    )
  }

  if (openWorkspace !== null) {
    return (
      <section className="workspaces" data-testid="file-list">
        <WorkspaceBrowser
          workspace={openWorkspace}
          active={active}
          onBack={() => {
            setOpenWorkspace(null)
            void loadOverview()
          }}
        />
      </section>
    )
  }

  return (
    <section className="workspaces" data-testid="file-list">
      <p className="scope-note">
        这里的路径指 omp-bot 服务所在电脑上的目录，不是手机或浏览器所在设备。
        登记成功后由后端保存，本机与局域网设备共享；取消登记不会删除目录或文件。
      </p>

      <form className="workspace-form" onSubmit={(event) => void submit(event)}>
        <label className="workspace-form__label" htmlFor="workspace-path">
          服务器目录路径
        </label>
        <div className="workspace-form__row">
          <input
            id="workspace-path"
            className="workspace-form__input"
            data-testid="workspace-path"
            type="text"
            value={path}
            onChange={(event) => setPath(event.target.value)}
            placeholder="C:\Projects\example"
            autoComplete="off"
            spellCheck={false}
            disabled={pending !== null}
          />
          <button
            type="submit"
            className="workspace-register"
            data-testid="register-workspace"
            disabled={pending !== null}
          >
            登记
          </button>
        </div>
      </form>

      {feedback !== null && (
        <p
          className="workspace-feedback"
          data-testid="workspace-feedback"
          data-kind={feedback.kind}
          role={feedback.kind === 'error' ? 'alert' : 'status'}
        >
          {feedback.text}
        </p>
      )}

      <h2 className="workspaces__heading">已登记的 workspace</h2>

      {loading && workspaces === null && (
        <p className="workspaces__loading" data-testid="workspace-loading" role="status">
          正在读取 workspace 登记列表……
        </p>
      )}

      {listFailure !== null && (
        <p className="workspace-error" data-testid="workspace-list-error" role="alert">
          {listFailure}
        </p>
      )}

      {overviewFailure !== null && workspaces !== null && workspaces.length > 0 && (
        <p className="workspace-error" data-testid="workspace-overview-error" role="alert">
          {overviewFailure}
        </p>
      )}

      {workspaces !== null &&
        (workspaces.length === 0 ? (
          <p className="workspaces__empty" data-testid="workspace-empty">
            尚未登记 workspace。
          </p>
        ) : (
          <ul className="workspace-list" data-testid="workspace-list">
            {workspaces.map((registration) => (
              <li key={registration.path} className="workspace-item" data-testid="workspace-item">
                <code className="workspace-item__path">{registration.path}</code>
                {badgeFor(registration.path)}
                <button
                  type="button"
                  className="workspace-item__open"
                  aria-label={`浏览 ${registration.path}`}
                  data-testid="open-workspace"
                  disabled={pending !== null}
                  onClick={() => setOpenWorkspace(registration.path)}
                >
                  浏览
                </button>
                <button
                  type="button"
                  className="workspace-item__remove"
                  aria-label={`取消登记 ${registration.path}`}
                  disabled={pending !== null}
                  onClick={() => void remove(registration)}
                >
                  取消登记
                </button>
              </li>
            ))}
          </ul>
        ))}
    </section>
  )
}
