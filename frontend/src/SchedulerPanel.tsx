import { useEffect, useState } from 'react'
import { listMaps, loadMap, type WorkflowMap } from './schedulerApi'
import { MapGraph } from './MapGraph'
import { MapNodeDetails } from './MapNodeDetails'
import './scheduler.css'

type Listing = { kind: 'loading' } | { kind: 'ready'; files: string[] } | { kind: 'error'; message: string }

export function SchedulerPanel({ active }: { active: boolean }) {
  const [listing, setListing] = useState<Listing>({ kind: 'loading' })
  const [revision, setRevision] = useState(0)
  const [file, setFile] = useState<string | null>(null)
  const [preview, setPreview] = useState<{ kind: 'loading' } | { kind: 'ready'; map: WorkflowMap } | { kind: 'error'; message: string } | null>(null)
  const [selectedId, setSelectedId] = useState<string | null>(null)
  useEffect(() => {
    if (!active || file === null) return
    const controller = new AbortController()
    setPreview({ kind: 'loading' })
    setSelectedId(null)
    loadMap(file, controller.signal).then(
      (map) => { if (!controller.signal.aborted) setPreview({ kind: 'ready', map }) },
      (error: unknown) => {
        if (!controller.signal.aborted) setPreview({ kind: 'error', message: error instanceof Error ? error.message : 'Map 读取失败。' })
      },
    )
    return () => controller.abort()
  }, [active, file, revision])
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
  return <section className="scheduler" aria-label="Map 目录">
    <h2>scheduler</h2>
    <p>服务主机上的 Map，与当前 zone 无关。仅列出目录直接包含的 JSON 文件。</p>
    <button type="button" onClick={() => setRevision((value) => value + 1)}>刷新 Map 列表</button>
    {listing.kind === 'loading' && <p role="status">正在读取 Map 目录……</p>}
    {listing.kind === 'error' && <p role="alert" data-testid="scheduler-error">{listing.message}</p>}
    {listing.kind === 'ready' && (listing.files.length === 0
      ? <p data-testid="scheduler-empty">此目录没有 Map JSON 文件。</p>
      : <ul data-testid="scheduler-map-list">{listing.files.map((name) => <li key={name}>
        <button type="button" aria-pressed={file === name} onClick={() => setFile(name)}>{name}</button>
      </li>)}</ul>)}
    {preview?.kind === 'loading' && <p role="status">正在读取 Map……</p>}
    {preview?.kind === 'error' && <p role="alert">{preview.message}</p>}
    {preview?.kind === 'ready' && <section aria-label="Map 预览">
      <h3>{preview.map.title}</h3>
      <p className="scheduler-text">{preview.map.description}</p>
      <p>箭头仅表示 blocked_by 前置依赖，不表示执行、材料传递或运行状态。</p>
      <MapGraph map={preview.map} selectedId={selectedId} onSelect={setSelectedId} />
      {selectedId !== null && <MapNodeDetails map={preview.map} nodeId={selectedId} />}
    </section>}
  </section>
}
