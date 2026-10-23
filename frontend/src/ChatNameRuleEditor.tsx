import {
  useEffect,
  useRef,
  type ClipboardEvent,
  type DragEvent,
  type KeyboardEvent,
} from 'react'

import { NAMING_FIELDS } from './chatNaming'

const TOKEN_PATTERN = /(\{(?:zone\.name|bot\.name|agent|chat\.id)\})/

function tokenText(field: string): string {
  return `{${field}}`
}

/** 从编辑器 DOM 还原规则；变量块按占位符序列化，删除按钮不进入规则。 */
function serializeNode(node: Node): string {
  if (node.nodeType === Node.TEXT_NODE) return node.textContent ?? ''
  if (node instanceof HTMLBRElement) return '\n'
  if (node instanceof HTMLElement) {
    const token = node.dataset.chatNameToken
    if (token !== undefined) return token
  }
  return Array.from(node.childNodes).map(serializeNode).join('')
}

/**
 * chat-name 命名规则编辑器。
 *
 * 固定文字在可编辑表面中自由修改；四个已承诺字段以不可拆分的行内变量块呈现，点击 × 或
 * 在变量块后按退格会整块删除。插入按钮使用当前光标位置，而不是永远追加到末尾。
 */
export function ChatNameRuleEditor({
  value,
  onChange,
  disabled = false,
}: {
  value: string
  onChange: (value: string) => void
  disabled?: boolean
}) {
  const editor = useRef<HTMLDivElement>(null)
  const onChangeRef = useRef(onChange)
  const disabledRef = useRef(disabled)
  onChangeRef.current = onChange
  disabledRef.current = disabled

  function readEditor(): string {
    const root = editor.current
    if (root === null) return ''
    const value = Array.from(root.childNodes)
      .map(serializeNode)
      .join('')
      .replace(/\r\n?/g, '\n')
      .replace(/\u00a0/g, ' ')
    // 清空 contenteditable 时浏览器可能留下 <br> 或空白节点；规则语义仍是空字符串。
    return /^\s*$/.test(value) ? '' : value
  }

  function emit(): void {
    if (disabledRef.current) return
    onChangeRef.current(readEditor())
  }

  function tokenNode(field: string): HTMLSpanElement {
    const token = document.createElement('span')
    token.className = 'chat-name-token'
    token.dataset.chatNameToken = tokenText(field)
    token.dataset.testid = 'chat-name-token'
    token.contentEditable = 'false'
    token.spellcheck = false
    token.append(document.createTextNode(tokenText(field)))

    const remove = document.createElement('button')
    remove.type = 'button'
    remove.className = 'chat-name-token__remove'
    remove.textContent = '×'
    remove.setAttribute('aria-label', `删除变量 ${field}`)
    remove.addEventListener('mousedown', (event) => event.preventDefault())
    remove.addEventListener('click', (event) => {
      event.preventDefault()
      event.stopPropagation()
      if (disabledRef.current) return
      const root = editor.current
      if (root === null) return

      const next = token.nextSibling
      const previous = token.previousSibling
      token.remove()

      const range = document.createRange()
      if (next !== null) {
        range.setStart(next, 0)
      } else if (previous !== null && previous.nodeType === Node.TEXT_NODE) {
        range.setStart(previous, previous.textContent?.length ?? 0)
      } else if (previous !== null) {
        range.setStartAfter(previous)
      } else {
        range.selectNodeContents(root)
        range.collapse(true)
      }
      range.collapse(true)
      const selection = window.getSelection()
      selection?.removeAllRanges()
      selection?.addRange(range)
      root.focus()
      emit()
    })
    token.append(remove)
    return token
  }

  function renderValue(nextValue: string): void {
    const root = editor.current
    if (root === null) return
    root.replaceChildren()
    for (const part of nextValue.split(TOKEN_PATTERN)) {
      if (part === '') continue
      if (TOKEN_PATTERN.test(part)) {
        root.append(tokenNode(part.slice(1, -1)))
      } else {
        root.append(document.createTextNode(part))
      }
    }
  }

  // 只在外部值确实不同于 DOM 时重建，避免 React 每次输入后覆盖用户的光标。
  useEffect(() => {
    const root = editor.current
    if (root === null) return
    if (value === '' && (root.textContent ?? '').trim() === '') {
      root.replaceChildren()
      return
    }
    if (readEditor() !== value) renderValue(value)
    // readEditor/renderValue 只读取当前 DOM 与 props；重建时机由 value 变化驱动。
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [value])

  function currentRange(root: HTMLDivElement): Range {
    const selection = window.getSelection()
    if (
      selection !== null &&
      selection.rangeCount > 0 &&
      root.contains(selection.getRangeAt(0).commonAncestorContainer)
    ) {
      return selection.getRangeAt(0)
    }
    const range = document.createRange()
    range.selectNodeContents(root)
    range.collapse(false)
    return range
  }

  function placeCaretAfter(node: Node, root: HTMLDivElement): void {
    const range = document.createRange()
    range.setStartAfter(node)
    range.collapse(true)
    const selection = window.getSelection()
    selection?.removeAllRanges()
    selection?.addRange(range)
    root.focus()
  }

  function insertField(field: string): void {
    if (disabledRef.current) return
    const root = editor.current
    if (root === null) return
    const range = currentRange(root)
    root.focus()
    range.deleteContents()
    const token = tokenNode(field)
    range.insertNode(token)
    placeCaretAfter(token, root)
    emit()
  }

  function insertPlainText(text: string): void {
    if (disabledRef.current) return
    const root = editor.current
    if (root === null) return
    const normalized = text.replace(/[\r\n]+/g, ' ')
    const range = currentRange(root)
    root.focus()
    range.deleteContents()
    if (normalized === '') {
      emit()
      return
    }
    const node = document.createTextNode(normalized)
    range.insertNode(node)
    placeCaretAfter(node, root)
    emit()
  }

  /** 删除紧邻光标之前的变量块；普通文字仍由浏览器按退格逐字删除。 */
  function removeTokenBeforeCaret(): boolean {
    const root = editor.current
    const selection = window.getSelection()
    if (root === null || selection === null || selection.rangeCount === 0) return false

    const range = selection.getRangeAt(0)
    if (!range.collapsed || !root.contains(range.startContainer)) return false

    const container = range.startContainer
    const containerElement =
      container.nodeType === Node.ELEMENT_NODE
        ? (container as HTMLElement)
        : container.parentElement
    const containingToken = containerElement?.closest<HTMLElement>('[data-chat-name-token]')

    let candidate: Element | null
    if (containingToken !== null && containingToken !== undefined && root.contains(containingToken)) {
      candidate = containingToken
    } else if (container.nodeType === Node.ELEMENT_NODE) {
      let previous: ChildNode | null =
        (container as HTMLElement).childNodes[range.startOffset - 1] ?? null
      while (previous !== null && previous.nodeType === Node.TEXT_NODE && previous.textContent === '') {
        previous = previous.previousSibling
      }
      candidate = previous instanceof HTMLElement ? previous : null
    } else if (range.startOffset === 0) {
      let previous = container.previousSibling
      while (previous !== null && previous.nodeType === Node.TEXT_NODE && previous.textContent === '') {
        previous = previous.previousSibling
      }
      candidate = previous instanceof HTMLElement ? previous : null
    } else {
      return false
    }

    if (!(candidate instanceof HTMLElement) || candidate.dataset.chatNameToken === undefined) {
      return false
    }

    const next = candidate.nextSibling
    const previous = candidate.previousSibling
    candidate.remove()

    const nextRange = document.createRange()
    if (next !== null) {
      nextRange.setStart(next, 0)
    } else if (previous !== null && previous.nodeType === Node.TEXT_NODE) {
      nextRange.setStart(previous, previous.textContent?.length ?? 0)
    } else if (previous !== null) {
      nextRange.setStartAfter(previous)
    } else {
      nextRange.selectNodeContents(root)
      nextRange.collapse(true)
    }
    nextRange.collapse(true)
    selection.removeAllRanges()
    selection.addRange(nextRange)
    root.focus()
    return true
  }

  function onKeyDown(event: KeyboardEvent<HTMLDivElement>): void {
    // 变量里的 × 是独立按钮；它获得焦点时让原生 Enter/Space 激活，不被编辑器的单行拦截吞掉。
    if (
      event.target instanceof Element &&
      event.target.closest('.chat-name-token__remove') !== null
    ) {
      return
    }
    if (event.key === 'Enter') {
      event.preventDefault()
      return
    }
    if (event.key === 'Backspace' && removeTokenBeforeCaret()) {
      event.preventDefault()
      emit()
    }
  }

  function onPaste(event: ClipboardEvent<HTMLDivElement>): void {
    event.preventDefault()
    insertPlainText(event.clipboardData.getData('text/plain'))
  }

  function onDrop(event: DragEvent<HTMLDivElement>): void {
    event.preventDefault()
    insertPlainText(event.dataTransfer.getData('text/plain'))
  }

  return (
    <div className="chat-name-editor">
      <label id="bot-chat-name-label" className="zone-form__label">
        单行 chat-name 命名规则
      </label>
      <div
        ref={editor}
        id="bot-chat-name"
        className="chat-name-editor__surface"
        data-testid="bot-chat-name"
        data-placeholder="输入固定文字，或插入下方变量"
        role="textbox"
        aria-labelledby="bot-chat-name-label"
        aria-describedby="bot-chat-name-hint"
        aria-multiline="false"
        aria-disabled={disabled}
        contentEditable={!disabled}
        suppressContentEditableWarning
        spellCheck={false}
        tabIndex={disabled ? -1 : 0}
        onInput={emit}
        onKeyDown={onKeyDown}
        onPaste={onPaste}
        onDrop={onDrop}
      />

      <p id="bot-chat-name-hint" className="bot-hint">
        文字可直接编辑；点击变量旁的 ×，或把光标放在变量后按退格，可整块删除。先定位光标再插入变量。
      </p>

      <fieldset className="bot-fields">
        <legend>插入命名字段</legend>
        <div className="bot-field-buttons">
          {NAMING_FIELDS.map((field) => (
            <button
              key={field.key}
              type="button"
              data-testid={`insert-${field.key}`}
              disabled={disabled}
              aria-controls="bot-chat-name"
              onMouseDown={(event) => event.preventDefault()}
              onClick={() => insertField(field.token.slice(1, -1))}
            >
              ＋ {field.token}
            </button>
          ))}
        </div>
      </fieldset>
    </div>
  )
}
