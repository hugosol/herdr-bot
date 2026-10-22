import { useCallback, useSyncExternalStore } from 'react'

import {
  EMPTY_ZONE_WATCH,
  type ZoneWatchFeed,
  type ZoneWatchSnapshot,
} from './zoneWatch'

/**
 * 订阅自动更新数据源；只有 zone 入口处于前台时才连接。
 *
 * 切到其它顶层 tab 时退订并回到空视图：重新进入 zone 入口会重新连接并等待完整快照，
 * 不把上一次连接的健康状态当成当前事实；进入 zone 时的即时读取仍由页面自己发起。
 */
export function useZoneWatch(
  feed: ZoneWatchFeed | undefined,
  active: boolean,
): ZoneWatchSnapshot {
  const source = active && feed !== undefined ? feed : EMPTY_ZONE_WATCH
  const subscribe = useCallback(
    (onChange: () => void) => source.subscribe(onChange),
    [source],
  )
  const getSnapshot = useCallback(() => source.getSnapshot(), [source])
  return useSyncExternalStore(subscribe, getSnapshot)
}
