export type OmpSkillSource = { kind: 'collection' | 'skill'; path: string }
export type Suite = {
  id: string
  name: string
  extensions: string[]
  mcp: string[]
} & (
  { harness: 'omp'; skills: OmpSkillSource[]; lsp: string[]; dap: string[]; prompts?: never; codemode?: never } |
  { harness: 'pi'; skills: string[]; prompts: string[]; codemode: 'on' | 'only'; lsp?: never; dap?: never }
)
export type SuiteFixedExtension = {
  id: string
  label: string
  agent: 'omp' | 'pi'
  kind: 'fixed'
  entrypoint: string
  roles: ('standalone' | 'manager' | 'node' | 'chief' | 'chief-delegated')[]
}
export type ResourceKind = 'mcp' | 'lsp' | 'dap'
export type ResourceDefinition = {
  name: string
  description: string
  config: Record<string, unknown>
}
export type ResourceOption = ResourceDefinition & { id: string }
export type SuiteList = { suites: Suite[]; mcp: ResourceOption[]; lsp: ResourceOption[]; dap: ResourceOption[]; fixedExtensions: SuiteFixedExtension[] }

const isStringArray = (value: unknown): value is string[] =>
  Array.isArray(value) && value.every(item => typeof item === 'string')

function isResourceOption(value: unknown): value is ResourceOption {
  if (value === null || typeof value !== 'object' || Array.isArray(value)) return false
  const { id, name, description, config } = value as Record<string, unknown>
  return typeof id === 'string' && typeof name === 'string' && typeof description === 'string'
    && config !== null && typeof config === 'object' && !Array.isArray(config)
    && Object.keys(value).every(key => ['id', 'name', 'description', 'config'].includes(key))
}

function isOmpSkillSource(value: unknown): value is OmpSkillSource {
  if (value === null || typeof value !== 'object' || Array.isArray(value)) return false
  const { kind, path } = value as Record<string, unknown>
  return (kind === 'collection' || kind === 'skill') && typeof path === 'string' && path.trim() !== ''
    && Object.keys(value).every(key => key === 'kind' || key === 'path')
}

export function isSuite(value: unknown): value is Suite {
  if (value === null || typeof value !== 'object' || Array.isArray(value)) return false
  const { id, name, harness, skills, extensions, mcp, lsp, dap, prompts, codemode } = value as Record<string, unknown>
  if (harness !== 'omp' && harness !== 'pi') return false
  const nameSuffix = harness === 'omp' ? ' · OMP' : ' · Pi'
  if (typeof id !== 'string' || !/^[a-zA-Z0-9_-]+$/.test(id)
    || typeof name !== 'string' || !name.endsWith(nameSuffix) || !name.slice(0, -nameSuffix.length).trim()
    || !isStringArray(extensions) || !isStringArray(mcp)) return false
  const keys = ['id', 'name', 'harness', 'skills', 'extensions', 'mcp']
  if (harness === 'omp') return Array.isArray(skills) && skills.every(isOmpSkillSource)
    && isStringArray(lsp) && isStringArray(dap)
    && Object.keys(value).every(key => keys.includes(key) || key === 'lsp' || key === 'dap')
  return isStringArray(skills) && isStringArray(prompts) && (codemode === 'on' || codemode === 'only')
    && Object.keys(value).every(key => keys.includes(key) || key === 'prompts' || key === 'codemode')
}

export async function suiteRequest(method = 'GET', id?: string, suite?: Suite): Promise<SuiteList> {
  const response = await fetch(`/api/suites${id === undefined ? '' : `/${encodeURIComponent(id)}`}`, {
    method,
    ...(suite === undefined ? {} : { headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(suite) }),
  })
  return readSuiteResponse(response)
}

export async function saveResource(kind: ResourceKind, id: string, raw: string): Promise<SuiteList> {
  const response = await fetch(`/api/suites/resources/${kind}/${encodeURIComponent(id)}`, {
    method: 'PUT', headers: { 'Content-Type': 'application/json' }, body: raw,
  })
  return readSuiteResponse(response)
}

async function readSuiteResponse(response: Response): Promise<SuiteList> {
  const result = await response.json()
  if (!response.ok) throw new Error(typeof result.message === 'string' ? result.message : 'Suite 请求失败。')
  if (result === null || typeof result !== 'object'
    || !Array.isArray(result.suites) || !result.suites.every(isSuite)
    || !(['mcp', 'lsp', 'dap'] as const).every(kind => Array.isArray(result[kind]) && result[kind].every(isResourceOption))
    || Object.keys(result).some(key => !['suites', 'mcp', 'lsp', 'dap', 'fixedExtensions'].includes(key))
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
