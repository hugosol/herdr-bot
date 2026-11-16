/**
 * zone 只读浏览接口客户端。
 *
 * 目录与文件读取都以已登记的 zone 路径 + 相对路径定位；后端负责边界检查。页面在进入
 * zone 时读取一次整树快照，之后只有手动刷新会重读；单目录与单文件入口保留给直接读取。
 */

export type EntryKind = 'directory' | 'file' | 'link'

/** 目录里的一个条目。链接条目会带上目标与不可进入原因。 */
export type BrowseEntry = {
  name: string
  path: string
  kind: EntryKind
  size?: number
  linkTarget?: string
  linkReason?: string
}

export type DirectoryListing = {
  zone: string
  path: string
  entries: BrowseEntry[]
}

/** 整树读取中某个目录的明确错误；其它目录仍可正常显示。 */
export type TreeDirectoryError = {
  code: string
  message: string
}

export type TreeDirectory = {
  path: string
  entries: BrowseEntry[]
  error?: TreeDirectoryError
}

/** 一次整树读取：目录路径 → 直接子项；ignoredHidden 表示本次是否隐藏了 ignored 路径。 */
export type ZoneTree = {
  zone: string
  directories: TreeDirectory[]
  ignoredHidden: boolean
}

/** 文本、二进制与超限三种预览；只有文本带内容。 */
export type FilePreview =
  | { kind: 'text'; content: string }
  | { kind: 'binary'; reason: string }
  | { kind: 'too-large'; reason: string; size: number; limit: number }

export type FileView = {
  zone: string
  path: string
  name: string
  size: number
  preview: FilePreview
}

/**
 * 一次浏览请求在页面一侧的归宿。
 *
 * - `ok`：后端返回了形状正确的目录或文件结果。
 * - `rejected`：后端给出明确失败类别与说明（越界、链接、不存在、读取失败等）。
 * - `unreachable`：没有得到可用响应；不能当成空目录或空内容。
 */
export type BrowseResult<T> =
  | { kind: 'ok'; value: T }
  | { kind: 'rejected'; code: string; message: string }
  | { kind: 'unreachable' }

function isEntry(value: unknown): value is BrowseEntry {
  if (typeof value !== 'object' || value === null) return false
  if (!('name' in value) || typeof value.name !== 'string') return false
  if (!('path' in value) || typeof value.path !== 'string') return false
  if (!('kind' in value)) return false
  if (value.kind !== 'directory' && value.kind !== 'file' && value.kind !== 'link') return false
  if ('size' in value && value.size !== undefined && typeof value.size !== 'number') return false
  if ('linkTarget' in value && value.linkTarget !== undefined && typeof value.linkTarget !== 'string') {
    return false
  }
  if ('linkReason' in value && value.linkReason !== undefined && typeof value.linkReason !== 'string') {
    return false
  }
  return true
}

function isDirectoryListing(value: unknown): value is DirectoryListing {
  if (typeof value !== 'object' || value === null) return false
  if (!('zone' in value) || typeof value.zone !== 'string') return false
  if (!('path' in value) || typeof value.path !== 'string') return false
  return (
    'entries' in value &&
    Array.isArray(value.entries) &&
    value.entries.every(isEntry)
  )
}

function isPreview(value: unknown): value is FilePreview {
  if (typeof value !== 'object' || value === null || !('kind' in value)) return false
  if (value.kind === 'text') {
    return 'content' in value && typeof value.content === 'string'
  }
  if (value.kind === 'binary') {
    return 'reason' in value && typeof value.reason === 'string'
  }
  if (value.kind === 'too-large') {
    return (
      'reason' in value &&
      typeof value.reason === 'string' &&
      'size' in value &&
      typeof value.size === 'number' &&
      'limit' in value &&
      typeof value.limit === 'number'
    )
  }
  return false
}

function isTreeDirectory(value: unknown): value is TreeDirectory {
  if (typeof value !== 'object' || value === null) return false
  if (!('path' in value) || typeof value.path !== 'string') return false
  if (!('entries' in value) || !Array.isArray(value.entries)) return false
  if (!value.entries.every(isEntry)) return false
  if ('error' in value && value.error !== undefined) {
    return (
      typeof value.error === 'object' &&
      value.error !== null &&
      'code' in value.error &&
      typeof value.error.code === 'string' &&
      'message' in value.error &&
      typeof value.error.message === 'string'
    )
  }
  return true
}

function isZoneTree(value: unknown): value is ZoneTree {
  if (typeof value !== 'object' || value === null) return false
  if (!('zone' in value) || typeof value.zone !== 'string') return false
  if (!('ignoredHidden' in value) || typeof value.ignoredHidden !== 'boolean') return false
  return (
    'directories' in value &&
    Array.isArray(value.directories) &&
    value.directories.every(isTreeDirectory)
  )
}

function isFileView(value: unknown): value is FileView {
  if (typeof value !== 'object' || value === null) return false
  if (!('zone' in value) || typeof value.zone !== 'string') return false
  if (!('path' in value) || typeof value.path !== 'string') return false
  if (!('name' in value) || typeof value.name !== 'string') return false
  if (!('size' in value) || typeof value.size !== 'number') return false
  return 'preview' in value && isPreview(value.preview)
}

function isErrorBody(value: unknown): value is { code: string; message: string } {
  return (
    typeof value === 'object' &&
    value !== null &&
    'code' in value &&
    typeof value.code === 'string' &&
    'message' in value &&
    typeof value.message === 'string'
  )
}

async function browseRequest<T>(
  url: string,
  accept: (payload: unknown) => payload is T,
): Promise<BrowseResult<T>> {
  let response: Response
  try {
    response = await fetch(url)
  } catch {
    return { kind: 'unreachable' }
  }

  let payload: unknown
  try {
    payload = await response.json()
  } catch {
    payload = null
  }

  if (response.ok) {
    return accept(payload) ? { kind: 'ok', value: payload } : { kind: 'unreachable' }
  }
  return isErrorBody(payload)
    ? { kind: 'rejected', code: payload.code, message: payload.message }
    : { kind: 'unreachable' }
}

function browseUrl(endpoint: string, zone: string, path: string): string {
  const params = new URLSearchParams({ zone, path })
  return `/api/zones/${endpoint}?${params.toString()}`
}

/** 读取一个目录的直接子项；空字符串表示 zone 根目录。 */
export function fetchDirectory(
  zone: string,
  path: string,
): Promise<BrowseResult<DirectoryListing>> {
  return browseRequest(browseUrl('entries', zone, path), isDirectoryListing)
}

/** 一次读取完整目录树；Git zone 的 ignored 路径已由后端省略。 */
export function fetchTree(zone: string): Promise<BrowseResult<ZoneTree>> {
  const params = new URLSearchParams({ zone })
  return browseRequest(`/api/zones/tree?${params.toString()}`, isZoneTree)
}

/** 读取单个文件元数据与内容预览。 */
export function fetchFile(
  zone: string,
  path: string,
): Promise<BrowseResult<FileView>> {
  return browseRequest(browseUrl('file', zone, path), isFileView)
}
