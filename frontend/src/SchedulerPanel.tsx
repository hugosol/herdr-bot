import { useEffect, useState } from 'react'
import { listMaps } from './schedulerApi'

type Listing = { kind: 'loading' } | { kind: 'ready'; files: string[] } | { kind: 'error'; message: string }

export function SchedulerPanel({ active }: { active: boolean }) {
  const [listing, setListing] = useState<Listing>({ kind: 'loading' })
  const [revision, setRevision] = useState(0)
  useEffect(() => {
    if (!active) return
    const controller = new AbortController()
    setListing({ kind: 'loading' })
    listMaps(controller.signal).then(
      (files) => { if (!controller.signal.aborted) setListing({ kind: 'ready', files }) },
      (error: unknown) => {
        if (!controller.signal.aborted) setListing({ kind: 'error', message: error instanceof Error ? error.message : 'Map 目录读取失败。' })
      },
    )
    return () => controller.abort()
  }, [active, revision])
  return <section aria-label="Map 目录">
    <h2>scheduler</h2>
    <p>服务主机上的 Map，与当前 zone 无关。仅列出目录直接包含的 JSON 文件。</p>
    <button type="button" onClick={() => setRevision((value) => value + 1)}>刷新 Map 列表</button>
    {listing.kind === 'loading' && <p role="status">正在读取 Map 目录……</p>}
    {listing.kind === 'error' && <p role="alert" data-testid="scheduler-error">{listing.message}</p>}
    {listing.kind === 'ready' && (listing.files.length === 0
      ? <p data-testid="scheduler-empty">此目录没有 Map JSON 文件。</p>
      : <ul data-testid="scheduler-map-list">{listing.files.map((name) => <li key={name}>{name}</li>)}</ul>)}
  </section>
}
