import Markdown from 'react-markdown'

/** React text nodes and the renderer's URL policy keep Map-authored Markdown inert. */
export function MapInputRequirements({ content }: { content: string }) {
  return <section aria-label="Map 输入要求" className="scheduler-details">
    <h3>Map 输入要求</h3>
    {content.trim() ? <div className="scheduler-input-markdown"><Markdown skipHtml>{content}</Markdown></div> : <p className="scheduler-help">此 Map 未声明输入要求。</p>}
  </section>
}
