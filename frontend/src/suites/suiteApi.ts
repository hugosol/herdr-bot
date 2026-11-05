export type Suite = {
  id: string
  name: string
  agents: ('omp' | 'pi')[]
  skills: string[]
  extensions: Partial<Record<'omp' | 'pi', string[]>>
  prompts: string[]
  lsp: Record<string, unknown>
  mcp: string[]
  pi?: { codemode: 'on' | 'only' }
}
export type SuiteFixedExtension = {
  id: string
  label: string
  agent: 'omp' | 'pi'
  kind: 'fixed'
  entrypoint: string
  roles: ('standalone' | 'manager' | 'node')[]
}
export type SuiteList = { suites: Suite[]; mcp: string[]; fixedExtensions: SuiteFixedExtension[] }

export async function suiteRequest(method = 'GET', id?: string, suite?: Suite): Promise<SuiteList> {
  const response = await fetch(`/api/suites${id === undefined ? '' : `/${encodeURIComponent(id)}`}`, {
    method,
    ...(suite === undefined ? {} : { headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(suite) }),
  })
  const result = await response.json()
  if (!response.ok) throw new Error(typeof result.message === 'string' ? result.message : 'Suite 请求失败。')
  if (result === null || typeof result !== 'object'
    || !Array.isArray(result.suites) || !Array.isArray(result.mcp)
    || Object.keys(result).some(key => !['suites', 'mcp', 'fixedExtensions'].includes(key))
    || !Array.isArray(result.fixedExtensions) || !result.fixedExtensions.every((extension: unknown) => {
      if (extension === null || typeof extension !== 'object') return false
      const option = extension as Record<string, unknown>
      return typeof option.id === 'string' && typeof option.label === 'string'
        && (option.agent === 'omp' || option.agent === 'pi')
        && option.kind === 'fixed' && typeof option.entrypoint === 'string'
        && Array.isArray(option.roles) && option.roles.every(role => role === 'standalone' || role === 'manager' || role === 'node')
    })) throw new Error('Suite 响应无效，未确认保存。')
  return result as SuiteList
}
