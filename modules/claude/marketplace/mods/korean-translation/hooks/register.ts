import type { EngineInterface, Register } from 'claude-code'
import { type FailureCode, MARKER, MODEL, needsTranslation, TranslationError, TranslationRun } from './translate'

type Phase = 'pending' | 'translating' | 'ready' | 'failed' | 'cancelled'
type Row = {
  id: string
  direction: 'input' | 'output'
  phase: Phase
  text?: string
  run?: TranslationRun
  error?: FailureCode
  abort: AbortController
}
type State = {
  supported: boolean
  enabled: boolean
  generation: number
  session: number
  input?: Row
  requests: Set<string>
  turns: Map<string, boolean>
  rows: Map<string, Row>
  queue: Row[]
  draining: boolean
  lastFailure?: string
  ticker?: { cancel: () => void }
}

const USAGE = 'Usage: /translate on|off|status'
const DROP_HINT = 'Run /translate off to send the prompt without translation.'
const MAX_ROWS = 128
const MAX_PENDING = 4
const requestTag = (id: string) => `[korean-translation-request:${id}]`
const failureCode = (error: unknown): FailureCode => error instanceof TranslationError ? error.code : 'unexpected'
const diagnostic = (row: Row) => `${row.direction}: ${row.phase}${row.error ? ` [${row.error}]` : ''}; attempts: ${row.run?.attempts ?? 0}; elapsed: ${row.run?.elapsedMs ?? 0} ms`
const rowId = () => `translation-${crypto.randomUUID()}`
const isActive = (row: Row) => row.phase === 'pending' || row.phase === 'translating'
const FRAME_MS = 100
const GLOW = 4
const progress = (row: Row) => `Korean translation: ${row.phase}... ${((row.run?.elapsedMs ?? 0) / 1000).toFixed(1)}s${(row.run?.attempts ?? 0) > 1 ? ` (attempt ${row.run!.attempts})` : ''}`
// A bright window sweeps across the dim line; the frame comes from elapsed time, so each redraw advances it.
function shimmer(line: string, elapsedMs: number): [string, string, string] {
  const chars = [...line]
  const at = Math.floor(elapsedMs / FRAME_MS) % (chars.length + GLOW * 2) - GLOW
  const start = Math.max(0, at)
  const end = Math.max(0, at + GLOW)
  return [chars.slice(0, start).join(''), chars.slice(start, end).join(''), chars.slice(end).join('')]
}

async function translate($: EngineInterface, run: TranslationRun, signal: AbortSignal): Promise<string> {
  for (;;) {
    if (signal.aborted) return run.fail('deadline')
    const reply = await $.model.complete(run.request(), { signal }).catch(() => run.fail(signal.aborted ? 'deadline' : 'refused'))
    if (signal.aborted) return run.fail('deadline')
    if (!reply.isAnswered) return run.fail(reply.reason === 'api-error' ? 'provider' : reply.reason === 'aborted' ? 'deadline' : 'empty')
    const text = run.accept(reply.text, reply.usage.output_tokens)
    if (text !== undefined) return text
  }
}

// Redraws in-progress rows so their shimmer and elapsed time advance; stops once no row is in progress.
function animate($: EngineInterface, state: State) {
  state.ticker ??= $.clock.every(FRAME_MS, () => {
    if (![...state.rows.values()].some(isActive)) {
      state.ticker?.cancel()
      state.ticker = undefined
    }
    $.ui.invalidate('ui.render')
  })
}

function cancel(state: State) {
  state.generation++
  state.input?.abort.abort()
  state.input?.run?.cancel()
  for (const row of state.rows.values()) {
    if (row.phase === 'pending' || row.phase === 'translating') {
      row.phase = 'cancelled'
      row.abort.abort()
      row.run?.cancel()
    }
  }
  state.queue = []
  state.requests.clear()
  state.turns.clear()
}

// One worker per session; starting a new model turn does not cancel its predecessor's translation.
async function drain($: EngineInterface, state: State) {
  if (state.draining) return
  state.draining = true
  const generation = state.generation
  try {
    while (state.queue.length && generation === state.generation) {
      const row = state.queue.shift()!
      if (row.abort.signal.aborted) continue
      row.phase = 'translating'
      $.ui.invalidate('ui.render')
      try {
        row.text = await translate($, row.run!, row.abort.signal)
        if (generation !== state.generation || row.abort.signal.aborted) continue
        row.phase = 'ready'
      } catch (error) {
        if (generation !== state.generation || row.abort.signal.aborted) continue
        row.phase = 'failed'
        row.error = failureCode(error)
        state.lastFailure = diagnostic(row)
        $.ui.log(`Korean translation failed. ${state.lastFailure}. The English answer is unchanged.`)
      }
      $.ui.invalidate('ui.render')
    }
  } finally {
    state.draining = false
    if (state.queue.length) await drain($, state)
  }
}

export const register: Register = on => {
  const state: State = { supported: false, enabled: true, generation: 0, session: 0, requests: new Set(), turns: new Map(), rows: new Map(), queue: [], draining: false }

  const status = () => [
    `Korean translation: ${state.enabled ? 'on' : 'off'}`,
    `model: ${MODEL}`,
    'scope: typed prompts and final answers in the interactive terminal session',
    `pending: ${state.input || [...state.rows.values()].some(row => row.phase === 'pending' || row.phase === 'translating') ? 'yes' : 'no'}`,
    ...(state.input ? [diagnostic(state.input)] : []),
    ...[...state.rows.values()].filter(row => row.phase === 'pending' || row.phase === 'translating').map(diagnostic),
    ...(state.lastFailure ? [`last failure: ${state.lastFailure}`] : []),
  ].join('\n')

  on('session.start', async ($, e, next) => {
    state.supported = e.isInteractive && e.surface === 'terminal'
    if (state.supported) {
      await $.command.register({ name: 'translate', description: 'Turn Korean conversation translation on or off', argumentHint: 'on|off|status', immediate: true })
      $.ui.log(`Korean translation: on (${MODEL}). /translate off disables it for this session.`)
    }
    return next(e)
  })

  on('command.run', { command: 'translate' }, ($, e) => {
    const arg = e.args.trim()
    if (arg === 'on') state.enabled = true
    else if (arg === 'off') {
      state.enabled = false
      cancel(state)
      $.ui.invalidate('ui.render')
    } else if (arg !== '' && arg !== 'status') return { text: USAGE }
    return { text: status() }
  })

  on('prompt.submit', async ($, e, next) => {
    if (!state.supported || !state.enabled || e.origin.kind !== 'composer' || e.command || e.isMeta) return next(e)
    if (state.requests.size >= MAX_ROWS || state.rows.size >= MAX_ROWS) return { drop: `Korean translation reached its session limit. ${DROP_HINT}` }
    const id = rowId()
    const generation = state.generation
    const session = state.session
    const context = [...(e.context ?? []), `${MARKER}\n${requestTag(id)}`]
    let text = e.text
    if (needsTranslation(text, 'English')) {
      if (state.input) return { drop: 'An input translation is already running. Wait or cancel it before submitting again.' }
      const row: Row = { id, direction: 'input', phase: 'translating', abort: new AbortController() }
      const abort = () => row.abort.abort()
      next.signal.addEventListener('abort', abort, { once: true })
      if (next.signal.aborted) abort()
      state.input = row
      $.ui.status('Translating to English... (esc cancels)')
      try {
        row.run = new TranslationRun(text, 'English')
        text = await translate($, row.run, row.abort.signal)
        if (generation !== state.generation || row.abort.signal.aborted) throw new TranslationError('deadline')
        row.text = e.text
        row.phase = 'ready'
        state.rows.set(id, row)
        const placed = await $.ui.place({ id })
        if (!placed.isPlaced) throw new TranslationError('unexpected')
      } catch (error) {
        state.rows.delete(id)
        row.phase = row.abort.signal.aborted ? 'cancelled' : 'failed'
        row.error = failureCode(error)
        if (generation === state.generation) {
          state.lastFailure = diagnostic(row)
          if ((await $.prompt.read()).text === '') await $.prompt.fill({ text: e.text })
        } else if (session === state.session && !state.enabled && (await $.prompt.read()).text === '') {
          // Disabling in this session still lets the person recover their draft.
          await $.prompt.fill({ text: e.text })
        }
        return { drop: `Korean translation failed [${row.error}]. ${DROP_HINT}` }
      } finally {
        next.signal.removeEventListener('abort', abort)
        if (state.input === row) {
          state.input = undefined
          $.ui.status(undefined)
        }
      }
    }
    if (generation !== state.generation || next.signal.aborted) return { drop: `Korean translation was cancelled. ${DROP_HINT}` }
    state.requests.add(id)
    const result = await next({ ...e, text, context })
    if (result.drop !== undefined) state.requests.delete(id)
    return result
  }).catch(async ($, e, next) => {
    if (next.called) return next(e)
    if (!(state.supported && e.origin.kind === 'composer' && !e.command && !e.isMeta && needsTranslation(e.text, 'English'))) return next(e)
    try {
      if ((await $.prompt.read()).text === '') await $.prompt.fill({ text: e.text })
    } catch {
      // Refusing submission is still possible when restoring the editor is not.
    }
    return { drop: `Korean translation failed [unexpected]. ${DROP_HINT}` }
  })

  on('turn.start', async ($, e, next) => {
    if (!state.supported || e.agentId !== undefined) return next(e)
    const generation = state.generation
    let eligible = false
    if (state.enabled && e.text) {
      // The target API exposes the conversation, not a last-row filter. Inspect only its latest user message.
      const messages = await $.session.messages({ as: 'api' })
      const latest = [...messages].reverse().find(message => message.role === 'user')
      const text = latest?.content.filter(block => block.type === 'text').map(block => block.text).join('\n') ?? ''
      const ids = [...text.matchAll(/\[korean-translation-request:(translation-[a-z0-9-]+)\]/g)]
      const id = ids.at(-1)?.[1]
      if (id && state.requests.has(id)) {
        eligible = true
        state.requests.delete(id)
      }
    }
    if (generation === state.generation) state.turns.set(e.turnId, eligible)
    return next(e)
  }).catch(($, e, next) => {
    state.turns.set(e.turnId, false)
    $.ui.log('Korean translation could not identify this turn [turn-context]. Its answer will not be translated.')
    return next(e)
  })

  on('turn.complete', async ($, e, next) => {
    const result = await next(e)
    if (e.agentId !== undefined) return result
    const eligible = state.turns.get(e.turnId)
    state.turns.delete(e.turnId)
    if (e.isAborted) {
      cancel(state)
      $.ui.invalidate('ui.render')
      return result
    }
    if (!eligible || !state.supported || !state.enabled || e.reason !== 'answer' || !needsTranslation(e.answer, 'Korean')) return result
    const pending = [...state.rows.values()].filter(row => row.phase === 'pending' || row.phase === 'translating').length
    if (pending >= MAX_PENDING || state.rows.size >= MAX_ROWS) {
      $.ui.log('Korean translation skipped [capacity]. The English answer is unchanged.')
      return result
    }
    const row: Row = { id: rowId(), direction: 'output', phase: 'pending', abort: new AbortController() }
    const generation = state.generation
    try {
      row.run = new TranslationRun(e.answer, 'Korean')
      state.rows.set(row.id, row)
      const placed = await $.ui.place({ id: row.id })
      if (!placed.isPlaced) throw new TranslationError('unexpected')
      if (generation !== state.generation || row.abort.signal.aborted) return result
      state.queue.push(row)
      animate($, state)
      void drain($, state).catch(error => {
        if (generation !== state.generation || row.abort.signal.aborted) return
        row.phase = 'failed'
        row.error = failureCode(error)
        state.lastFailure = diagnostic(row)
        $.ui.log(`Korean translation failed. ${state.lastFailure}. The English answer is unchanged.`)
      })
    } catch (error) {
      row.phase = 'failed'
      row.error = failureCode(error)
      state.lastFailure = diagnostic(row)
      $.ui.log(`Korean translation failed. ${state.lastFailure}. The English answer is unchanged.`)
    }
    return result
  })

  on('ui.render', { component: 'Placed' }, ($, e, next) => {
    if (e.props.plugin !== $.plugin.name) return next(e)
    const row = state.rows.get(e.props.id)
    if (!row) return next(e)
    const { Box, Markdown, Text } = $.ui.resolve(e)
    if (row.phase === 'ready') {
      if (row.direction === 'input') return Markdown({ text: `**입력 원문**\n\n${row.text}` })
      return Box({ flexDirection: 'column', marginTop: 1, children: [
        Text({ dimColor: true, children: ['한국어 번역'] }),
        Box({ key: 'translation-output-row', flexDirection: 'row', children: [
          Box({ width: 2, flexShrink: 0, children: [Text({ children: ['⏺'] })] }),
          Box({ flexDirection: 'column', flexGrow: 1, flexShrink: 1, children: [Markdown({ key: 'translation-body', text: row.text! })] }),
        ] }),
      ] })
    }
    if (isActive(row)) {
      const [before, glow, after] = shimmer(progress(row), row.run?.elapsedMs ?? 0)
      return Box({ marginTop: 1, children: [Text({ children: [
        Text({ dimColor: true, children: [before] }),
        Text({ weight: 'bold', children: [glow] }),
        Text({ dimColor: true, children: [after] }),
      ] })] })
    }
    return Box({ marginTop: 1, children: [Text({ children: [`Korean translation: ${diagnostic(row)}. The English answer is unchanged.`] })] })
  })

  on('session.end', ($, e, next) => {
    cancel(state)
    state.session++
    state.input = undefined
    state.rows.clear()
    state.lastFailure = undefined
    if (!state.enabled && (e.reason === 'clear' || e.reason === 'resume')) {
      state.enabled = true
      $.ui.log('Korean translation: on again for the new session.')
    }
    return next(e)
  })
}
