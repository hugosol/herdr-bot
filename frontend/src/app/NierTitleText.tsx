import type { CSSProperties } from 'react'

/** Keep the full heading available to assistive technology while its letters enter. */
export function NierTitleText({ children }: { children: string }) {
  const letters = Array.from(children)
  const stagger = Math.min(35, 400 / Math.max(1, letters.length))
  return <>
    <span className="nier-title-readable">{children}</span>
    <span key={children} aria-hidden="true">{letters.map((letter, index) =>
      <span key={index} className="nier-title-letter" style={{ '--nier-letter-delay': `${index * stagger}ms` } as CSSProperties}>{letter}</span>,
    )}</span>
  </>
}
