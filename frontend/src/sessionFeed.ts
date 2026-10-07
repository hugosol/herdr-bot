/**
 * 会话列表与关机状态的数据源契约（接缝 S3）。
 *
 * 「会话列表」由**注入的数据源**驱动：组件只消费这里的接口，绝不直接接触全局
 * `EventSource`。这样组件测试可以用假数据源驱动 UI，真实 SSE 接线（`sseSessionFeed.ts`）
 * 只是这个契约的另一个实现，不必改动组件。
 *
 * 字段与 wire 视图一致：`state`/`sessionName`/`model` 缺省即「未知」或「隐藏」，
 * 不用哨兵值；`online` 是 keepAlive（在线/离线），与 agent 状态正交。
 *
 * 后端权威的关机状态（`shutdown`）走同一条观察流，因此也在这一份快照里：页面用它展示提醒与结果，
 * 不自己计时、也不自己决定到期执行。
 */

import { NO_SHUTDOWN, type ShutdownView } from './shutdownView'

/** keepAlive：会话是否仍在线；由服务端按心跳过期判定，与 agent 是否在跑是两回事。 */
export type KeepAlive = 'online' | 'offline'

/** agent 状态三值；缺省（首个 state 尚未到达）不在这个联合里，由 `state?` 表达未知。 */
export type AgentState = 'idle' | 'working' | 'awaiting-input'

/** 服务端下发的单个会话视图。 */
export type SessionView = {
  sessionId: string
  /** 会话名；缺省或空白时由界面回退为 `<cwd 目录名> · <sessionId 前 8 位>`。 */
  sessionName?: string
  cwd: string
  /** 模型展示名；缺省时界面隐藏该字段。 */
  model?: string
  /** keepAlive 取值：仍在收到心跳为 true。 */
  online: boolean
  /** agent 状态；缺省 = 尚未同步到状态，界面显示中性占位。 */
  state?: AgentState
}

/**
 * 订阅连接的健康状况。
 *
 * `interrupted` 表示推送流已断开、手上这份快照可能已经过期：会话列表保留最后一帧
 * 并在列表内标注「可能已经过期」；它同时是「轮次结束」判定的一条边界——断线与重连
 * 之间的状态变化只是快照收敛，不算前端观察到的迁移。
 */
export type ConnectionStatus = 'live' | 'interrupted'

/**
 * 数据源当前暴露的一帧：会话快照 + 连接状态 + 后端权威的关机状态。
 *
 * 关机状态与会话走**同一条**观察流（ADR 0005）：一条连接、首帧同时给出两者、关机变化以独立的命名事件
 * 携带完整状态。因此这里没有第二条连接或第二套读取约定。
 */
export type SessionFeedSnapshot = {
  sessions: SessionView[]
  connection: ConnectionStatus
  /** 后端权威的关机状态：调用者据此展示提醒、取消与结果，而不是自己计时。 */
  shutdown: ShutdownView
}

/**
 * 会话列表的数据源。形状与 `useSyncExternalStore` 对齐：`subscribe` 注册变更通知并
 * 返回退订函数，`getSnapshot` 返回当前快照。
 *
 * **`getSnapshot` 必须在数据未变化时返回同一引用**（缓存住快照对象）。每次调用都新造
 * 对象会被 React 判为无限循环并让组件抛错——真实浏览器里已实测到。数据变化时再换新对象。
 */
export type SessionFeed = {
  subscribe(onChange: () => void): () => void
  getSnapshot(): SessionFeedSnapshot
}

/** 尚未接线时的空快照：无会话、连接正常、还没有任何后端关机状态。 */
const EMPTY_SNAPSHOT: SessionFeedSnapshot = {
  sessions: [],
  connection: 'live',
  shutdown: NO_SHUTDOWN,
}

/**
 * 空数据源：组件在没人注入数据源时的默认值（页面显示引导空态）。
 * 运行时页面注入的是真实 SSE 数据源（`createSseSessionFeed`），它才是「打开即实况」的来源。
 */
export const EMPTY_SESSION_FEED: SessionFeed = {
  subscribe: () => () => {},
  getSnapshot: () => EMPTY_SNAPSHOT,
}
