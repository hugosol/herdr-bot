import type { BrowseEntry } from '../src/zones/browser/browseApi'

/** 把测试夹具的目录映射转换成整树接口响应。测试只通过公开响应形状驱动页面。 */
export function treeResponse(
  zone: string,
  directories: Record<string, BrowseEntry[]>,
  ignoredHidden = false,
) {
  return {
    zone,
    directories: Object.entries(directories).map(([path, entries]) => ({
      path,
      entries,
    })),
    ignoredHidden,
  }
}
