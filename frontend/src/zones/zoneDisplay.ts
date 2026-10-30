/** zone 列表中的短名称：只取注册路径的末级目录名，完整路径仍用于身份与详情。 */
export function zoneDisplayName(path: string): string {
  const trimmed = path.replace(/[\\/]+$/, '')
  if (trimmed === '') return path
  const parts = trimmed.split(/[\\/]+/)
  const last = parts[parts.length - 1]
  return last === '' ? path : last
}
