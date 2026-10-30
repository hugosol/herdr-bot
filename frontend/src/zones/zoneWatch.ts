/**
 * 文件列表自动更新的真实数据源：后端只读 SSE 长连接。
 *
 * 服务端为每个 zone 合并本地文件事件，按最小间隔下发 `refresh`；`health` 明确报告监听失效。
 * 页面只消费「可以重新读取」的通知与健康状态，不把本地通知当成远端状态，也不因它自动 fetch。
 *
 * 首帧 `snapshot` 是整份监听视图（实际间隔、每个 zone 的健康与最近刷新）。连接错误只降级为
 * 「可能已过期」，恢复实况的唯一证据是又一份完整 `snapshot`；`refresh`/`health` 增量不构成恢复证据。
 */

import type { ConnectionStatus } from '../chats/sessionFeed'

export type ZoneWatchHealth = 'live' | 'error'

/** 后端下发的单个 zone 监听视图。 */
export type ZoneWatchEntry = {
  zone: string
  health: ZoneWatchHealth
  /** 监听失败原因；正常监听时缺省。 */
  message?: string
  /** 最近一次自动刷新通知的服务端 Unix 毫秒时间；尚未刷新时缺省。 */
  lastRefreshAt?: number
  /** 服务端累计的刷新代数；同一 zone 的观察者看到同一个值。 */
  generation: number
}

/** 数据源当前暴露的一帧监听视图。 */
export type ZoneWatchSnapshot = {
  connection: ConnectionStatus
  intervalSeconds: number
  zones: ZoneWatchEntry[]
}

/**
 * 自动更新数据源。形状与 `useSyncExternalStore` 对齐；`getSnapshot` 在数据未变化时必须返回同一引用。
 */
export type ZoneWatchFeed = {
  subscribe(onChange: () => void): () => void
  getSnapshot(): ZoneWatchSnapshot
}

/** 尚未接线时的空视图：没有监听项，也不产生任何自动刷新。 */
const EMPTY_SNAPSHOT: ZoneWatchSnapshot = {
  connection: 'live',
  intervalSeconds: 10,
  zones: [],
}

export const EMPTY_ZONE_WATCH: ZoneWatchFeed = {
  subscribe: () => () => {},
  getSnapshot: () => EMPTY_SNAPSHOT,
}

/** 默认端点：与页面同源，开发时由 Vite 代理转发到本机后端。 */
export const ZONE_WATCH_URL = '/api/zones/watch'

/** 本模块用到的 `EventSource` 子集：真实实现与测试假实现都能满足它。 */
export type ZoneWatchSource = {
  addEventListener(type: string, listener: (event: { data: string }) => void): void
  close(): void
}

export type ZoneWatchSourceFactory = (url: string) => ZoneWatchSource

const defaultSourceFactory: ZoneWatchSourceFactory = (url) => new EventSource(url)

function parsePayload(data: string): Record<string, unknown> | null {
  try {
    const parsed: unknown = JSON.parse(data)
    return typeof parsed === 'object' && parsed !== null
      ? (parsed as Record<string, unknown>)
      : null
  } catch {
    return null
  }
}

function isGeneration(value: unknown): value is number {
  return typeof value === 'number' && Number.isInteger(value) && value >= 0
}

function parseEntry(value: unknown): ZoneWatchEntry | null {
  if (typeof value !== 'object' || value === null) return null
  const record = value as Record<string, unknown>
  if (typeof record.zone !== 'string' || record.zone === '') return null
  if (record.health !== 'live' && record.health !== 'error') return null
  if (!isGeneration(record.generation)) return null
  if (record.message !== undefined && typeof record.message !== 'string') return null
  if (
    record.lastRefreshAt !== undefined &&
    (typeof record.lastRefreshAt !== 'number' || !Number.isFinite(record.lastRefreshAt))
  ) {
    return null
  }
  return {
    zone: record.zone,
    health: record.health,
    generation: record.generation,
    ...(record.message === undefined ? {} : { message: record.message }),
    ...(record.lastRefreshAt === undefined
      ? {}
      : { lastRefreshAt: record.lastRefreshAt }),
  }
}

function parseSnapshot(payload: Record<string, unknown>): ZoneWatchSnapshot | null {
  const interval = payload.intervalSeconds
  if (typeof interval !== 'number' || !Number.isInteger(interval) || interval < 1) return null
  if (!Array.isArray(payload.zones)) return null
  const zones: ZoneWatchEntry[] = []
  for (const item of payload.zones) {
    const entry = parseEntry(item)
    if (entry === null) return null
    zones.push(entry)
  }
  return { connection: 'live', intervalSeconds: interval, zones }
}

export type CreateZoneWatchOptions = {
  url?: string
  /** 连接的构造方式；缺省用真实 `EventSource`，测试注入假实现。 */
  eventSourceFactory?: ZoneWatchSourceFactory
}

/**
 * 建立真实数据源：第一个订阅者出现时连线，最后一个退订时关闭。
 *
 * 退订后重置为初始视图：重新进入文件列表时会重新订阅并等待新的完整快照，不把上一次连接的
 * 健康状态当成当前事实。
 */
export function createZoneWatchFeed(
  options: CreateZoneWatchOptions = {},
): ZoneWatchFeed {
  const url = options.url ?? ZONE_WATCH_URL
  const createSource = options.eventSourceFactory ?? defaultSourceFactory

  let snapshot: ZoneWatchSnapshot = EMPTY_SNAPSHOT
  let source: ZoneWatchSource | null = null
  const subscribers = new Set<() => void>()

  function publish(next: ZoneWatchSnapshot) {
    snapshot = next
    for (const notify of subscribers) notify()
  }

  function noteInterrupted() {
    if (snapshot.connection === 'interrupted') return
    publish({ ...snapshot, connection: 'interrupted' })
  }

  function upsertEntry(entry: ZoneWatchEntry) {
    const index = snapshot.zones.findIndex(
      (known) => known.zone === entry.zone,
    )
    const zones =
      index === -1
        ? [...snapshot.zones, entry]
        : snapshot.zones.map((known, position) =>
            position === index ? entry : known,
          )
    publish({ ...snapshot, zones })
  }

  function handleFrame(name: string, data: string) {
    const payload = parsePayload(data)
    if (payload === null) return

    if (name === 'snapshot') {
      const next = parseSnapshot(payload)
      if (next === null) return
      publish(next)
      return
    }

    // 断线期间的增量不是恢复证据：手上市价可能已经过期，等完整快照再收敛。
    if (snapshot.connection === 'interrupted') return

    if (name === 'refresh') {
      const zone = payload.zone
      const at = payload.at
      const generation = payload.generation
      if (typeof zone !== 'string' || zone === '') return
      if (typeof at !== 'number' || !Number.isFinite(at) || !isGeneration(generation)) return
      const existing = snapshot.zones.find((entry) => entry.zone === zone)
      upsertEntry({
        zone,
        health: existing?.health ?? 'live',
        generation,
        lastRefreshAt: at,
        ...(existing?.message === undefined ? {} : { message: existing.message }),
      })
      return
    }

    if (name === 'health') {
      const zone = payload.zone
      if (typeof zone !== 'string' || zone === '') return
      if (payload.health !== 'live' && payload.health !== 'error') return
      if (!isGeneration(payload.generation)) return
      if (payload.message !== undefined && typeof payload.message !== 'string') return
      const existing = snapshot.zones.find((entry) => entry.zone === zone)
      upsertEntry({
        zone,
        health: payload.health,
        generation: payload.generation,
        ...(payload.health === 'error' && typeof payload.message === 'string'
          ? { message: payload.message }
          : {}),
        ...(existing?.lastRefreshAt === undefined
          ? {}
          : { lastRefreshAt: existing.lastRefreshAt }),
      })
    }
  }

  function connect() {
    const opened = createSource(url)
    source = opened
    opened.addEventListener('snapshot', (event) => handleFrame('snapshot', event.data))
    opened.addEventListener('refresh', (event) => handleFrame('refresh', event.data))
    opened.addEventListener('health', (event) => handleFrame('health', event.data))
    // 一次连接错误即降级；浏览器自行重连，恢复只由完整 snapshot 首帧完成。
    opened.addEventListener('error', () => noteInterrupted())
  }

  return {
    subscribe(onChange) {
      subscribers.add(onChange)
      if (subscribers.size === 1) connect()
      return () => {
        subscribers.delete(onChange)
        if (subscribers.size > 0) return
        source?.close()
        source = null
        snapshot = EMPTY_SNAPSHOT
      }
    },
    getSnapshot: () => snapshot,
  }
}

/** 页面上的自动更新问题：监听失效优先于连接中断；两者都不表示手动读取不可用。 */
export type ZoneWatchProblem = {
  kind: 'listener' | 'connection'
  text: string
}

export function describeWatchProblem(
  connection: ConnectionStatus,
  entry: ZoneWatchEntry | null,
  subject?: string,
): ZoneWatchProblem | null {
  if (connection === 'interrupted') {
    return {
      kind: 'connection',
      text: '自动更新连接已中断，当前内容可能已过期；恢复连接后会自动对齐，仍可手动刷新读取当前事实。',
    }
  }
  if (entry?.health === 'error') {
    const prefix = subject === undefined ? '' : `${subject}：`
    return {
      kind: 'listener',
      text: `${prefix}自动更新已停止：${entry.message ?? '无法继续监听文件变化。'} 当前内容可能已过期；仍可使用“刷新”手动读取最新状态。`,
    }
  }
  return null
}
