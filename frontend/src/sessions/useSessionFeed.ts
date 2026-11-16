import { useCallback, useSyncExternalStore } from 'react'

import type { SessionFeed, SessionFeedSnapshot } from './sessionFeed'

/**
 * 订阅会话数据源并返回当前快照。
 *
 * `useSyncExternalStore` 的接线只写在这里：「会话列表」的卡片流订阅的就是这一帧数据。
 */
export function useSessionFeedSnapshot(feed: SessionFeed): SessionFeedSnapshot {
  const subscribe = useCallback((onChange: () => void) => feed.subscribe(onChange), [feed])
  const getSnapshot = useCallback(() => feed.getSnapshot(), [feed])
  return useSyncExternalStore(subscribe, getSnapshot)
}
