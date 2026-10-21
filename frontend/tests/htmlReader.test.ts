import { describe, expect, it } from 'vitest'

import {
  buildHtmlReaderDocument,
  classifyHtmlReaderLink,
  isHtmlPath,
  readerAssetUrl,
} from '../src/htmlReader'

const ZONE = 'C:\\work\\project'
const ORIGIN = 'http://localhost:8787'
const PAGE_URL = readerAssetUrl(ZONE, 'lessons/page.html', ORIGIN)

describe('HTML 阅读模式文档', () => {
  it('构造 zone 原始资源地址并保留相对目录结构', () => {
    expect(readerAssetUrl(ZONE, 'lessons/page one.html', ORIGIN)).toBe(
      'http://localhost:8787/api/zones/reader/433a5c776f726b5c70726f6a656374/lessons/page%20one.html',
    )
  })

  it('移除可执行、嵌入、表单与外部资源，只保留 zone 内的相对资源', () => {
    const source = `<!doctype html>
<html><head>
  <meta http-equiv="refresh" content="0;url=https://outside.example">
  <link rel="stylesheet" href="https://outside.example/theme.css">
  <link rel="stylesheet" href="../assets/style.css">
</head><body onload="window.__pwned=1">
  <script>window.__pwned=1</script>
  <iframe src="https://outside.example"></iframe>
  <form action="https://outside.example"><input name="x"></form>
  <img id="local" src="../assets/pixel.png">
  <img id="external" src="https://outside.example/pixel.png">
  <a id="external-link" href="https://example.com/article">external</a>
  <a id="file-link" href="file:///C:/secret.html">file</a>
  <a id="anchor" href="#section">anchor</a>
  <div onclick="window.__pwned=1" style="background-image:url(https://outside.example/a.png)">text</div>
</body></html>`

    const output = buildHtmlReaderDocument(source, PAGE_URL)
    const document = new DOMParser().parseFromString(output, 'text/html')

    expect(document.querySelector('script')).toBeNull()
    expect(document.querySelector('iframe')).toBeNull()
    expect(document.querySelector('form')).toBeNull()
    expect(document.querySelector('meta[http-equiv="refresh"]')).toBeNull()
    expect(document.querySelector('[onload]')).toBeNull()
    expect(document.querySelector('[onclick]')).toBeNull()
    expect(document.querySelector('#local')?.getAttribute('src')).toBe(
      '../assets/pixel.png',
    )
    expect(document.querySelector('#external')?.hasAttribute('src')).toBe(false)
    expect(document.querySelector('link[href^="https://"]')).toBeNull()
    expect(document.querySelector('link[href="../assets/style.css"]')).not.toBeNull()
    expect(document.querySelector('#external-link')?.getAttribute('href')).toBe(
      'https://example.com/article',
    )
    expect(document.querySelector('#file-link')?.hasAttribute('href')).toBe(false)
    expect(document.querySelector('#anchor')?.getAttribute('href')).toBe('#section')
    expect(document.querySelector('base')?.getAttribute('href')).toBe(PAGE_URL)
    expect(
      document.querySelector('meta[http-equiv="Content-Security-Policy"]')?.getAttribute(
        'content',
      ),
    ).toContain("script-src 'none'")
  })

  it('按大小写不敏感后缀识别 HTML 文件', () => {
    expect(isHtmlPath('page.html')).toBe(true)
    expect(isHtmlPath('PAGE.HTM')).toBe(true)
    expect(isHtmlPath('page.html?x=1')).toBe(true)
    expect(isHtmlPath('page.txt')).toBe(false)
    expect(isHtmlPath('html/page')).toBe(false)
  })
})

describe('HTML 阅读模式链接', () => {
  it('区分页内锚点、zone HTML、外部网页与需拦截的本地链接', () => {
    expect(classifyHtmlReaderLink('#section', PAGE_URL)).toEqual({ kind: 'anchor' })
    expect(classifyHtmlReaderLink('0006-next.html', PAGE_URL)).toEqual({
      kind: 'zone',
      path: 'lessons/0006-next.html',
    })
    expect(classifyHtmlReaderLink('../homework/evidence.html', PAGE_URL)).toEqual({
      kind: 'zone',
      path: 'homework/evidence.html',
    })
    expect(classifyHtmlReaderLink('https://example.com/article', PAGE_URL)).toEqual({
      kind: 'external',
      url: 'https://example.com/article',
    })
    expect(classifyHtmlReaderLink('notes.txt', PAGE_URL).kind).toBe('blocked')
    expect(classifyHtmlReaderLink('file:///C:/secret.html', PAGE_URL).kind).toBe(
      'blocked',
    )
    expect(classifyHtmlReaderLink('mailto:person@example.com', PAGE_URL).kind).toBe(
      'blocked',
    )
  })
})
