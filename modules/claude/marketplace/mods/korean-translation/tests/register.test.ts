import { expect, mock, test } from 'claude-code/testing'
import type { On } from 'claude-code'

const TAG = '[korean-translation: active]'
const USAGE = { input_tokens: 1, output_tokens: 1, cache_read_input_tokens: 0, cache_creation_input_tokens: 0 }
const answered = (text: string, output_tokens = 1) => ({ value: { isAnswered: true, text, usage: { ...USAGE, output_tokens } } })
const failed = (fields: object) => ({ value: { isAnswered: false, usage: USAGE, ...fields } })

// Keeps placeholders in place: Hangul becomes "word" on the way in, the answer gains a Korean lead on the way out.
const translator = (e: any) =>
  answered(e.system.includes('into Korean') ? `번역 ${e.prompt}` : e.prompt.replace(/\p{Script=Hangul}+/gu, 'word'))

// The engine beneath the mod. `reply` answers each model call; `box` is what the prompt box holds.
function world(on: On, reply: (e: any, call: number) => unknown = translator) {
  const w = {
    reply,
    box: '',
    isReadDenied: false,
    model: [] as any[],
    logs: [] as string[],
    status: [] as (string | undefined)[],
    fills: [] as any[],
    submitted: [] as any[],
    registered: [] as any[],
    session: mock.session(on),
    notes: () => w.session.appended().map(row => (row.message.content[0] as any).text as string),
  }
  on('model.complete', async (_, e) => {
    w.model.push(e)
    return (await w.reply(e, w.model.length)) as any
  })
  on('ui.log', (_, e) => {
    w.logs.push(e.text)
    return { value: undefined }
  })
  on('ui.status', (_, e) => {
    w.status.push(e.text)
    return { value: undefined }
  })
  on('command.register', (_, e) => {
    w.registered.push(e)
    return { value: { command: e.name } }
  })
  on('prompt.read', () => (w.isReadDenied ? { deny: 'no prompt box' } : { value: { text: w.box, cursor: 0 } }))
  on('prompt.fill', (_, e) => {
    w.fills.push(e)
    return { isFilled: true }
  })
  on('session.start', (_, e) => ({ cwd: e.cwd }))
  on('session.end', (_, e) => ({ sessionId: e.sessionId }))
  on('prompt.submit', (_, e) => {
    w.submitted.push(e)
    return { text: e.text, context: e.context }
  })
  on('turn.start', (_, e) => ({ turnId: e.turnId }))
  on('turn.complete', (_, e) => ({ text: e.answer }))
  return w
}

const start = ($: any, over: object = {}) => $.session.start({ cwd: '/', surface: 'terminal', isInteractive: true, ...over })
const submit = ($: any, text: string, over: object = {}) => $.prompt.submit({ text, wait: false, origin: { kind: 'composer' }, ...over })
const turn = ($: any, over: object = {}) =>
  $.turn.complete({ answer: 'The build passed.', durationMs: 1, isAborted: false, turnId: 't1', reason: 'answer', ...over })
const command = ($: any, args: string) =>
  $.command.run({ command: 'translate', args, origin: { kind: 'composer' }, presentation: { isFullscreen: false, columns: 80 } })
const end = ($: any, reason: string) => $.session.end({ reason, sessionId: 's1', resume: { id: 's1' } })

// The output translation runs after its hook has returned, so a test waits for it in real time.
declare const setTimeout: (run: (value: unknown) => void, ms: number) => unknown
const settle = () => new Promise(resolve => setTimeout(resolve, 40))
function gate() {
  let open!: () => void
  const opened = new Promise<void>(resolve => (open = resolve))
  return { opened, open }
}

const KOREAN = '이 함수를 고쳐줘'
const STATUS_ON = 'Korean translation: on\nmodel: haiku\nscope: typed prompts and final answers in the interactive terminal session\npending: no'
const STATUS_OFF = STATUS_ON.replace(': on', ': off')

// ---- load and controls

test('loading in an interactive terminal registers /translate and announces the state without a model call', async ($, on) => {
  const w = world(on)
  await start($)
  expect(w.registered).toEqual([
    { name: 'translate', description: 'Turn Korean conversation translation on or off', argumentHint: 'on|off|status', immediate: true },
  ])
  expect(w.logs).toEqual(['Korean translation: on (haiku). /translate off disables it for this session.'])
  expect(w.model).toHaveLength(0)
})

test('/translate reports, switches and rejects without a model call or a working-model prompt', async ($, on) => {
  const w = world(on)
  await start($)
  expect(await command($, '')).toEqual({ text: STATUS_ON })
  expect(await command($, 'status')).toEqual({ text: STATUS_ON })
  expect(await command($, 'off')).toEqual({ text: STATUS_OFF })
  expect(await command($, 'bogus')).toEqual({ text: 'Usage: /translate on|off|status' })
  expect(await command($, 'ON')).toEqual({ text: 'Usage: /translate on|off|status' })
  expect(await command($, '')).toEqual({ text: STATUS_OFF })
  expect(await command($, ' on ')).toEqual({ text: STATUS_ON })
  expect(await command($, 'bogus')).toEqual({ text: 'Usage: /translate on|off|status' })
  expect(await command($, 'status')).toEqual({ text: STATUS_ON })
  expect(w.model).toHaveLength(0)
  expect(w.submitted).toHaveLength(0)
  expect(w.session.appended()).toHaveLength(0)
})

test('a /clear or resume after /translate off announces that translation is on again', async ($, on) => {
  const w = world(on)
  await start($)
  await end($, 'clear')
  expect(w.logs).toHaveLength(1)
  await command($, 'off')
  await end($, 'resume')
  expect(w.logs.slice(1)).toEqual(['Korean translation: on again for the new session.'])
  expect(await command($, 'status')).toEqual({ text: STATUS_ON })
})

// ---- input

test('Korean input is translated once and submitted once with its other fields kept', async ($, on) => {
  const w = world(on, () => answered('Fix this function'))
  await start($)
  const attachments = [{ type: 'image', mediaType: 'image/png', id: 'img-1' }]
  const result = await submit($, KOREAN, { attachments, wait: true, turnId: 'turn-9', context: ['earlier note'] })

  expect(result.text).toBe('Fix this function')
  expect(w.submitted).toHaveLength(1)
  const sent = w.submitted[0]
  expect(sent.text).toBe('Fix this function')
  expect(sent.attachments).toEqual(attachments)
  expect(sent.wait).toBe(true)
  expect(sent.turnId).toBe('turn-9')
  expect(sent.origin).toEqual({ kind: 'composer' })
  expect(sent.context).toHaveLength(2)
  expect(sent.context[0]).toBe('earlier note')
  expect(sent.context[1]).toStartWith(TAG)

  // Haiku gets the protected text and the instructions, nothing of the attachments or the conversation.
  expect(w.model).toHaveLength(1)
  expect(Object.keys(w.model[0]).sort()).toEqual(['effort', 'maxTokens', 'model', 'prompt', 'system', 'timeoutMs'])
  expect(w.model[0]).toMatchObject({ model: 'haiku', prompt: KOREAN, maxTokens: 32000 })
  expect(w.model[0].system).toStartWith('Translate the supplied document into English.')
  expect(w.model[0].timeoutMs).toBeLessThanOrEqual(60000)

  // Nothing is appended for the model or the transcript; the indication is shown, then cleared.
  expect(w.session.appended()).toHaveLength(0)
  expect(w.status).toEqual(['Translating to English... (esc cancels)', undefined])
  expect(w.fills).toHaveLength(0)
})

declare const h: (...args: any[]) => any
test('the prompt row shows the Korean original above the English that was sent, and only for a translated composer prompt', async ($, on) => {
  const w = world(on)
  on('ui.render', ($, e: any) => h($.ui.resolve(e).Text, null, e.props.text))
  await start($)
  await submit($, KOREAN)
  const english = w.submitted[0].text
  const row = (props: object) =>
    $.ui.mount({ plugin: 'korean-translation', surface: 'terminal', component: 'UserMessage', requestId: 'm1', props: { text: english, origin: { kind: 'composer' }, isExpanded: true, ...props } } as any)

  let ui = await row({})
  expect((await ui.find({ type: 'Text' }))?.text).toBe(`${KOREAN}\n\n[English] ${english}`)
  await ui.unmount()

  ui = await row({ origin: { kind: 'peer' } })
  expect((await ui.find({ type: 'Text' }))?.text).toBe(english)
  await ui.unmount()

  ui = await row({ text: 'typed in English' })
  expect((await ui.find({ type: 'Text' }))?.text).toBe('typed in English')
  await ui.unmount()

  await end($, 'clear')
  ui = await row({})
  expect((await ui.find({ type: 'Text' }))?.text).toBe(english)
  await ui.unmount()
})

test('mixed input keeps code, paths and links out of the request and intact in the prompt', async ($, on) => {
  const w = world(on)
  await start($)
  const text = '`foo($&)` 를 ./src/a.ts 에서 고치고 https://example.com/x?y=1 참고해 and keep going\n```ts\nconst 값 = 1\n```\n'
  const result = await submit($, text)

  expect(w.model).toHaveLength(1)
  for (const literal of ['foo(', './src/a.ts', 'https://', 'const 값']) expect(w.model[0].prompt).not.toContain(literal)
  expect(w.model[0].prompt).toContain('and keep going')
  expect(w.submitted).toHaveLength(1)
  expect(result.text).toBe('`foo($&)` word ./src/a.ts word word https://example.com/x?y=1 word and keep going\n```ts\nconst 값 = 1\n```\n')
})

const UNTRANSLATED: [string, string, object][] = [
  ['English-only input', 'fix the build', {}],
  ['input whose only Korean is inside protected literals', 'rename `함수` in ./src/a.ts\n```\n주석\n```\n', {}],
  ['a slash command', '/review 이 변경을 봐줘', { command: { name: 'review', args: '이 변경을 봐줘' } }],
  ['a hidden (isMeta) prompt', KOREAN, { isMeta: true }],
]
for (const [name, text, over] of UNTRANSLATED) {
  test(`${name} passes through unchanged with the marker and no model call`, async ($, on) => {
    const w = world(on)
    await start($)
    const result = await submit($, text, over)
    expect(result.text).toBe(text)
    expect(w.submitted).toHaveLength(1)
    expect(w.submitted[0]).toMatchObject({ text, wait: false, origin: { kind: 'composer' }, ...over })
    expect(w.submitted[0].context).toHaveLength(1)
    expect(w.submitted[0].context[0]).toStartWith(TAG)
    expect(w.model).toHaveLength(0)
    expect(w.session.appended()).toHaveLength(0)
    expect(w.status).toHaveLength(0)
  })
}

for (const origin of [{ kind: 'sdk' }, { kind: 'bridge' }, { kind: 'scheduled-trigger' }, { kind: 'task-notification' }, { kind: 'peer' }]) {
  test(`a Korean prompt of origin ${origin.kind} is not rewritten or marked`, async ($, on) => {
    const w = world(on)
    await start($)
    await submit($, KOREAN, { origin, context: ['kept'] })
    expect(w.submitted).toHaveLength(1)
    expect(w.submitted[0]).toMatchObject({ text: KOREAN, origin, context: ['kept'] })
    expect(w.model).toHaveLength(0)
    // Its answer is not translated either.
    await turn($)
    await settle()
    expect(w.model).toHaveLength(0)
    expect(w.session.appended()).toHaveLength(0)
  })
}

const UNSUPPORTED: [string, object][] = [
  ['a non-interactive run', { isInteractive: false }],
  ['the desktop surface', { surface: 'desktop' }],
  ['a session with no surface', { surface: null }],
]
for (const [name, session] of UNSUPPORTED) {
  test(`${name} gets no command, no announcement and no translation`, async ($, on) => {
    const w = world(on)
    await start($, session)
    await submit($, KOREAN)
    await turn($)
    await settle()
    expect(w.registered).toHaveLength(0)
    expect(w.logs).toHaveLength(0)
    expect(w.submitted).toHaveLength(1)
    expect(w.submitted[0].text).toBe(KOREAN)
    expect(w.submitted[0].context).toBeUndefined()
    expect(w.model).toHaveLength(0)
    expect(w.session.appended()).toHaveLength(0)
  })
}

test('after /translate off, Korean input and its answer pass with no marker and no model call', async ($, on) => {
  const w = world(on)
  await start($)
  await command($, 'off')
  await submit($, KOREAN)
  await turn($)
  await settle()
  expect(w.submitted).toHaveLength(1)
  expect(w.submitted[0].text).toBe(KOREAN)
  expect(w.submitted[0].context).toBeUndefined()
  expect(w.model).toHaveLength(0)
  expect(w.session.appended()).toHaveLength(0)

  // Turning it back on marks the next prompt again; the earlier turn is not backfilled.
  await command($, 'on')
  await settle()
  expect(w.model).toHaveLength(0)
  await submit($, 'fix the build')
  expect(w.submitted[1].context[0]).toStartWith(TAG)
})

// ---- input failures

const SECRET = 'sk-provider-detail policy body'
const FAILURES: [string, () => unknown, string][] = [
  ['an API error', () => failed({ reason: 'api-error', status: 529, error: 'overloaded' }), 'haiku returned an API error (overloaded)'],
  ['an aborted call', () => failed({ reason: 'aborted' }), 'it was cancelled or timed out'],
  ['an empty reply', () => failed({ reason: 'empty-reply' }), 'haiku returned no text'],
  ['a blank answer', () => answered(' \n '), 'haiku returned a malformed reply'],
  ['an answer that used the whole output limit', () => answered('Fix this', 32000), 'the translation exceeded the output limit'],
  ['a refused call', () => ({ deny: SECRET }), 'the haiku request was refused'],
]
for (const [name, reply, reason] of FAILURES) {
  test(`${name} drops the prompt, restores the draft and is not retried`, async ($, on) => {
    const w = world(on, reply)
    await start($)
    const result = await submit($, KOREAN, { attachments: [{ type: 'image', id: 'img-1' }] })

    expect(result).toEqual({ drop: `Korean translation failed because ${reason}. Run /translate off to send the prompt without translation.` })
    expect(result.drop).not.toContain(KOREAN)
    expect(result.drop).not.toContain('sk-provider')
    expect(w.submitted).toHaveLength(0)
    expect(w.model).toHaveLength(1)
    expect(w.fills).toHaveLength(1)
    expect(w.fills[0]).toMatchObject({ text: KOREAN, mode: 'replace' })
    expect(w.session.appended()).toHaveLength(0)
    expect(w.logs).toHaveLength(1)
    expect(w.status).toEqual(['Translating to English... (esc cancels)', undefined])
    // A failed input translation leaves nothing pending.
    expect(await command($, 'status')).toEqual({ text: STATUS_ON })
  })
}

test('a failed translation does not overwrite what the person typed meanwhile', async ($, on) => {
  const w = world(on, () => failed({ reason: 'api-error', status: 500, error: 'server_error' }))
  w.box = 'newer draft'
  await start($)
  const result = await submit($, KOREAN)
  expect(result.drop).toContain('server_error')
  expect(w.submitted).toHaveLength(0)
  expect(w.fills).toHaveLength(0)
})

test('a reply that breaks a placeholder is retried exactly once from the same protected text', async ($, on) => {
  const w = world(on, (e, call) => (call === 1 ? answered('Fix the function') : translator(e)))
  await start($)
  const result = await submit($, '`foo()` 함수를 고쳐줘')

  expect(result.text).toBe('`foo()` word word')
  expect(w.submitted).toHaveLength(1)
  expect(w.model).toHaveLength(2)
  expect(w.model[1].prompt).toBe(w.model[0].prompt)
  expect(w.model[0].system).not.toContain('previous attempt failed')
  expect(w.model[1].system).toContain('previous attempt failed placeholder validation')
  expect(w.model[1].timeoutMs).toBeLessThanOrEqual(w.model[0].timeoutMs)
  expect(w.fills).toHaveLength(0)
})

test('a second placeholder failure drops the prompt after two calls', async ($, on) => {
  const w = world(on, e => answered(`${e.prompt} ${e.prompt}`))
  await start($)
  const result = await submit($, '`foo()` 함수를 고쳐줘')
  expect(result).toEqual({
    drop: 'Korean translation failed because the translation changed protected code, paths, or links. Run /translate off to send the prompt without translation.',
  })
  expect(w.model).toHaveLength(2)
  expect(w.submitted).toHaveLength(0)
  expect(w.fills).toHaveLength(1)
})

test('a source over 64 KiB of UTF-8 is dropped without a model call; one at the limit is sent whole', async ($, on) => {
  const w = world(on, () => answered('ok then'))
  await start($)
  const atLimit = `${'가'.repeat(21845)}a` // 65,536 bytes in 21,846 characters
  const over = `${atLimit}a`

  const dropped = await submit($, over)
  expect(dropped).toEqual({ drop: 'Korean translation failed because the text exceeds 64 KiB. Run /translate off to send the prompt without translation.' })
  expect(w.model).toHaveLength(0)
  expect(w.submitted).toHaveLength(0)
  expect(w.fills[0].text).toBe(over)

  await submit($, atLimit)
  expect(w.model).toHaveLength(1)
  expect(w.model[0].prompt).toBe(atLimit)
  expect(w.submitted).toHaveLength(1)
})

test('/translate off while the input translation runs drops the prompt instead of sending it', async ($, on) => {
  const g = gate()
  const w = world(on, async () => {
    await g.opened
    return answered('Fix this function')
  })
  await start($)
  const pending = submit($, KOREAN)
  await settle()
  expect(w.model).toHaveLength(1)
  expect((await command($, 'status')).text).toEndWith('pending: yes')
  await command($, 'off')
  g.open()
  const result = await pending
  expect(result.drop).toStartWith('Korean translation failed because it was cancelled')
  expect(w.submitted).toHaveLength(0)
  expect(w.session.appended()).toHaveLength(0)
  expect(w.fills).toHaveLength(1)
  expect(await command($, 'status')).toEqual({ text: STATUS_OFF })
})

// $.ui.status is fire-and-forget (a denial is dropped, not thrown), so the throw comes from $.prompt.read in the failure path.
test('a hook that throws falls to its catch handler, which keeps untranslated Korean from being sent', async ($, on) => {
  const w = world(on, () => failed({ reason: 'empty-reply' }))
  w.isReadDenied = true
  await start($)
  const result = await submit($, KOREAN)
  expect(result).toEqual({ drop: 'Korean translation failed. Run /translate off to send the prompt without translation.' })
  expect(w.submitted).toHaveLength(0)
  expect(w.model).toHaveLength(1)
})

// ---- output

test('a finished parent turn gets one Korean notice, appended after the hook has returned', async ($, on) => {
  const g = gate()
  const w = world(on, async e => {
    if (e.system.includes('into Korean')) await g.opened
    return translator(e)
  })
  await start($)
  await submit($, KOREAN)
  const answer = 'Run `npm test` first.\n\n- It passed.\n- See ./src/a.ts'
  const result = await turn($, { answer })

  // turn.complete has resolved with the English answer while the translation is still in flight.
  expect(result).toEqual({ text: answer })
  expect(w.model).toHaveLength(2)
  expect(w.notes()).toEqual([])
  expect((await command($, 'status')).text).toEndWith('pending: yes')

  g.open()
  await settle()
  expect(w.notes()).toEqual([`한국어 번역\n\n번역 ${answer}`])
  expect(w.session.appended()[0]).toMatchObject({ door: 'note', origin: { kind: 'plugin', name: 'korean-translation' }, message: { type: 'system' } })
  expect(w.model[1]).toMatchObject({ model: 'haiku' })
  expect(w.model[1].system).toStartWith('Translate the supplied document into Korean.')
  expect(w.model[1].prompt).not.toContain('npm test')
  // The translation is shown, never submitted as a prompt.
  expect(w.submitted).toHaveLength(1)
  expect(w.logs).toHaveLength(1)
  expect(await command($, 'status')).toEqual({ text: STATUS_ON })
})

test('the answer to an English prompt is translated too', async ($, on) => {
  const w = world(on)
  await start($)
  await submit($, 'fix the build')
  await turn($)
  await settle()
  expect(w.model).toHaveLength(1)
  expect(w.notes()).toEqual(['한국어 번역\n\n번역 The build passed.'])
})

const SKIPPED: [string, object][] = [
  ['a subagent turn', { agentId: 'agent-1' }],
  ['an interrupted turn', { isAborted: true }],
  ['a turn that ended aborted', { reason: 'aborted', isAborted: true }],
  ['a turn that ended in an error', { reason: 'error' }],
  ['a refused turn', { reason: 'refusal', refusal: { category: null } }],
  ['an empty answer', { answer: '' }],
  ['an answer with nothing but code', { answer: '```sh\nnpm test\n```\n' }],
  ['an answer already in Korean', { answer: '빌드가 통과했습니다.' }],
]
for (const [name, over] of SKIPPED) {
  test(`${name} is not translated`, async ($, on) => {
    const w = world(on)
    await start($)
    await submit($, 'fix the build')
    const result = await turn($, over)
    await settle()
    expect(result).toEqual({ text: 'answer' in over ? over.answer : 'The build passed.' })
    expect(w.model).toHaveLength(0)
    expect(w.session.appended()).toHaveLength(0)
    expect(w.logs).toHaveLength(1)
  })
}

test('a turn with no marked prompt before it is not translated', async ($, on) => {
  const w = world(on)
  await start($)
  await turn($)
  await settle()
  expect(w.model).toHaveLength(0)
  expect(w.session.appended()).toHaveLength(0)
})

test('a turn delivered twice is translated once', async ($, on) => {
  const w = world(on)
  await start($)
  await submit($, 'fix the build')
  await turn($)
  await turn($)
  await settle()
  await turn($)
  await settle()
  expect(w.model).toHaveLength(1)
  expect(w.session.appended()).toHaveLength(1)
})

const STALE: [string, ($: any) => Promise<unknown>][] = [
  ['a new main-loop turn', $ => $.turn.start({ text: 'next request', turnId: 't2' })],
  ['/translate off', $ => command($, 'off')],
  ['the end of the session', $ => end($, 'clear')],
]
for (const [name, act] of STALE) {
  test(`${name} discards the output translation still in flight`, async ($, on) => {
    const g = gate()
    const w = world(on, async e => {
      await g.opened
      return translator(e)
    })
    await start($)
    await submit($, 'fix the build')
    await turn($)
    await settle()
    expect(w.model).toHaveLength(1)

    await act($)
    g.open()
    await settle()
    expect(w.session.appended()).toHaveLength(0)
    // Nothing about the discarded translation is reported, and it is no longer pending.
    expect(w.logs.filter(line => line.includes('failed'))).toHaveLength(0)
    expect(w.model).toHaveLength(1)
    expect((await command($, 'status')).text).toEndWith('pending: no')
  })
}

test('a subagent turn starting leaves the output translation in flight', async ($, on) => {
  const g = gate()
  const w = world(on, async e => {
    await g.opened
    return translator(e)
  })
  await start($)
  await submit($, 'fix the build')
  await turn($)
  await $.turn.start({ text: 'explore', turnId: 'a1', agentId: 'agent-1' })
  g.open()
  await settle()
  expect(w.notes()).toEqual(['한국어 번역\n\n번역 The build passed.'])
})

test('an automated prompt delivered into a marked turn leaves its answer translated; one starting its own turn does not', async ($, on) => {
  const w = world(on)
  await start($)
  await submit($, 'fix the build')
  await submit($, 'task done', { origin: { kind: 'task-notification' }, turnId: 't1' })
  await turn($)
  await settle()
  expect(w.notes()).toHaveLength(1)

  await submit($, 'task done', { origin: { kind: 'task-notification' } })
  await turn($, { turnId: 't2' })
  await settle()
  expect(w.notes()).toHaveLength(1)
})

test('a failed output translation leaves the English answer and one warning without its text', async ($, on) => {
  const w = world(on, () => ({ deny: SECRET }))
  await start($)
  await submit($, 'fix the build')
  const answer = 'The secret plan passed.'
  const result = await turn($, { answer })
  await settle()

  expect(result).toEqual({ text: answer })
  expect(w.model).toHaveLength(1)
  expect(w.session.appended()).toHaveLength(0)
  expect(w.submitted).toHaveLength(1)
  expect(w.logs.slice(1)).toEqual(['Korean translation failed because the haiku request was refused. The English answer is unchanged.'])
  expect(await command($, 'status')).toEqual({ text: STATUS_ON })
})
