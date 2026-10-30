import type { SyncFacts } from './gitApi'

/** 本地已知 upstream 引用的领先提交数；不代表已经检查远端或工作树干净。 */
export function GitAheadIndicator({ sync }: { sync?: SyncFacts }) {
  if (sync?.upstream == null || sync.ahead == null || sync.ahead <= 0) return null
  const description = `有 ${sync.ahead} 个本地提交尚未推送（基于本地已知 upstream）`
  return (
    <span
      className="git-ahead"
      data-testid="git-ahead"
      role="img"
      aria-label={description}
      title={description}
    >
      ↑ {sync.ahead}
    </span>
  )
}
