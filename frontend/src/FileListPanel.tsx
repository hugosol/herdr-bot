import { useCallback, useEffect, useRef, useState, type FormEvent } from 'react'

import { WorkspaceBrowser } from './WorkspaceBrowser'
import {
  fetchWorkspaceOverview,
  type WorkspaceSummary,
} from './gitApi'
import { useWorkspaceWatch } from './useWorkspaceWatch'
import {
  createWorkspace,
  fetchWorkspaces,
  registerWorkspace,
  unregisterWorkspace,
  type WorkspaceCreation,
  type WorkspaceRegistration,
  type WorkspaceResult,
} from './workspaceApi'
import {
  describeWatchProblem,
  type WorkspaceWatchFeed,
} from './workspaceWatch'

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
export function FileListPanel({
  active,
  watchFeed,
}: {
  active: boolean
  watchFeed?: WorkspaceWatchFeed
}) {
  const watch = useWorkspaceWatch(watchFeed, active)
  const [workspaces, setWorkspaces] = useState<WorkspaceRegistration[] | null>(null)
  const [listFailure, setListFailure] = useState<string | null>(null)
  const [loading, setLoading] = useState(false)
  const [path, setPath] = useState('')
  const [name, setName] = useState('')
  const [dialog, setDialog] = useState<'register' | 'create' | null>(null)
  const [formError, setFormError] = useState<string | null>(null)
  const [creation, setCreation] = useState<WorkspaceCreation>({ available: false })
  const [pending, setPending] = useState<
    'register' | 'create' | `remove:${string}` | null
  >(null)
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
        setCreation(result.creation)
        setListFailure(null)
      } else {
        setWorkspaces(null)
        setCreation({ available: false })
        setListFailure(describeFailure(result, '列表读取'))
      }
    })
    void loadOverview()
    return () => {
      cancelled = true
    }
  }, [active, loadOverview])

  // 自动更新通知到达时只重读角标概览：事件已由后端按 workspace 合并，页面不再自行节流。
  const watchRevision = watch.workspaces.reduce(
    (sum, entry) => sum + entry.generation,
    0,
  )
  useEffect(() => {
    if (!active || openWorkspace !== null || watchRevision === 0) return
    void loadOverview()
  }, [active, openWorkspace, watchRevision, loadOverview])

  function openDialog(kind: 'register' | 'create') {
    setDialog(kind)
    setPath('')
    setName('')
    setFormError(null)
    setFeedback(null)
  }

  function closeDialog() {
    setDialog(null)
    setFormError(null)
    setPath('')
    setName('')
  }

  async function submitRegister(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    const candidate = path.trim()
    if (candidate === '') {
      setFormError('请填写服务器上的目录路径。')
      return
    }

    setPending('register')
    setFormError(null)
    const result = await registerWorkspace(candidate)
    setPending(null)
    if (result.kind === 'ok') {
      setWorkspaces(result.workspaces)
      setCreation(result.creation)
      setListFailure(null)
      setDialog(null)
      setPath('')
      setFeedback({ kind: 'success', text: `已登记：${candidate}` })
      void loadOverview()
    } else {
      setFormError(describeFailure(result, '登记'))
    }
  }

  async function submitCreate(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    const candidate = name.trim()
    if (candidate === '') {
      setFormError('请填写新 workspace 的名称。')
      return
    }

    const previousPaths = new Set((workspaces ?? []).map((entry) => entry.path))
    setPending('create')
    setFormError(null)
    const result = await createWorkspace(candidate)
    setPending(null)
    if (result.kind === 'ok') {
      const createdPath = result.workspaces.find(
        (entry) => !previousPaths.has(entry.path),
      )?.path
      setWorkspaces(result.workspaces)
      setCreation(result.creation)
      setListFailure(null)
      setDialog(null)
      setName('')
      setFeedback({
        kind: 'success',
        text: createdPath === undefined ? '已创建 workspace。' : `已创建：${createdPath}`,
      })
      void loadOverview()
    } else {
      setFormError(describeFailure(result, '新建'))
    }
  }

  async function remove(registration: WorkspaceRegistration) {
    setPending(`remove:${registration.path}`)
    setFeedback(null)
    const result = await unregisterWorkspace(registration.path)
    setPending(null)
    if (result.kind === 'ok') {
      setWorkspaces(result.workspaces)
      setCreation(result.creation)
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

  // 列表上的监听问题只针对当前登记项：取消登记后残留的旧健康状态不应继续困扰页面。
  const registeredPaths = new Set((workspaces ?? []).map((registration) => registration.path))
  const failedWatch =
    watch.workspaces.find(
      (entry) => entry.health === 'error' && registeredPaths.has(entry.workspace),
    ) ?? null
  const listProblem =
    workspaces === null || workspaces.length === 0
      ? null
      : describeWatchProblem(watch.connection, failedWatch, failedWatch?.workspace)

  if (openWorkspace !== null) {
    const watchEntry =
      watch.workspaces.find((entry) => entry.workspace === openWorkspace) ?? null
    return (
      <section className="workspaces" data-testid="file-list">
        <WorkspaceBrowser
          workspace={openWorkspace}
          active={active}
          watch={{
            entry: watchEntry,
            connection: watch.connection,
            generation: watchEntry?.generation ?? 0,
          }}
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
      <div className="workspace-actions">
        <button
          type="button"
          className="workspace-action workspace-action--register"
          data-testid="register-open"
          disabled={pending !== null}
          onClick={() => openDialog('register')}
        >
          登记
        </button>
        <button
          type="button"
          className="workspace-action workspace-action--create"
          data-testid="create-open"
          disabled={
            pending !== null || workspaces === null || !creation.available
          }
          onClick={() => openDialog('create')}
        >
          新增
        </button>
      </div>

      {workspaces !== null && !creation.available && (
        <p className="workspace-note" data-testid="create-unavailable" role="note">
          未配置 [workspaces] create_parent_directory；“新增”暂不可用。
        </p>
      )}

      {listProblem !== null && (
        <p
          className="watch-warning"
          data-testid={listProblem.kind === 'listener' ? 'watch-error' : 'watch-stale'}
          role="alert"
        >
          {listProblem.text}
        </p>
      )}

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
                {/* 整行就是浏览入口：路径与角标都在按钮内部，点击行内任一非取消区域都会进入 workspace。 */}
                <button
                  type="button"
                  className="workspace-item__open"
                  aria-label={`浏览 ${registration.path}`}
                  data-testid="open-workspace"
                  disabled={pending !== null}
                  onClick={() => setOpenWorkspace(registration.path)}
                >
                  <code className="workspace-item__path">{registration.path}</code>
                  {badgeFor(registration.path)}
                </button>
                <button
                  type="button"
                  className="workspace-item__remove"
                  aria-label={`取消登记 ${registration.path}`}
                  title={`取消登记 ${registration.path}`}
                  disabled={pending !== null}
                  onClick={() => void remove(registration)}
                >
                  ×
                </button>
              </li>
            ))}
          </ul>
        ))}

      {dialog === 'register' && (
        <div className="workspace-modal">
          <form
            className="workspace-dialog"
            data-testid="register-dialog"
            role="dialog"
            aria-modal="true"
            aria-label="登记 workspace"
            onSubmit={(event) => void submitRegister(event)}
          >
            <h2 className="workspace-dialog__title">登记已有目录</h2>
            <p className="workspace-dialog__hint">
              输入 omp-bot 服务所在电脑上的绝对目录路径。取消登记不会删除目录或文件。
            </p>
            <label className="workspace-form__label" htmlFor="workspace-path">
              服务器目录路径
            </label>
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
            {formError !== null && (
              <p
                className="workspace-feedback"
                data-testid="workspace-feedback"
                data-kind="error"
                role="alert"
              >
                {formError}
              </p>
            )}
            <div className="workspace-dialog__actions">
              <button
                type="button"
                className="workspace-dialog__cancel"
                data-testid="register-cancel"
                disabled={pending !== null}
                onClick={closeDialog}
              >
                取消
              </button>
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
        </div>
      )}

      {dialog === 'create' && (
        <div className="workspace-modal">
          <form
            className="workspace-dialog"
            data-testid="create-dialog"
            role="dialog"
            aria-modal="true"
            aria-label="新增 workspace"
            onSubmit={(event) => void submitCreate(event)}
          >
            <h2 className="workspace-dialog__title">新增 workspace</h2>
            <p className="workspace-dialog__hint">
              将在{' '}
              <code>{creation.parentDirectory ?? '配置的父目录'}</code>{' '}
              下创建直接子目录，执行 git init 后加入登记列表。
            </p>
            <label className="workspace-form__label" htmlFor="workspace-name">
              新 workspace 名称
            </label>
            <input
              id="workspace-name"
              className="workspace-form__input"
              data-testid="create-name"
              type="text"
              value={name}
              onChange={(event) => setName(event.target.value)}
              placeholder="new-project"
              autoComplete="off"
              spellCheck={false}
              disabled={pending !== null}
            />
            {formError !== null && (
              <p
                className="workspace-feedback"
                data-testid="workspace-feedback"
                data-kind="error"
                role="alert"
              >
                {formError}
              </p>
            )}
            <div className="workspace-dialog__actions">
              <button
                type="button"
                className="workspace-dialog__cancel"
                data-testid="create-cancel"
                disabled={pending !== null}
                onClick={closeDialog}
              >
                取消
              </button>
              <button
                type="submit"
                className="workspace-register"
                data-testid="create-workspace"
                disabled={pending !== null}
              >
                创建
              </button>
            </div>
          </form>
        </div>
      )}
    </section>
  )
}
