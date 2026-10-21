import { useCallback, useMemo, useRef, useState } from 'react'

import {
  fetchFile,
  type BrowseResult,
  type FileView,
} from './browseApi'
import {
  buildHtmlReaderDocument,
  classifyHtmlReaderLink,
  isHtmlPath,
  readerAssetUrl,
} from './htmlReader'

/** 阅读模式内的 HTML 文件读取；只有 HTML 文本能替换当前文档。 */
function readFailure(result: BrowseResult<FileView>, path: string): string {
  if (result.kind === 'rejected') return result.message
  if (result.kind === 'unreachable') {
    return `无法确认 HTML 文件 ${path} 的读取结果：请求没有得到可用响应。`
  }
  if (result.value.preview.kind === 'binary') {
    return `不可阅读：${result.value.preview.reason}`
  }
  if (result.value.preview.kind === 'too-large') {
    return `文件大小 ${result.value.preview.size} 字节，超过预览上限 ${result.value.preview.limit} 字节；未加载完整内容。`
  }
  return `文件 ${path} 不是可阅读的 HTML 文本。`
}

/**
 * 全屏 HTML 阅读模式。
 *
 * 内容先在父页面净化，再放进只允许同源、禁止脚本的 sandbox iframe。zone 内 HTML 链接由父页面
 * 继续在当前阅读模式打开；外部 http(s) 链接在新标签页打开；file:// 与其他 scheme 被拦截。
 */
export function HtmlReader({
  zone,
  initialPath,
  initialSource,
  onClose,
}: {
  zone: string
  initialPath: string
  initialSource: string
  onClose: () => void
}) {
  const [current, setCurrent] = useState({ path: initialPath, source: initialSource })
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [notice, setNotice] = useState<string | null>(null)
  const frameRef = useRef<HTMLIFrameElement>(null)
  const sequence = useRef(0)

  const navigateTo = useCallback(
    async (path: string) => {
      if (!isHtmlPath(path)) {
        setNotice('只有 zone 内的 HTML 文件可以在阅读模式中打开。')
        return
      }
      const run = sequence.current + 1
      sequence.current = run
      setLoading(true)
      setError(null)
      setNotice(null)
      const result = await fetchFile(zone, path)
      if (sequence.current !== run) return
      setLoading(false)
      if (result.kind === 'ok' && result.value.preview.kind === 'text') {
        setCurrent({ path, source: result.value.preview.content })
        return
      }
      setError(readFailure(result, path))
    },
    [zone],
  )

  const documentUrl = readerAssetUrl(zone, current.path)
  const srcDoc = useMemo(
    () => buildHtmlReaderDocument(current.source, documentUrl),
    [current.source, documentUrl],
  )

  function handleFrameLoad() {
    const document = frameRef.current?.contentDocument
    if (document === null || document === undefined) return
    document.addEventListener('click', (event) => {
      const target = event.target as { closest?: (selector: string) => Element | null } | null
      const anchor = target?.closest?.('a[href]')
      if (anchor === null || anchor === undefined) return
      const href = anchor.getAttribute('href')
      if (href === null) return
      const action = classifyHtmlReaderLink(href, documentUrl)
      switch (action.kind) {
        case 'anchor':
          return
        case 'zone':
          event.preventDefault()
          void navigateTo(action.path)
          return
        case 'external':
          event.preventDefault()
          window.open(action.url, '_blank', 'noopener,noreferrer')
          return
        case 'blocked':
          event.preventDefault()
          setNotice(action.reason)
          return
      }
    })
  }

  return (
    <section
      className="html-reader"
      data-testid="html-reader"
      role="dialog"
      aria-modal="true"
      aria-label="HTML 阅读模式"
    >
      <header className="html-reader__bar">
        <code className="html-reader__path" data-testid="html-reader-path">
          {current.path}
        </code>
        <button
          type="button"
          className="html-reader__exit"
          data-testid="exit-html-reader"
          onClick={onClose}
        >
          退出
        </button>
      </header>

      {loading && (
        <p className="html-reader__status" role="status">
          正在读取 HTML……
        </p>
      )}
      {error !== null && (
        <p className="html-reader__error" data-testid="html-reader-error" role="alert">
          {error}
        </p>
      )}
      {notice !== null && (
        <p className="html-reader__notice" data-testid="html-reader-notice" role="status">
          {notice}
        </p>
      )}

      <iframe
        ref={frameRef}
        className="html-reader__frame"
        data-testid="html-reader-frame"
        title={`HTML 阅读：${current.path}`}
        sandbox="allow-same-origin"
        referrerPolicy="no-referrer"
        srcDoc={srcDoc}
        onLoad={handleFrameLoad}
      />
    </section>
  )
}
