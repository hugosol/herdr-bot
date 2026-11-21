export type Suite = {
  id: string
  name: string
  skills: string[]
  extensions: string[]
  mcp: string[]
} & (
  { harness: 'omp'; lsp: Record<string, unknown>; prompts?: never; codemode?: never } |
  { harness: 'pi'; prompts: string[]; codemode: 'on' | 'only'; lsp?: never }
)
export type SuiteFixedExtension = {
  id: string
  label: string
  agent: 'omp' | 'pi'
  kind: 'fixed'
  entrypoint: string
  roles: ('standalone' | 'manager' | 'node' | 'chief' | 'chief-delegated')[]
}
export type SuiteList = { suites: Suite[]; mcp: string[]; fixedExtensions: SuiteFixedExtension[] }

const isStringArray = (value: unknown): value is string[] =>
  Array.isArray(value) && value.every(item => typeof item === 'string')

export function isSuite(value: unknown): value is Suite {
  if (value === null || typeof value !== 'object' || Array.isArray(value)) return false
  const { id, name, harness, skills, extensions, mcp, lsp, prompts, codemode } = value as Record<string, unknown>
  if (harness !== 'omp' && harness !== 'pi') return false
  const idSuffix = `-${harness}`
  const nameSuffix = harness === 'omp' ? ' · OMP' : ' · Pi'
  if (typeof id !== 'string' || !id.endsWith(idSuffix) || !id.slice(0, -idSuffix.length).trim()
    || typeof name !== 'string' || !name.endsWith(nameSuffix) || !name.slice(0, -nameSuffix.length).trim()
    || !isStringArray(skills) || !isStringArray(extensions) || !isStringArray(mcp)) return false
  const keys = ['id', 'name', 'harness', 'skills', 'extensions', 'mcp']
  if (harness === 'omp') return lsp !== null && typeof lsp === 'object' && !Array.isArray(lsp)
    && Object.keys(value).every(key => keys.includes(key) || key === 'lsp')
  return isStringArray(prompts) && (codemode === 'on' || codemode === 'only')
    && Object.keys(value).every(key => keys.includes(key) || key === 'prompts' || key === 'codemode')
}

export async function suiteRequest(method = 'GET', id?: string, suite?: Suite): Promise<SuiteList> {
  const response = await fetch(`/api/suites${id === undefined ? '' : `/${encodeURIComponent(id)}`}`, {
    method,
    ...(suite === undefined ? {} : { headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(suite) }),
  })
  const result = await response.json()
  if (!response.ok) throw new Error(typeof result.message === 'string' ? result.message : 'Suite 请求失败。')
  if (result === null || typeof result !== 'object'
    || !Array.isArray(result.suites) || !result.suites.every(isSuite) || !isStringArray(result.mcp)
    || Object.keys(result).some(key => !['suites', 'mcp', 'fixedExtensions'].includes(key))
    || !Array.isArray(result.fixedExtensions) || !result.fixedExtensions.every((extension: unknown) => {
      if (extension === null || typeof extension !== 'object') return false
      const option = extension as Record<string, unknown>
      return typeof option.id === 'string' && typeof option.label === 'string'
        && (option.agent === 'omp' || option.agent === 'pi')
        && option.kind === 'fixed' && typeof option.entrypoint === 'string'
        && Array.isArray(option.roles) && option.roles.every(role => role === 'standalone' || role === 'manager' || role === 'node' || role === 'chief' || role === 'chief-delegated')
    })) throw new Error('Suite 响应无效，未确认保存。')
  return result as SuiteList
}
