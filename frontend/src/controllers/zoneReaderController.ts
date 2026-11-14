/** HTML 阅读资源地址；文档净化与链接安全规则仍由 htmlReader 持有。 */
export const READER_ROUTE = '/api/zones/reader/'

function zoneHex(zone: string): string {
  const bytes = new TextEncoder().encode(zone)
  return Array.from(bytes, (byte) => byte.toString(16).padStart(2, '0')).join('')
}

/** HTML 阅读资源路由的路径前缀；相对链接与资源都从同一 zone 前缀解析。 */
function readerRoutePrefix(zone: string): string {
  return `${READER_ROUTE}${zoneHex(zone)}/`
}

/** 为当前 HTML 构造绝对资源地址，作为 iframe 文档的 `<base href>`。 */
export function readerAssetUrl(
  zone: string,
  path: string,
  origin = window.location.origin,
): string {
  const encodedPath = path
    .replace(/^\/+/, '')
    .split('/')
    .map((segment) => encodeURIComponent(segment))
    .join('/')
  return `${origin}${readerRoutePrefix(zone)}${encodedPath}`
}
