import { useCallback, useEffect, useRef, useState, type FormEvent } from 'react'

import { ChatCreationDialog } from './ChatCreationDialog'
import { GitAheadIndicator } from './GitAheadIndicator'
import { ZoneBrowser } from './ZoneBrowser'
import {
  fetchZoneOverview,
  type ZoneSummary,
} from './gitApi'
import { useZoneWatch } from './useZoneWatch'
import {
  createZone,
  fetchZones,
  registerZone,
  unregisterZone,
  type ZoneCreation,
  type ZoneRegistration,
  type ZoneResult,
} from './zoneApi'
import { zoneDisplayName } from './zoneDisplay'
import {
  describeWatchProblem,
  type ZoneWatchFeed,
} from './zoneWatch'

type Feedback = {
  kind: 'success' | 'error'
  text: string
}

/** 把一次失败归一化成面向使用者的说明；未得到可用响应时明确说“无法确认”。 */
function describeFailure(result: ZoneResult, action: string): string {
  return result.kind === 'rejected'
    ? result.message
    : `无法确认 zone ${action}结果：请求没有得到可用响应。`
}

/**
 * zone 入口：zone 登记列表与文件浏览的入口。
 *
 * 路径指 herdr-bot 服务所在电脑上的目录，不是手机或浏览器所在设备；页面没有客户端文件选择器。
 * 每次进入这个顶层入口都重新读取后端列表，因此另一台设备新增的登记会在这里出现；登记成功
 * 只表示后端已经持久化，取消登记只移除登记项。
 */
export function FileListPanel({
  active,
  watchFeed,
}: {
  active: boolean
  watchFeed?: ZoneWatchFeed
}) {
  const watch = useZoneWatch(watchFeed, active)
  const [zones, setZones] = useState<ZoneRegistration[] | null>(null)
  const [listFailure, setListFailure] = useState<string | null>(null)
  const [loading, setLoading] = useState(false)
  const [path, setPath] = useState('')
  const [name, setName] = useState('')
  const [dialog, setDialog] = useState<'register' | 'create' | null>(null)
  // 从这一行的 zone 打开发起 Chat 弹窗；弹窗自身负责 Task、名称预览与创建阶段。
  const [chatZone, setChatZone] = useState<string | null>(null)
  const [formError, setFormError] = useState<string | null>(null)
  const [creation, setCreation] = useState<ZoneCreation>({ available: false })
  const [pending, setPending] = useState<
    'register' | 'create' | `remove:${string}` | null
  >(null)
  const [feedback, setFeedback] = useState<Feedback | null>(null)
  // 当前进入的 zone；顶层 tab 往返期间保持，返回列表时才清除。
  const [openZone, setOpenZone] = useState<string | null>(null)
  // 每个 zone 的待提交项数；只有后端确认过的事实才进入这里。
  const [summaries, setSummaries] = useState<Map<string, ZoneSummary> | null>(null)
  const [overviewFailure, setOverviewFailure] = useState<string | null>(null)
  const overviewSequence = useRef(0)

  const loadOverview = useCallback(async () => {
    const sequence = overviewSequence.current + 1
    overviewSequence.current = sequence
    const result = await fetchZoneOverview()
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

  // 进入 zone 入口时读取后端共享列表；切走时取消尚未落地的读取结果。
  useEffect(() => {
    if (!active) return

    let cancelled = false
    setLoading(true)
    void fetchZones().then((result) => {
      if (cancelled) return
      setLoading(false)
      if (result.kind === 'ok') {
        setZones(result.zones)
        setCreation(result.creation)
        setListFailure(null)
      } else {
        setZones(null)
        setCreation({ available: false })
        setListFailure(describeFailure(result, '列表读取'))
      }
    })
    void loadOverview()
    return () => {
      cancelled = true
    }
  }, [active, loadOverview])

  // 自动更新通知到达时只重读角标概览：事件已由后端按 zone 合并，页面不再自行节流。
  const watchRevision = watch.zones.reduce(
    (sum, entry) => sum + entry.generation,
    0,
  )
  useEffect(() => {
    if (!active || openZone !== null || watchRevision === 0) return
    void loadOverview()
  }, [active, openZone, watchRevision, loadOverview])

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
    const result = await registerZone(candidate)
    setPending(null)
    if (result.kind === 'ok') {
      setZones(result.zones)
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
      setFormError('请填写新 zone 的名称。')
      return
    }

    const previousPaths = new Set((zones ?? []).map((entry) => entry.path))
    setPending('create')
    setFormError(null)
    const result = await createZone(candidate)
    setPending(null)
    if (result.kind === 'ok') {
      const createdPath = result.zones.find(
        (entry) => !previousPaths.has(entry.path),
      )?.path
      setZones(result.zones)
      setCreation(result.creation)
      setListFailure(null)
      setDialog(null)
      setName('')
      setFeedback({
        kind: 'success',
        text: createdPath === undefined ? '已创建 zone。' : `已创建：${createdPath}`,
      })
      void loadOverview()
    } else {
      setFormError(describeFailure(result, '新建'))
    }
  }

  async function remove(registration: ZoneRegistration) {
    setPending(`remove:${registration.path}`)
    setFeedback(null)
    const result = await unregisterZone(registration.path)
    setPending(null)
    if (result.kind === 'ok') {
      setZones(result.zones)
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
          className="zone-badge zone-badge--error"
          data-testid="zone-badge"
          role="img"
          aria-label={`待提交状态不可用：${summary.error}`}
          title={summary.error}
        >
          ⚠
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
        className="zone-badge"
        data-testid="zone-badge"
        data-conflict={summary.conflict ? 'true' : undefined}
        aria-label={`${summary.pendingCount} 项待提交${summary.conflict ? '，存在冲突' : ''}`}
        title={`${summary.pendingCount} 项待提交${summary.conflict ? '，存在冲突' : ''}`}
      >
        {summary.pendingCount}
      </span>
    )
  }

  // 列表上的监听问题只针对当前登记项：取消登记后残留的旧健康状态不应继续困扰页面。
  const registeredPaths = new Set((zones ?? []).map((registration) => registration.path))
  const failedWatch =
    watch.zones.find(
      (entry) => entry.health === 'error' && registeredPaths.has(entry.zone),
    ) ?? null
  const listProblem =
    zones === null || zones.length === 0
      ? null
      : describeWatchProblem(watch.connection, failedWatch, failedWatch?.zone)

  if (openZone !== null) {
    const watchEntry =
      watch.zones.find((entry) => entry.zone === openZone) ?? null
    return (
      <section className="zones" data-testid="file-list">
        <ZoneBrowser
          zone={openZone}
          active={active}
          watch={{
            entry: watchEntry,
            connection: watch.connection,
            generation: watchEntry?.generation ?? 0,
          }}
          onBack={() => {
            setOpenZone(null)
            void loadOverview()
          }}
        />
      </section>
    )
  }

  return (
    <section className="zones" data-testid="file-list">
      <div className="zone-actions">
        <button
          type="button"
          className="zone-action zone-action--register"
          data-testid="register-open"
          disabled={pending !== null}
          onClick={() => openDialog('register')}
        >
          登记
        </button>
        <button
          type="button"
          className="zone-action zone-action--create"
          data-testid="create-open"
          disabled={
            pending !== null || zones === null || !creation.available
          }
          onClick={() => openDialog('create')}
        >
          新增
        </button>
      </div>

      {zones !== null && !creation.available && (
        <p className="zone-note" data-testid="create-unavailable" role="note">
          未配置 [zones] create_parent_directory；“新增”暂不可用。
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
          className="zone-feedback"
          data-testid="zone-feedback"
          data-kind={feedback.kind}
          role={feedback.kind === 'error' ? 'alert' : 'status'}
        >
          {feedback.text}
        </p>
      )}

      <h2 className="zones__heading">已登记的 zone</h2>

      {loading && zones === null && (
        <p className="zones__loading" data-testid="zone-loading" role="status">
          正在读取 zone 登记列表……
        </p>
      )}

      {listFailure !== null && (
        <p className="zone-error" data-testid="zone-list-error" role="alert">
          {listFailure}
        </p>
      )}

      {overviewFailure !== null && zones !== null && zones.length > 0 && (
        <p className="zone-error" data-testid="zone-overview-error" role="alert">
          {overviewFailure}
        </p>
      )}

      {zones !== null &&
        (zones.length === 0 ? (
          <p className="zones__empty" data-testid="zone-empty">
            尚未登记 zone。
          </p>
        ) : (
          <ul className="zone-list" data-testid="zone-list">
            {zones.map((registration) => (
              <li
                key={registration.path}
                className="zone-item"
                data-testid="zone-item"
                data-zone-path={registration.path}
              >
                {/* 浏览入口只显示末级目录名；完整路径留在无障碍名称、悬停标题与进入后的工具栏中。 */}
                <button
                  type="button"
                  className="zone-item__open"
                  aria-label={`浏览 ${registration.path}`}
                  title={registration.path}
                  data-testid="open-zone"
                  disabled={pending !== null}
                  onClick={() => setOpenZone(registration.path)}
                >
                  <span className="zone-item__name">{zoneDisplayName(registration.path)}</span>
                  {summaries?.get(registration.path)?.repository &&
                    summaries.get(registration.path)?.error === undefined && (
                      <GitAheadIndicator sync={summaries.get(registration.path)?.sync} />
                    )}
                  {badgeFor(registration.path)}
                </button>
                <button
                  type="button"
                  className="zone-item__chat"
                  data-testid="zone-create-chat"
                  aria-label={`从 ${registration.path} 发起 Chat`}
                  title={`从 ${registration.path} 发起 Chat`}
                  disabled={pending !== null}
                  onClick={() => setChatZone(registration.path)}
                >
                  +
                </button>
                <button
                  type="button"
                  className="zone-item__remove"
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
        <div className="zone-modal">
          <form
            className="zone-dialog"
            data-testid="register-dialog"
            role="dialog"
            aria-modal="true"
            aria-label="登记 zone"
            onSubmit={(event) => void submitRegister(event)}
          >
            <h2 className="zone-dialog__title">登记已有目录</h2>
            <p className="zone-dialog__hint">
              输入 herdr-bot 服务所在电脑上的绝对目录路径。取消登记不会删除目录或文件。
            </p>
            <label className="zone-form__label" htmlFor="zone-path">
              服务器目录路径
            </label>
            <input
              id="zone-path"
              className="zone-form__input"
              data-testid="zone-path"
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
                className="zone-feedback"
                data-testid="zone-feedback"
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
                data-testid="register-cancel"
                disabled={pending !== null}
                onClick={closeDialog}
              >
                取消
              </button>
              <button
                type="submit"
                className="zone-register"
                data-testid="register-zone"
                disabled={pending !== null}
              >
                登记
              </button>
            </div>
          </form>
        </div>
      )}

      {dialog === 'create' && (
        <div className="zone-modal">
          <form
            className="zone-dialog"
            data-testid="create-dialog"
            role="dialog"
            aria-modal="true"
            aria-label="新增 zone"
            onSubmit={(event) => void submitCreate(event)}
          >
            <h2 className="zone-dialog__title">新增 zone</h2>
            <p className="zone-dialog__hint">
              将在{' '}
              <code>{creation.parentDirectory ?? '配置的父目录'}</code>{' '}
              下创建直接子目录，执行 git init 后加入登记列表。
            </p>
            <label className="zone-form__label" htmlFor="zone-name">
              新 zone 名称
            </label>
            <input
              id="zone-name"
              className="zone-form__input"
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
                className="zone-feedback"
                data-testid="zone-feedback"
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
                data-testid="create-cancel"
                disabled={pending !== null}
                onClick={closeDialog}
              >
                取消
              </button>
              <button
                type="submit"
                className="zone-register"
                data-testid="create-zone"
                disabled={pending !== null}
              >
                创建
              </button>
            </div>
          </form>
        </div>
      )}

      {chatZone !== null && (
        <ChatCreationDialog zone={chatZone} onClose={() => setChatZone(null)} />
      )}
    </section>
  )
}
