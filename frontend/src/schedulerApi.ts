export async function listMaps(signal?: AbortSignal): Promise<string[]> {
  const response = await fetch('/api/scheduler/maps', { signal })
  const value: unknown = await response.json()
  if (!response.ok) {
    throw new Error(typeof value === 'object' && value !== null && 'message' in value && typeof value.message === 'string'
      ? value.message : `Map 目录读取失败：HTTP ${response.status}`)
  }
  if (typeof value !== 'object' || value === null || !('files' in value) ||
      !Array.isArray(value.files) || !value.files.every((name) => typeof name === 'string')) {
    throw new Error('Map 目录响应无效。')
  }
  return value.files
}
