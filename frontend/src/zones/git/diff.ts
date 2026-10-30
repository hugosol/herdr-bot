/**
 * 行级 diff：只在页面一侧把旧新文本整理成可渲染的行。
 *
 * 输入是后端已经完成限额与文本识别的内容，因此这里不负责判断文件大小或二进制；
 * 它只做纯文本的行比较。输出只含字符串，不解析 HTML。
 */

export type DiffLineKind = 'context' | 'removed' | 'added'

/** 统一 diff 的一行；`oldNumber` / `newNumber` 为 `null` 表示该侧没有这一行。 */
export type DiffRow = {
  kind: DiffLineKind
  oldNumber: number | null
  newNumber: number | null
  text: string
}

export type DiffCellKind = DiffLineKind | 'empty'

/** 左右对照的一格；空一格表示该侧在此处没有对应行。 */
export type DiffCell = {
  kind: DiffCellKind
  number: number | null
  text: string
}

/** 左右对照的一行。 */
export type SideBySideRow = {
  old: DiffCell
  new: DiffCell
}

/**
 * 按行切分，末尾换行不额外产生一行；空文本是 0 行。
 *
 * CRLF 先归一化为 LF：工作树文件在 Windows 上可能是 CRLF，而 Git 对象保存 LF；
 * Git 会按文本属性/autocrlf 把两者视为同一行内容。如果不先归一化，每行末尾的 `\r`
 * 会让所有行都被误判为修改。
 *
 * 这里不比较“文件末尾是否有换行”这一字节事实：预览与 diff 都以可见行为单位，
 * 而且两侧都经过同一规则，展示不会因此伪造或隐藏整行内容。
 */
export function splitLines(text: string): string[] {
  if (text === '') return []
  const lines = text.replace(/\r\n/g, '\n').split('\n')
  if (lines[lines.length - 1] === '') lines.pop()
  return lines
}

/** LCS 表格的单元上限：超过时退化为“旧侧全部删除 + 新侧全部新增”，仍完整显示两侧。 */
const MAX_LCS_CELLS = 4_000_000

type Operation = { kind: DiffLineKind; text: string }

/**
 * 生成统一 diff 行。
 *
 * 先去掉公共前后缀，再对中间部分求最长公共子序列；这样常见的局部修改不会退化成整文件
 * 重写，而超大的完全改写也有明确上限，避免浏览器为了展示 diff 失去响应。
 */
export function diffRows(oldText: string, newText: string): DiffRow[] {
  const oldLines = splitLines(oldText)
  const newLines = splitLines(newText)

  let prefix = 0
  while (
    prefix < oldLines.length &&
    prefix < newLines.length &&
    oldLines[prefix] === newLines[prefix]
  ) {
    prefix += 1
  }

  let suffix = 0
  while (
    suffix < oldLines.length - prefix &&
    suffix < newLines.length - prefix &&
    oldLines[oldLines.length - 1 - suffix] === newLines[newLines.length - 1 - suffix]
  ) {
    suffix += 1
  }

  const oldMiddle = oldLines.slice(prefix, oldLines.length - suffix)
  const newMiddle = newLines.slice(prefix, newLines.length - suffix)

  const operations: Operation[] = []
  for (let index = 0; index < prefix; index += 1) {
    operations.push({ kind: 'context', text: oldLines[index] })
  }
  if (oldMiddle.length * newMiddle.length <= MAX_LCS_CELLS) {
    operations.push(...lcsOperations(oldMiddle, newMiddle))
  } else {
    for (const text of oldMiddle) operations.push({ kind: 'removed', text })
    for (const text of newMiddle) operations.push({ kind: 'added', text })
  }
  for (let index = oldLines.length - suffix; index < oldLines.length; index += 1) {
    operations.push({ kind: 'context', text: oldLines[index] })
  }

  let oldNumber = 1
  let newNumber = 1
  return operations.map((operation) => {
    const row: DiffRow = {
      kind: operation.kind,
      text: operation.text,
      oldNumber: operation.kind === 'added' ? null : oldNumber,
      newNumber: operation.kind === 'removed' ? null : newNumber,
    }
    if (operation.kind !== 'added') oldNumber += 1
    if (operation.kind !== 'removed') newNumber += 1
    return row
  })
}

function lcsOperations(oldLines: string[], newLines: string[]): Operation[] {
  const rows = oldLines.length
  const columns = newLines.length
  const width = columns + 1
  const table = new Uint32Array((rows + 1) * width)

  for (let row = rows - 1; row >= 0; row -= 1) {
    for (let column = columns - 1; column >= 0; column -= 1) {
      table[row * width + column] =
        oldLines[row] === newLines[column]
          ? table[(row + 1) * width + column + 1] + 1
          : Math.max(
              table[(row + 1) * width + column],
              table[row * width + column + 1],
            )
    }
  }

  const operations: Operation[] = []
  let row = 0
  let column = 0
  while (row < rows && column < columns) {
    if (oldLines[row] === newLines[column]) {
      operations.push({ kind: 'context', text: oldLines[row] })
      row += 1
      column += 1
    } else if (
      table[(row + 1) * width + column] >= table[row * width + column + 1]
    ) {
      operations.push({ kind: 'removed', text: oldLines[row] })
      row += 1
    } else {
      operations.push({ kind: 'added', text: newLines[column] })
      column += 1
    }
  }
  while (row < rows) {
    operations.push({ kind: 'removed', text: oldLines[row] })
    row += 1
  }
  while (column < columns) {
    operations.push({ kind: 'added', text: newLines[column] })
    column += 1
  }
  return operations
}

/**
 * 把统一 diff 行整理成左右对照：连续的删除与新增按顺序配对，剩余一侧留空。
 * 上下文行两侧显示同一行内容。
 */
export function sideBySideRows(rows: DiffRow[]): SideBySideRow[] {
  const pairs: SideBySideRow[] = []
  let index = 0
  while (index < rows.length) {
    const row = rows[index]
    if (row.kind === 'context') {
      pairs.push({
        old: { kind: 'context', number: row.oldNumber, text: row.text },
        new: { kind: 'context', number: row.newNumber, text: row.text },
      })
      index += 1
      continue
    }

    const removed: DiffRow[] = []
    const added: DiffRow[] = []
    while (index < rows.length && rows[index].kind !== 'context') {
      const changed = rows[index]
      if (changed.kind === 'removed') removed.push(changed)
      else added.push(changed)
      index += 1
    }

    const count = Math.max(removed.length, added.length)
    for (let offset = 0; offset < count; offset += 1) {
      const left = removed[offset]
      const right = added[offset]
      pairs.push({
        old: left
          ? { kind: 'removed', number: left.oldNumber, text: left.text }
          : { kind: 'empty', number: null, text: '' },
        new: right
          ? { kind: 'added', number: right.newNumber, text: right.text }
          : { kind: 'empty', number: null, text: '' },
      })
    }
  }
  return pairs
}
