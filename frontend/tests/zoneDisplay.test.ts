// @vitest-environment node
import { describe, expect, it } from 'vitest'

import { zoneDisplayName } from '../src/zoneDisplay'

describe('zoneDisplayName', () => {
  it('只保留 Windows 与 POSIX 路径的末级目录名', () => {
    expect(zoneDisplayName('C:\\work\\alpha')).toBe('alpha')
    expect(zoneDisplayName('D:\\projects\\alpha\\')).toBe('alpha')
    expect(zoneDisplayName('/home/user/project')).toBe('project')
    expect(zoneDisplayName('\\\\server\\share\\project')).toBe('project')
  })

  it('根目录没有普通末级名称时保留输入，不显示空白', () => {
    expect(zoneDisplayName('/')).toBe('/')
    expect(zoneDisplayName('C:\\')).toBe('C:')
    expect(zoneDisplayName('')).toBe('')
  })
})
