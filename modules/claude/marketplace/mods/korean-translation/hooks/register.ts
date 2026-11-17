import type { EngineInterface, Register } from 'claude-code'
import {
  byteLength,
  DEADLINE_MS,
  hasHangul,
  type Language,
  MARKER,
  MAX_BYTES,
  MAX_TOKENS,
  MODEL,
  needsTranslation,
  protect,
  systemPrompt,
} from './translate'

// Messages thrown here are shown to the user, so they never carry prompt text or provider payloads.
class TranslationError extends Error {}

const USAGE = 'Usage: /translate on|off|status'
const DROP_HINT = 'Run /translate off to send the prompt without translation.'

async function translate($: EngineInterface, source: string, language: Language, signal: AbortSignal): Promise<string> {
  if (byteLength(source) > MAX_BYTES) throw new TranslationError('the text exceeds 64 KiB')
  const guarded = protect(source)
  const deadline = Date.now() + DEADLINE_MS
  for (let isRetry = false; ; isRetry = true) {
    const timeoutMs = deadline - Date.now()
    if (signal.aborted || timeoutMs <= 0) throw new TranslationError('it was cancelled or timed out')
    const reply = await $.model
      .complete(
        { model: MODEL, system: systemPrompt(language, isRetry), prompt: guarded.text, maxTokens: MAX_TOKENS, effort: 'low', timeoutMs },
        { signal },
      )
      .catch(() => {
        // The engine refused to send the request (a blocked model, organization policy); its text is not shown.
        throw new TranslationError(`the ${MODEL} request was refused`)
      })
    if (!reply.isAnswered) {
      if (reply.reason === 'api-error') throw new TranslationError(`${MODEL} returned an API error (${reply.error})`)
      throw new TranslationError(reply.reason === 'aborted' ? 'it was cancelled or timed out' : `${MODEL} returned no text`)
    }
    // The result carries no stop reason, so a reply that used the whole token limit is treated as truncated.
    if (reply.usage.output_tokens >= MAX_TOKENS) throw new TranslationError('the translation exceeded the output limit')
    if (!reply.text.trim() || byteLength(reply.text) > MAX_BYTES * 4) throw new TranslationError(`${MODEL} returned a malformed reply`)
    const restored = guarded.restore(reply.text.trim())
    if (restored !== undefined) return restored
    if (isRetry) throw new TranslationError('the translation changed protected code, paths, or links')
  }
}

const reasonOf = (error: unknown): string => (error instanceof TranslationError ? error.message : 'of an unexpected error')

const notice = (text: string) => ({ message: { type: 'system' as const, content: [{ type: 'text' as const, text }] } })

export const register: Register = on => {
  // Module state is memory-only: a reload or a replaced session starts enabled again.
  let isSupported = false
  let isEnabled = true
  let generation = 0
  // Whether the latest main-loop prompt carried MARKER, so its answer is English-only and needs the display translation.
  // shortcut: an automated prompt delivered into a marked turn clears this and that turn's answer is left untranslated; key it by turn id if that happens in practice.
  let isMarked = false
  let isInputPending = false
  let output: AbortController | undefined
  let lastTurnId: string | undefined

  const invalidate = () => {
    generation++
    output?.abort()
    output = undefined
  }

  const status = () =>
    [
      `Korean translation: ${isEnabled ? 'on' : 'off'}`,
      `model: ${MODEL}`,
      'scope: typed prompts and final answers in the interactive terminal session',
      `pending: ${isInputPending || output ? 'yes' : 'no'}`,
    ].join('\n')

  on('session.start', async ($, e, next) => {
    isSupported = e.isInteractive && e.surface === 'terminal'
    if (isSupported) {
      await $.command.register({
        name: 'translate',
        description: 'Turn Korean conversation translation on or off',
        argumentHint: 'on|off|status',
        immediate: true,
      })
      await $.ui.log(`Korean translation: on (${MODEL}). /translate off disables it for this session.`)
    }
    return next(e)
  })

  on('command.run', { command: 'translate' }, (_, e) => {
    const arg = e.args.trim()
    if (arg === 'on') isEnabled = true
    else if (arg === 'off') {
      isEnabled = false
      invalidate()
    } else if (arg !== '' && arg !== 'status') return { text: USAGE }
    return { text: status() }
  })

  on('prompt.submit', async ($, e, next) => {
    if (!isSupported || e.origin.kind !== 'composer') {
      // A prompt delivered into a running turn (turnId set) leaves that turn's marking alone.
      if (e.turnId === undefined) isMarked = false
      return next(e)
    }
    isMarked = isEnabled
    if (!isEnabled) return next(e)
    const context = [...(e.context ?? []), MARKER]
    // The engine does not apply a text rewrite to slash commands or hidden prompts; they pass through unchanged.
    if (e.command || e.isMeta || !needsTranslation(e.text, 'English')) return next({ ...e, context })

    const mine = generation
    isInputPending = true
    await $.ui.status('Translating to English... (esc cancels)')
    let text: string
    try {
      text = await translate($, e.text, 'English', next.signal)
      if (mine !== generation || !isEnabled || next.signal.aborted) throw new TranslationError('it was cancelled')
    } catch (error) {
      // Put the Korean draft back unless the person has already typed something else.
      if ((await $.prompt.read()).text === '') await $.prompt.fill({ text: e.text })
      return { drop: `Korean translation failed because ${reasonOf(error)}. ${DROP_HINT}` }
    } finally {
      isInputPending = false
      await $.ui.status(undefined)
    }
    const result = await next({ ...e, text, context })
    // Shown only once the English prompt has entered; a refused notice changes nothing the model reads.
    if (result.drop === undefined) await $.session.append(notice(`Korean original:\n${e.text}`)).catch(() => undefined)
    return result
  }).catch(async ($, e, next) => {
    if (next.called) return next(e)
    // Fail closed only for prompts this hook would have translated.
    if (!(isSupported && isEnabled && e.origin.kind === 'composer' && !e.command && !e.isMeta && hasHangul(e.text))) return next(e)
    try {
      if ((await $.prompt.read()).text === '') await $.prompt.fill({ text: e.text })
    } catch {
      // The drop below matters more than the draft: a throw here would let the untranslated prompt through.
    }
    return { drop: `Korean translation failed. ${DROP_HINT}` }
  })

  // Any new main-loop turn, typed, queued, or automated, cancels the preceding turn's unfinished output translation.
  on('turn.start', (_, e, next) => {
    if (e.agentId === undefined) {
      output?.abort()
      output = undefined
    }
    return next(e)
  })

  on('turn.complete', async ($, e, next) => {
    const result = await next(e)
    if (e.agentId !== undefined || e.reason !== 'answer' || e.isAborted) return result
    if (!isSupported || !isEnabled || !isMarked || e.turnId === lastTurnId) return result
    lastTurnId = e.turnId
    if (!needsTranslation(e.answer, 'Korean')) return result

    output?.abort()
    const mine = generation
    const abort = (output = new AbortController())
    const isStale = () => mine !== generation || abort.signal.aborted || !isEnabled
    // Not awaited: the turn ends at once and the translation lands as a display-only notice the model never reads.
    void translate($, e.answer, 'Korean', abort.signal)
      .then(
        async text => {
          if (isStale()) return
          const appended = await $.session.append(notice(`한국어 번역\n\n${text}`))
          if ('deny' in appended) await $.ui.log('Korean translation could not be shown. The English answer is unchanged.')
        },
        error => (isStale() ? undefined : $.ui.log(`Korean translation failed because ${reasonOf(error)}. The English answer is unchanged.`)),
      )
      .catch(() => undefined)
      .finally(() => {
        if (output === abort) output = undefined
      })
    return result
  })

  on('session.end', async ($, e, next) => {
    invalidate()
    isMarked = false
    lastTurnId = undefined
    // After /clear or a resume the process goes on with no session.start, so the opt-out resets here.
    if (!isEnabled && (e.reason === 'clear' || e.reason === 'resume')) {
      isEnabled = true
      await $.ui.log('Korean translation: on again for the new session.')
    }
    return next(e)
  })
}
