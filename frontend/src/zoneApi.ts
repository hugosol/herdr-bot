/**
 * zone 登记接口客户端：页面与接口同源，请求地址直接使用相对路径。
 *
 * 登记列表由后端持久保存，电脑与手机读取同一份；页面不缓存列表，也不使用客户端文件选择器。
 */

/** 一个已登记的 zone：服务器上的绝对目录路径。 */
export type ZoneRegistration = {
  path: string
}

/** 后端配置是否允许新建 zone，以及创建父目录的展示值。 */
export type ZoneCreation = {
  available: boolean
  parentDirectory?: string
}

/**
 * 一次 zone 请求在页面一侧的可能归宿。
 *
 * - `ok`：后端返回了当前完整列表；只有这时才能更新界面或显示成功。
 * - `rejected`：后端给出了明确的失败类别与说明（非法路径、存储失败等）。
 * - `unreachable`：没有得到可用响应（网络中断或响应无法解析），不能当成成功或失败。
 */
export type ZoneResult =
  | {
      kind: 'ok'
      zones: ZoneRegistration[]
      creation: ZoneCreation
    }
  | { kind: 'rejected'; code: string; message: string }
  | { kind: 'unreachable' }

/** 单个登记项的形状检查。 */
function isZoneRegistration(value: unknown): value is ZoneRegistration {
  return (
    typeof value === 'object' &&
    value !== null &&
    'path' in value &&
    typeof value.path === 'string'
  )
}

function isZoneCreation(value: unknown): value is ZoneCreation {
  return (
    typeof value === 'object' &&
    value !== null &&
    'available' in value &&
    typeof value.available === 'boolean' &&
    (!('parentDirectory' in value) ||
      value.parentDirectory === undefined ||
      typeof value.parentDirectory === 'string')
  )
}

/** 列表响应的形状检查：形状不对的响应不能被当作成功。 */
function isZoneList(
  value: unknown,
): value is { zones: ZoneRegistration[]; creation?: ZoneCreation } {
  if (typeof value !== 'object' || value === null || !('zones' in value)) {
    return false
  }
  if (!Array.isArray(value.zones) || !value.zones.every(isZoneRegistration)) {
    return false
  }
  return !('creation' in value) || value.creation === undefined || isZoneCreation(value.creation)
}

/** 明确失败响应的形状检查。 */
function isZoneError(value: unknown): value is { code: string; message: string } {
  return (
    typeof value === 'object' &&
    value !== null &&
    'code' in value &&
    typeof value.code === 'string' &&
    'message' in value &&
    typeof value.message === 'string'
  )
}

/**
 * 发出一次 zone 请求并归一化答复。
 *
 * 只有 HTTP 成功且响应形状正确才算 `ok`；明确的错误码与说明原样交给界面；
 * 其余情形（网络中断、非 JSON、形状不符）一律是 `unreachable`，不能显示成成功。
 */
async function zoneRequest(
  input: RequestInfo | URL,
  init?: RequestInit,
): Promise<ZoneResult> {
  let response: Response
  try {
    response = await fetch(input, init)
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
    return isZoneList(payload)
      ? {
          kind: 'ok',
          zones: payload.zones,
          creation: payload.creation ?? { available: false },
        }
      : { kind: 'unreachable' }
  }

  return isZoneError(payload)
    ? { kind: 'rejected', code: payload.code, message: payload.message }
    : { kind: 'unreachable' }
}

/** 读取当前登记列表；每次进入文件列表都重新读取后端，不使用页面缓存。 */
export function fetchZones(): Promise<ZoneResult> {
  return zoneRequest('/api/zones')
}

/** 登记一个服务器目录；成功只意味着后端已经持久化。 */
export function registerZone(path: string): Promise<ZoneResult> {
  return zoneRequest('/api/zones', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ path }),
  })
}

/** 新建 zone：只提交名称，父目录必须由后端配置决定。 */
export function createZone(name: string): Promise<ZoneResult> {
  return zoneRequest('/api/zones/create', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ name }),
  })
}

/** 取消登记；只移除登记项，不删除目录或文件。 */
export function unregisterZone(path: string): Promise<ZoneResult> {
  return zoneRequest('/api/zones/remove', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ path }),
  })
}
