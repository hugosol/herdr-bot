/**
 * HTML 阅读模式的文档净化与链接分类。
 *
 * 阅读模式只渲染用户明确选中的本地 HTML；脚本、表单、嵌入内容和外部资源自动加载都被移除或由
 * CSP/iframe sandbox 再次限制。相对 CSS、图片等资源通过后端的 workspace 原始资源路由加载，根目录
 * 仍是 workspace 本身。
 */

const READER_ROUTE = '/api/workspaces/reader/'

const CONTENT_SECURITY_POLICY = [
  "default-src 'none'",
  "img-src 'self' data: blob:",
  "media-src 'self' data: blob:",
  "font-src 'self' data:",
  "style-src 'self' 'unsafe-inline'",
  "script-src 'none'",
  "object-src 'none'",
  "frame-src 'none'",
  "child-src 'none'",
  "connect-src 'none'",
  "form-action 'none'",
  "base-uri 'self'",
].join('; ')

const REMOVED_TAGS: Record<string, true> = {
  script: true,
  iframe: true,
  frame: true,
  frameset: true,
  object: true,
  embed: true,
  applet: true,
  form: true,
}

const REMOVED_ATTRIBUTES: Record<string, true> = {
  srcdoc: true,
  ping: true,
  download: true,
  formaction: true,
  action: true,
}

/** 扩展名决定是否提供“阅读 HTML”；大小写不敏感，查询与片段不参与。 */
export function isHtmlPath(path: string): boolean {
  return /\.html?$/i.test(path.split(/[?#]/, 1)[0] ?? '')
}

function workspaceHex(workspace: string): string {
  const bytes = new TextEncoder().encode(workspace)
  return Array.from(bytes, (byte) => byte.toString(16).padStart(2, '0')).join('')
}

/** HTML 阅读资源路由的路径前缀；相对链接与资源都从同一 workspace 前缀解析。 */
function readerRoutePrefix(workspace: string): string {
  return `${READER_ROUTE}${workspaceHex(workspace)}/`
}

/** 为当前 HTML 构造绝对资源地址，作为 iframe 文档的 `<base href>`。 */
export function readerAssetUrl(
  workspace: string,
  path: string,
  origin = window.location.origin,
): string {
  const encodedPath = path
    .replace(/^\/+/, '')
    .split('/')
    .map((segment) => encodeURIComponent(segment))
    .join('/')
  return `${origin}${readerRoutePrefix(workspace)}${encodedPath}`
}

function routePrefixFromDocument(documentUrl: string): string {
  const pathname = new URL(documentUrl).pathname
  const markerIndex = pathname.indexOf(READER_ROUTE)
  if (markerIndex < 0) return pathname
  const remainder = pathname.slice(markerIndex + READER_ROUTE.length)
  const slash = remainder.indexOf('/')
  return pathname.slice(
    0,
    markerIndex + READER_ROUTE.length + (slash < 0 ? remainder.length : slash + 1),
  )
}

function withoutPrefix(url: URL, documentUrl: string): string | null {
  const base = new URL(documentUrl)
  const prefix = routePrefixFromDocument(documentUrl)
  if (url.origin !== base.origin || !url.pathname.startsWith(prefix)) {
    return null
  }
  try {
    return decodeURIComponent(url.pathname.slice(prefix.length))
  } catch {
    return null
  }
}

function allowedResourceUrl(raw: string, documentUrl: string): boolean {
  const value = raw.trim()
  if (value === '' || value.startsWith('#')) return true
  if (value.startsWith('data:')) return true
  try {
    return withoutPrefix(new URL(value, documentUrl), documentUrl) !== null
  } catch {
    return false
  }
}

function allowedSrcset(raw: string): boolean {
  return !/(?:^|,)\s*(?:https?:)?\/\//i.test(raw) && !/file:/i.test(raw)
}

/**
 * 清洗 HTML，并加入 base/CSP。
 *
 * 这里移除脚本、表单、嵌入、meta refresh、事件处理器和内联导航属性；外部资源属性也被移除，
 * 使“不自动加载外网资源”同时由净化结果、CSP 与 sandbox 三层保证。外部 `<a>` 仍保留，由父页面
 * 的点击拦截改为新标签页打开。
 */
export function buildHtmlReaderDocument(html: string, documentUrl: string): string {
  const parser = new DOMParser()
  const document = parser.parseFromString(html, 'text/html')

  for (const tag of Object.keys(REMOVED_TAGS)) {
    for (const element of Array.from(document.querySelectorAll(tag))) {
      element.remove()
    }
  }

  for (const element of Array.from(document.querySelectorAll('base'))) {
    element.remove()
  }
  for (const meta of Array.from(document.querySelectorAll('meta[http-equiv]'))) {
    const equivalent = meta.getAttribute('http-equiv')?.toLowerCase()
    if (equivalent === 'refresh' || equivalent === 'content-security-policy') {
      meta.remove()
    }
  }

  for (const element of Array.from(document.querySelectorAll('*'))) {
    for (const attribute of Array.from(element.attributes)) {
      const name = attribute.name.toLowerCase()
      const value = attribute.value
      if (name.startsWith('on') || REMOVED_ATTRIBUTES[name] === true) {
        element.removeAttribute(attribute.name)
        continue
      }

      const tagName = element.tagName.toLowerCase()
      const isLink = tagName === 'a' || tagName === 'area'
      if (name === 'href' && isLink) {
        if (/^(?:javascript|vbscript|file):/i.test(value.trim())) {
          element.removeAttribute(attribute.name)
        }
        continue
      }

      if (name === 'srcset' || name === 'imagesrcset') {
        if (!allowedSrcset(value)) element.removeAttribute(attribute.name)
        continue
      }
      if (
        name === 'src' ||
        name === 'href' ||
        name === 'xlink:href' ||
        name === 'poster' ||
        name === 'background' ||
        name === 'data'
      ) {
        if (!allowedResourceUrl(value, documentUrl)) {
          element.removeAttribute(attribute.name)
        }
      }
    }
  }

  const head = document.head
  const charset = document.createElement('meta')
  charset.setAttribute('charset', 'utf-8')
  const csp = document.createElement('meta')
  csp.setAttribute('http-equiv', 'Content-Security-Policy')
  csp.setAttribute('content', CONTENT_SECURITY_POLICY)
  const base = document.createElement('base')
  base.setAttribute('href', documentUrl)
  head.prepend(charset, csp, base)

  return `<!doctype html>${document.documentElement.outerHTML}`
}

export type HtmlReaderLink =
  | { kind: 'anchor' }
  | { kind: 'workspace'; path: string }
  | { kind: 'external'; url: string }
  | { kind: 'blocked'; reason: string }

/**
 * 把 iframe 内的链接分成四类：页内锚点、workspace 内 HTML、外部网页、以及必须拦截的本地/其他方案。
 */
export function classifyHtmlReaderLink(
  rawHref: string,
  documentUrl: string,
): HtmlReaderLink {
  const href = rawHref.trim()
  if (href.startsWith('#')) return { kind: 'anchor' }
  let url: URL
  try {
    url = new URL(href, documentUrl)
  } catch {
    return { kind: 'blocked', reason: '链接地址无效，未打开。' }
  }

  if (url.protocol === 'file:') {
    return {
      kind: 'blocked',
      reason: 'file:// 链接不会打开服务器上的其他本地文件。',
    }
  }
  if (url.protocol === 'http:' || url.protocol === 'https:') {
    const relative = withoutPrefix(url, documentUrl)
    if (relative !== null) {
      if (!isHtmlPath(relative)) {
        return {
          kind: 'blocked',
          reason: '只有 workspace 内的 HTML 文件可以在阅读模式中打开。',
        }
      }
      return { kind: 'workspace', path: relative }
    }
    return { kind: 'external', url: url.href }
  }
  return {
    kind: 'blocked',
    reason: `不会从阅读模式打开 ${url.protocol} 链接。`,
  }
}
