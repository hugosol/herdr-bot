import { useEffect, useState } from 'react'
import Markdown from 'react-markdown'
import type { BotRevision } from '../../controllers/botController'
import { groupOnlyRevision, loadMapRules, saveMapRules, type WorkflowMap } from '../../controllers/schedulerController'

type RuleText = { declared: boolean; description: string; files: string }
type Input = WorkflowMap['map_input_rule']
type Output = WorkflowMap['map_output_rule']

const sides = {
  input: {
    toggle: '声明 Map 输入规则', prose: 'Map 输入要求 Markdown', files: 'Map 文件输入声明 JSON',
    help: '输入文件项必须显式填写布尔 required，例如 {"id":"brief","required":true}。',
  },
  output: {
    toggle: '声明 Map 输出规则', prose: 'Map 输出说明 Markdown', files: 'Map 文件输出声明 JSON',
    help: '输出文件项例如 {"id":"result"}；声明即必交，不填写 required。只有说明、不交付文件时使用 []。',
  },
}

function ruleText(rule?: Input | Output): RuleText {
  return { declared: rule !== undefined, description: rule?.description ?? '', files: JSON.stringify(rule?.files ?? [], null, 2) }
}

function RuleFields({ side, value, disabled, onChange }: { side: keyof typeof sides; value: RuleText; disabled: boolean; onChange: (value: RuleText) => void }) {
  const labels = sides[side]
  return <fieldset className="scheduler-rule-fieldset" disabled={disabled}>
    <legend>{side === 'input' ? '输入规则' : '输出规则'}</legend>
    <label className="scheduler-rule-toggle"><input type="checkbox" checked={value.declared} onChange={event => onChange({ ...value, declared: event.target.checked })} />{labels.toggle}</label>
    <p>{labels.help}</p>
    <label>{labels.prose}<textarea value={value.description} disabled={!value.declared} onChange={event => onChange({ ...value, description: event.target.value })} /></label>
    <label>{labels.files}<textarea spellCheck={false} value={value.files} disabled={!value.declared} onChange={event => onChange({ ...value, files: event.target.value })} /></label>
  </fieldset>
}

function rulePayload(rule: RuleText): { description: string; files: unknown } | undefined {
  return rule.declared ? { description: rule.description, files: JSON.parse(rule.files) } : undefined
}

export function MapRulesEditor({ file, onClose, onSaved }: { file: string; onClose: () => void; onSaved: () => void }) {
  const [input, setInput] = useState(() => ruleText())
  const [output, setOutput] = useState(() => ruleText())
  const [revision, setRevision] = useState<BotRevision | null>(null)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  useEffect(() => {
    let cancelled = false
    void loadMapRules(file).then(draft => {
      if (!cancelled) {
        setInput(ruleText(draft.map_input_rule))
        setOutput(ruleText(draft.map_output_rule))
        setRevision(draft.revision)
      }
    }, failure => { if (!cancelled) setError(failure instanceof Error ? failure.message : '公开规则读取失败。') })
    return () => { cancelled = true }
  }, [file])
  useEffect(() => {
    function updated(event: Event) { const { kind, id, revision } = (event as CustomEvent<{ kind: string; id: string; revision: BotRevision }>).detail; if (kind === 'maps' && id === file) setRevision(previous => groupOnlyRevision(previous, revision) ? revision : previous) }
    window.addEventListener('herdr-group-revision', updated)
    return () => window.removeEventListener('herdr-group-revision', updated)
  }, [file])
  async function save() {
    if (!revision || busy) return
    setBusy(true); setError(null)
    try {
      await saveMapRules(file, { map_input_rule: rulePayload(input), map_output_rule: rulePayload(output), revision })
      onSaved()
    } catch (failure) { setError(failure instanceof Error ? failure.message : '公开规则保存失败。') }
    finally { setBusy(false) }
  }
  return <form className="scheduler-run-form" aria-label="Map 公开规则" onSubmit={event => { event.preventDefault(); void save() }}>
    <h3>Map 公开规则</h3>
    <p>两侧分别按 ID 声明文件，ID 可以跨侧同名。说明可写 Markdown；逐文件说明写在各项 description，不填写本次实际地址。未勾选的规则不保存。</p>
    <RuleFields side="input" value={input} disabled={!revision || busy} onChange={setInput} />
    <RuleFields side="output" value={output} disabled={!revision || busy} onChange={setOutput} />
    {error && <p role="alert">{error}</p>}
    <div className="scheduler-actions">
      <button type="submit" disabled={!revision || busy}>保存公开规则</button>
      <button type="button" disabled={busy} onClick={onClose}>放弃公开规则编辑</button>
    </div>
  </form>
}

export function MapRulesSummary({ input, output }: { input: Input; output: Output }) {
  if (!input && !output) return null
  return <div className="scheduler-map-rules">{([['Map 输入规则', input], ['Map 输出规则', output]] as const).map(([label, rule]) => rule &&
    <section key={label} aria-label={label} className="scheduler-details">
      <h4>{label}</h4>
      {rule.description?.trim() && <div className="scheduler-input-markdown"><Markdown skipHtml>{rule.description}</Markdown></div>}
      {rule.files?.length ? <ul>{rule.files.map(file => <li key={file.id} className="scheduler-text">
        <strong>{file.id}</strong>（{'required' in file && !file.required ? '可选' : '必需'}）{file.description && `：${file.description}`}
      </li>)}</ul> : <p className="scheduler-help">未声明文件。</p>}
    </section>)}</div>
}
