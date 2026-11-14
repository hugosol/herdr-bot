import type { TaskInput, TaskOptions } from './controller'

type Selection = Pick<TaskInput, 'agent' | 'model' | 'thinking'>

/** Shared by saved Task presets and temporary Chat launch choices. */
export function modelCompatibility(value: Selection, options: TaskOptions) {
  const thinkingOptions = value.model === 'default' ? [] : (options.thinking[value.agent]?.[value.model] ?? [])
  return {
    modelUnavailable: !(value.model in (options.thinking[value.agent] ?? {})),
    thinkingOptions,
    thinkingUnavailable: value.thinking !== 'default' && !thinkingOptions.includes(value.thinking),
  }
}

export function changeTaskModel(value: Selection, model: TaskInput['model'], options: TaskOptions) {
  const supported = model === 'default' ? [] : (options.thinking[value.agent]?.[model] ?? [])
  const retained = value.thinking === 'default' || supported.includes(value.thinking)
  return {
    model,
    thinking: retained ? value.thinking : 'default',
    notice: retained ? null : `当前思考级别「${value.thinking}」不适用于 ${value.agent} / ${model}，已重置为「沿用 Agent 默认值」。`,
  }
}

export function TaskModelControls({ value, options, disabled, prefix = 'task', onModel, onThinking }: {
  value: Selection
  options: TaskOptions
  disabled: boolean
  prefix?: string
  onModel: (model: TaskInput['model']) => void
  onThinking: (thinking: TaskInput['thinking']) => void
}) {
  const { modelUnavailable, thinkingOptions, thinkingUnavailable } = modelCompatibility(value, options)
  return <>
    <label className="zone-form__label" htmlFor={`${prefix}-model`}>model</label>
    <select id={`${prefix}-model`} className="zone-form__input zone-form__select" data-testid={`${prefix}-model`}
      value={value.model} onChange={event => onModel(event.target.value)} disabled={disabled}>
      {modelUnavailable && <option value={value.model} disabled>{value.model}（不可用）</option>}
      {options.models.filter(model => model in (options.thinking[value.agent] ?? {})).map(model => (
        <option key={model} value={model}>
          {options.labels?.models?.[model] ?? (model === 'default' ? '沿用 Agent 默认值' : model)}
        </option>
      ))}
    </select>
    <label className="zone-form__label" htmlFor={`${prefix}-thinking`}>thinking</label>
    <select id={`${prefix}-thinking`} className="zone-form__input zone-form__select" data-testid={`${prefix}-thinking`}
      value={value.thinking} onChange={event => onThinking(event.target.value)} disabled={disabled}>
      <option value="default">沿用 Agent 默认值</option>
      {thinkingUnavailable && <option value={value.thinking} disabled>{value.thinking}（不可用）</option>}
      {thinkingOptions.map(thinking => <option key={thinking} value={thinking}>{thinking}</option>)}
    </select>
  </>
}
