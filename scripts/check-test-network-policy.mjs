import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')

// Exact infrastructure functions, not directory-wide exemptions. Runtime helpers
// enforce addresses; this check catches accidental new listener implementations.
const infrastructure = new Map([
  ['backend/tests/common/mod.rs', new Set(['bind_app_on'])],
  ['backend/tests/mdns_conflict.rs', new Set(['start', 'local_publishable_ipv4', 'hostname_is_answered', 'browse_http_instances'])],
  ['backend/tests/mdns_failure.rs', new Set(['local_publishable_ipv4', 'resolved_addresses', 'browse_http_instances'])],
  ['frontend/browser-tests/support.ts', new Set(['browserListenAddress'])],
])
const manualRustTests = new Map([
  ['backend/tests/session_write_boundary.rs', new Set(['same_host_access_via_local_nic_address_is_accepted'])],
  ['backend/tests/monitoring.rs', new Set(['a_non_loopback_source_may_monitor_and_clear_but_not_write_session_events'])],
])
const mdnsFiles = new Set(['backend/tests/mdns_conflict.rs', 'backend/tests/mdns_failure.rs'])
const manualBrowserFile = 'frontend/browser-tests/lanAccess.manual.spec.ts'

function files(directory) {
  if (!fs.existsSync(directory)) return []
  return fs.readdirSync(directory, { withFileTypes: true }).flatMap(entry => {
    const target = path.join(directory, entry.name)
    return entry.isDirectory() ? files(target) : /\.(rs|tsx?|mjs|cjs|js)$/.test(entry.name) ? [target] : []
  })
}

// Keep offsets/newlines so diagnostics point at the original file. Mask comments
// and literals before looking for executable APIs (including Rust raw strings).
function codeOnly(source) {
  return source.replace(/r(#+)?"[\s\S]*?"\1|\/\*[\s\S]*?\*\/|\/\/[^\n]*|"(?:\\.|[^"\\])*"|'(?:\\.|[^'\\\n])*'|`(?:\\.|[^`\\])*`/g,
    text => text.replace(/[^\n]/g, ' '))
}

export function inspect(relative, source) {
  const code = codeOnly(source)
  const errors = []
  const functions = [...code.matchAll(/\b(?:fn|function)\s+(\w+)\s*(?:<[^>]*>)?\s*\(/g)]
  const enclosing = offset => functions.findLast(item => item.index < offset)
  const report = (offset, message) => errors.push(`${relative}:${source.slice(0, offset).split('\n').length}: ${message}`)
  const allowed = offset => infrastructure.get(relative)?.has(enclosing(offset)?.[1])
  if (relative !== 'frontend/browser-tests/support.ts') {
    for (const match of source.matchAll(/(?:from\s*|require\s*\(|import\s*\()\s*['"](?:node:)?(?:net|dgram|http|https|http2|tls)['"]/g)) {
      report(match.index, 'Network modules must stay in the controlled server infrastructure (aliases are not an escape).')
    }
  }
  if (!infrastructure.has(relative)) {
    for (const match of code.matchAll(/\buse\s+[^;]*(?:TcpListener|UdpSocket|TcpSocket|ServiceDaemon|socket2)[^;]*;/g)) {
      report(match.index, 'Listener imports must stay in the controlled infrastructure (including renamed imports).')
    }
  }
  const primitives = /\b(?:TcpListener|UdpSocket|TcpSocket|ServiceDaemon|Socket)\s*::\s*(?:bind|new|new_v4|new_v6)\s*\(|\b(?:axum|hyper)\s*::\s*serve\s*\(|\b(?:Bun|Deno)\s*\.\s*serve\s*\(|\bnew\s+(?:WebSocketServer|Server)\s*\(|\b(?:createServer|createSocket)\s*\(|\.\s*(?:listen|bind)\s*\(/g
  for (const match of code.matchAll(primitives)) {
    if (!allowed(match.index)) report(match.index, 'Direct network listener outside approved infrastructure; use the loopback test helper.')
  }
  for (const match of code.matchAll(/\b(?:publish|publish_mdns)\s*\(/g)) {
    report(match.index, 'Real mDNS publishing belongs in the explicit manual infrastructure.')
  }
  for (const match of code.matchAll(/\b(?:start_manual_app_on|startManualServer)\s*\(/g)) {
    const fn = enclosing(match.index)
    const declaration = /\b(?:fn|function)\s*$/.test(code.slice(Math.max(0, match.index - 20), match.index))
    if (declaration) continue
    if (relative === manualBrowserFile && match[0].startsWith('startManualServer')) continue
    if (manualRustTests.get(relative)?.has(fn?.[1])) continue
    report(match.index, 'Manual network helper is only allowed in explicitly registered manual cases.')
  }
  if (relative.endsWith('.rs')) {
    for (const fn of functions) {
      const attributes = source.slice(0, fn.index).match(/(?:\s*#\[[^\]]*\]\s*)+(?:pub\s+)?(?:async\s+)?$/)?.[0] ?? ''
      const test = /#\[(?:tokio::)?test\b/.test(attributes)
      const ignored = /#\[ignore(?:\s|=|\])/.test(attributes)
      if (test && mdnsFiles.has(relative) && !ignored) report(fn.index, 'Real mDNS tests must be ignored by default.')
      if (manualRustTests.get(relative)?.has(fn[1]) && (!test || !ignored)) report(fn.index, 'Manual LAN cases must remain ignored tests.')
    }
  }
  return errors
}

export function checkRepository(base = root) {
  return ['backend/tests', 'frontend/tests', 'frontend/browser-tests'].flatMap(directory =>
    files(path.join(base, directory)).flatMap(file => inspect(path.relative(base, file).replaceAll('\\', '/'), fs.readFileSync(file, 'utf8'))))
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const errors = checkRepository()
  if (errors.length) {
    console.error(errors.join('\n'))
    process.exitCode = 1
  } else {
    console.log('Test network policy: passed (default helpers are loopback-only; manual exceptions are explicit).')
  }
}
