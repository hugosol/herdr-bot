// Public S2 subscription and S3 native widget/confirmation. No editor replacement,
// input interception, fake session state or synthetic user input.
import { appendFileSync, existsSync } from 'node:fs'
import { setTimeout as delay } from 'node:timers/promises'
import { ompAdapter } from './adapters/omp/index.ts'
import { piAdapter } from './adapters/pi/index.ts'
import { defineExtension, defineImplementation } from './framework/interface/index.ts'
import { toolConfirmation } from './framework/runtime/tool-confirmation.ts'

const env = process.env
const isOmp = env.PAIRED_AGENT === 'omp'
const title = 'Paired Task startup modal'
const question = 'Close this test confirmation without answering the Task.'
const definition = defineExtension({ id: 'paired-task-launch-probe', displayName: 'Paired Task launch probe' })

export default function probe(api) {
  let context
  let startupSession
  const record = (kind, fields = {}) => appendFileSync(env.PAIRED_TRACE, `${JSON.stringify({
    kind, pid: process.pid, unixMs: Date.now(), ...fields,
  })}\n`)
  const idOf = context => context.sessionManager.getSessionId()
  const pause = async () => {
    const deadline = Date.now() + 18000
    while (!existsSync(env.PAIRED_RELEASE)) {
      if (Date.now() > deadline) throw new Error('Paired Task acceptance did not release startup')
      await delay(20)
    }
    record('native-hook-released', { sessionId: startupSession })
  }
  api.on('session_start', async (_event, ctx) => {
    context = ctx
    const sessionId = idOf(ctx)
    record('native-session-start', { sessionId, cwd: ctx.cwd, file: ctx.sessionManager.getSessionFile(),
      model: ctx.model?.id, provider: ctx.model?.provider, thinking: isOmp ? api.getThinkingLevel?.() : ctx.thinkingLevel,
      extensionsHaveUI: ctx.hasUI, envLaunch: env.HERDR_BOT_LAUNCH_ID, envTarget: env.HERDR_BOT_LAUNCH_TARGET_ID })
    if (startupSession !== undefined) return
    startupSession = sessionId
    if (env.PAIRED_HOLD !== '1') return
    if (isOmp) {
      // Read the same native focus as a regular widget, without taking focus.
      ctx.ui.setWidget('paired-gate-observation', tui => {
        const disabled = tui.getFocused()?.disableSubmit
        record('omp-native-gate', { sessionId, disabled })
        return { render: () => [], invalidate() {} }
      })
      ctx.ui.setWidget('paired-gate-observation', undefined)
    } else {
      record('pi-native-initializing', { sessionId })
    }
    await pause()
  })
  api.on('session_switch', (_event, ctx) => {
    context = ctx
    record('native-session-switch', { sessionId: idOf(ctx), file: ctx.sessionManager.getSessionFile() })
  })
  if (env.PAIRED_HISTORY_HOLD === '1') {
    api.on('before_agent_start', async (_event, ctx) => {
      record('native-input-pending', { sessionId: idOf(ctx), file: ctx.sessionManager.getSessionFile() })
      const deadline = Date.now() + 18000
      while (!existsSync(env.PAIRED_HISTORY_RELEASE)) {
        if (Date.now() > deadline) throw new Error('Paired Task acceptance did not release native round')
        await delay(20)
      }
    })
  }
  const implementation = defineImplementation({ create: () => ({}), subscriptions: [{ scope: 'owner', events: {
    session_start: (_state, { session, owner }) => record('session_start', { sessionId: session.sessionId, owner: owner.key }),
    session_end: (_state, { session, owner }) => record('session_end', { sessionId: session.sessionId, owner: owner.key }),
    session_ready: (_state, { session, owner }) => {
      const sessionId = session.sessionId
      record('session_ready', { sessionId, owner: owner.key })
      if (!env.PAIRED_MODAL || sessionId !== startupSession) return
      setTimeout(async () => {
        try {
          const deadline = Date.now() + 30_000
          while (!existsSync(env.PAIRED_MODAL_RELEASE)) {
            if (Date.now() > deadline) throw new Error('Paired Task acceptance did not reach input initialization')
            await delay(20)
          }
          if (!context?.hasUI || idOf(context) !== sessionId) throw new Error('No matching interactive native owner')
          const pending = env.PAIRED_MODAL === 'tui' ? toolConfirmation((title, message, options) => {
            // Like the shipped Pi tool adapter, announce only after the native selector is installed.
            const answer = context.ui.confirm(title, message, options)
            options.onTimeoutStart()
            return answer
          }, undefined, (active, label) => api.events.emit('herdr:blocked', { active, label }))({
            title, message: question, timeoutMs: 60_000, notifyOnWait: true,
          }) : context.ui.confirm(title, question)
          record('native-confirmation-opened', { sessionId, title, question })
          record('native-confirmation-result', { sessionId, result: await pending })
        } catch (error) {
          record('native-confirmation-error', { sessionId, error: String(error) })
        }
      }, 0)
    },
  } }] })
  if (isOmp) ompAdapter(definition, { implementation })(api)
  else piAdapter(definition, { implementation })(api)
}
