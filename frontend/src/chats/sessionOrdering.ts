import type { SessionView } from './sessionFeed'

/** 展示 tier；数字即展示次序，「等待用户输入」置顶、离线沉底。 */
const TIER_ORDER = { waiting: 0, working: 1, idle: 2, unknown: 3, offline: 4 } as const

type SessionTier = keyof typeof TIER_ORDER

/**
 * 会话所属的展示 tier。只依赖 `online`/`state`：离线（含原等待者）一律沉底，
 * 不改写 agent 状态；`state` 缺省即「状态未知」，排在空闲之后。
 */
function tierOf(session: SessionView): SessionTier {
  if (!session.online) return 'offline'
  if (session.state === 'awaiting-input') return 'waiting'
  if (session.state === 'working') return 'working'
  if (session.state === 'idle') return 'idle'
  return 'unknown'
}

/** 是否属于等待组：在线的「等待用户输入」会话；离线的前等待者不算。 */
export function isAwaitingOnline(session: SessionView): boolean {
  return tierOf(session) === 'waiting'
}

/**
 * 前端最终排序口径：在线等待用户输入 → 在线工作中 → 空闲 → 状态未知 → 离线（末位）。
 * `sort` 是稳定排序，同 tier 之间因此保持数据源给出的注册顺序（服务端按注册顺序下发）。
 */
export function sortByTier(sessions: SessionView[]): SessionView[] {
  return [...sessions].sort((left, right) => TIER_ORDER[tierOf(left)] - TIER_ORDER[tierOf(right)])
}
