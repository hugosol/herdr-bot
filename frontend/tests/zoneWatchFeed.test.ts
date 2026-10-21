import { beforeEach, describe, expect, it, vi } from 'vitest'

import { EMPTY_ZONE_WATCH, createZoneWatchFeed } from '../src/zoneWatch'
import { FakeZoneEventSource, resetFakeZoneEventSources } from './fakeZoneEventSource'

const ZONE = 'C:\\work\\project'

function snapshot(payload: Record<string, unknown> = {}) {
  return {
    intervalSeconds: 10,
    zones: [
      {
        zone: ZONE,
        health: 'live',
        generation: 0,
      },
    ],
    ...payload,
  }
}

/** 建立一条订阅并返回新开的假连接（页面的真实路径：订阅即连线）。 */
function subscribed() {
  const feed = createZoneWatchFeed({
    eventSourceFactory: (url) => new FakeZoneEventSource(url),
  })
  const onChange = vi.fn()
  const unsubscribe = feed.subscribe(onChange)
  const source = FakeZoneEventSource.instances.at(-1)!
  return { feed, source, onChange, unsubscribe }
}

beforeEach(() => {
  resetFakeZoneEventSources()
})

describe('zone 自动更新数据源接线', () => {
  it('订阅即建立同源长连接，snapshot 建立全量监听视图', () => {
    const { feed, source, onChange, unsubscribe } = subscribed()

    expect(source.url).toBe('/api/zones/watch')
    expect(feed.getSnapshot()).toEqual(EMPTY_ZONE_WATCH.getSnapshot())

    source.emit('snapshot', snapshot())
    expect(onChange).toHaveBeenCalled()
    const state = feed.getSnapshot()
    expect(state.connection).toBe('live')
    expect(state.intervalSeconds).toBe(10)
    expect(state.zones).toEqual([
      { zone: ZONE, health: 'live', generation: 0 },
    ])

    // 重连后的快照是整份重置，不保留断线前错过的旧健康状态。
    source.emit(
      'snapshot',
      snapshot({
        zones: [{ zone: ZONE, health: 'error', message: '监听失败', generation: 4 }],
      }),
    )
    expect(feed.getSnapshot().zones).toEqual([
      { zone: ZONE, health: 'error', message: '监听失败', generation: 4 },
    ])

    unsubscribe()
    expect(source.closed).toBe(true)
  })

  it('refresh 更新同一 zone 的最近刷新与代数，未知 zone 直接追加', () => {
    const { feed, source } = subscribed()
    source.emit('snapshot', snapshot())

    source.emit('refresh', { zone: ZONE, at: 123, generation: 1 })
    expect(feed.getSnapshot().zones).toEqual([
      { zone: ZONE, health: 'live', generation: 1, lastRefreshAt: 123 },
    ])

    source.emit('refresh', {
      zone: 'C:\\work\\other',
      at: 456,
      generation: 1,
    })
    expect(feed.getSnapshot().zones).toEqual([
      { zone: ZONE, health: 'live', generation: 1, lastRefreshAt: 123 },
      {
        zone: 'C:\\work\\other',
        health: 'live',
        generation: 1,
        lastRefreshAt: 456,
      },
    ])
  })

  it('health 明确报告监听失效，而不是把自动更新继续显示为正常', () => {
    const { feed, source } = subscribed()
    source.emit('snapshot', snapshot())

    source.emit('health', {
      zone: ZONE,
      health: 'error',
      message: 'zone 根目录已被替换，自动更新已停止。',
      generation: 2,
    })
    expect(feed.getSnapshot().zones).toEqual([
      {
        zone: ZONE,
        health: 'error',
        message: 'zone 根目录已被替换，自动更新已停止。',
        generation: 2,
      },
    ])

    // 后续 refresh 不改掉错误：监听失效后不能悄悄恢复成「正常」。
    source.emit('refresh', { zone: ZONE, at: 789, generation: 3 })
    const zone = feed.getSnapshot().zones[0]
    expect(zone.health).toBe('error')
    expect(zone.generation).toBe(3)
  })

  it('连接错误标记快照可能过期，只有完整 snapshot 才恢复实况', () => {
    const { feed, source } = subscribed()
    source.emit('snapshot', snapshot())

    source.emit('error', {})
    expect(feed.getSnapshot().connection).toBe('interrupted')
    expect(feed.getSnapshot().zones).toHaveLength(1)

    // 断线期间的增量不构成恢复证据。
    source.emit('refresh', { zone: ZONE, at: 999, generation: 9 })
    expect(feed.getSnapshot().connection).toBe('interrupted')
    expect(feed.getSnapshot().zones[0].generation).toBe(0)

    source.emit('snapshot', snapshot())
    expect(feed.getSnapshot().connection).toBe('live')
    expect(feed.getSnapshot().zones[0].generation).toBe(0)
  })

  it('读不懂的快照整份丢弃，不把半份监听视图当成当前事实', () => {
    const { feed, source } = subscribed()
    source.emit('snapshot', snapshot())

    source.emit('snapshot', { intervalSeconds: 10 })
    source.emit('snapshot', {
      intervalSeconds: 10,
      zones: [{ zone: ZONE, health: 'unknown' }],
    })
    source.emit('snapshot', {
      intervalSeconds: 0,
      zones: [{ zone: ZONE, health: 'live', generation: 0 }],
    })

    expect(feed.getSnapshot().zones[0]).toEqual({
      zone: ZONE,
      health: 'live',
      generation: 0,
    })
  })
})
