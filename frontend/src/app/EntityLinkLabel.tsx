export const ENTITY_ICON_PATHS = {
  task: 'M4 4h16v16H4V4ZM10 8l6 4-6 4V8Z',
  bot: 'M5 7h14v13H5V7ZM12 3v4M8 12h1M15 12h1M9 16h6',
  map: 'M9 2h6v6H9V2ZM2 16h6v6H2v-6ZM16 16h6v6h-6v-6ZM12 8v4M5 16v-4h14v4',
} as const

export function EntityLinkLabel({ kind, name }: { kind: keyof typeof ENTITY_ICON_PATHS; name: string }) {
  return <span className="entity-link-label">
    <svg aria-hidden="true" focusable="false" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round"><path d={ENTITY_ICON_PATHS[kind]} /></svg>
    <span className="entity-link-label__name">{name}</span>
  </span>
}
