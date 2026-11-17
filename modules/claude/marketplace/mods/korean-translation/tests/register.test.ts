import { expect, mock, test } from 'claude-code/testing'
import type { On } from 'claude-code'

const TAG = '[korean-translation: active]'
const USAGE = { input_tokens: 1, output_tokens: 1, cache_read_input_tokens: 0, cache_creation_input_tokens: 0 }
const answered = (text: string, output_tokens = 1) => ({ value: { isAnswered: true, text, usage: { ...USAGE, output_tokens } } })
const failed = (fields: object) => ({ value: { isAnswered: false, usage: USAGE, ...fields } })
const koreanReply = (prompt: string) => prompt
  .replaceAll('The build passed.', '빌드가 통과했습니다.')
  .replace(/^Run (.+) first\.$/gm, '$1를 먼저 실행하세요.')
  .replaceAll('- It passed.', '- 검사가 통과했습니다.')
  .replace(/^- See (.+)$/gm, '- $1를 확인하세요.')
// Wiring stub only. Validation tests use complete bilingual prose fixtures.
const translator = (e: any) => answered(e.system.includes('into Korean') ? koreanReply(e.prompt) : e.prompt.replace(/\p{Script=Hangul}+/gu, 'word'))
const KOREAN = '이 함수를 고쳐줘'
const SECRET = 'sk-provider-detail policy body'

function gate() {
  let open!: () => void
  const opened = new Promise<void>(resolve => { open = resolve })
  return { opened, open }
}

function world($: any, on: On, reply: (e: any, call: number) => unknown = translator) {
  let change = gate()
  const emit = () => { const previous = change; change = gate(); previous.open() }
  const w = {
    box: '', isReadDenied: false, placeAllowed: true, messagesDenied: false,
    model: [] as any[], logs: [] as string[], status: [] as (string | undefined)[], fills: [] as any[],
    submitted: [] as any[], registered: [] as any[], placed: [] as string[], signals: [] as AbortSignal[],
    api: [] as any[], redraws: 0, session: mock.session(on),
    async until(condition: () => boolean) { while (!condition()) await change.opened },
    async idle(): Promise<void> {
      for (;;) {
        const changed = change.opened
        if (!(await w.command('status')).text.includes('pending: yes')) return
        await changed
      }
    },
    start: (over: object = {}) => $.session.start({ cwd: '/', surface: 'terminal', isInteractive: true, ...over }),
    command: (args: string) => $.command.run({ command: 'translate', args, origin: { kind: 'composer' }, presentation: { isFullscreen: false, columns: 80 } }),
    async submit(text: string, over: object = {}, turnId: string | null = 't1'): Promise<any> {
      const result = await $.prompt.submit({ text, wait: false, origin: { kind: 'composer' }, ...over })
      if (result.drop === undefined && turnId) await w.begin(turnId)
      return result
    },
    async begin(turnId: string, index?: number): Promise<void> {
      const prompt = w.submitted[index ?? w.submitted.length - 1]
      w.api = prompt ? [{ role: 'user', content: [{ type: 'text', text: prompt.text }, ...(prompt.context ?? []).map((text: string) => ({ type: 'text', text }))] }] : []
      await $.turn.start({ text: prompt?.text ?? '', turnId })
    },
    complete: (over: object = {}) => $.turn.complete({ answer: 'The build passed.', durationMs: 1, isAborted: false, turnId: 't1', reason: 'answer', ...over }),
    end: (reason = 'clear') => $.session.end({ reason, sessionId: 's1', resume: { id: 's1' } }),
    async row(index: number, plugin = 'korean-translation'): Promise<string | undefined> {
      const id = w.placed[index]
      if (!id) return undefined
      const ui = await $.ui.mount({ plugin: 'korean-translation', surface: 'terminal', component: 'Placed', requestId: `placed-${id}`, props: { id, plugin } })
      const leaf = (await ui.find({ type: 'Markdown' })) ?? (await ui.find({ type: 'Text' }))
      const text = leaf?.text
      await ui.unmount()
      return text
    },
  }
  on('model.complete', async (_, e, next) => {
    w.model.push(e)
    w.signals.push(next.signal)
    emit()
    return await reply(e, w.model.length) as any
  })
  on('ui.log', (_, e) => { w.logs.push(e.text); emit(); return { value: undefined } })
  on('ui.status', (_, e) => { w.status.push(e.text); emit(); return { value: undefined } })
  on('command.register', (_, e) => { w.registered.push(e); return { value: { command: e.name } } })
  on('prompt.read', () => w.isReadDenied ? { deny: 'no prompt box' } : { value: { text: w.box, cursor: 0 } })
  on('prompt.fill', (_, e) => { w.fills.push(e); return { isFilled: true } })
  on('session.start', (_, e) => ({ cwd: e.cwd }))
  on('session.end', (_, e) => ({ sessionId: e.sessionId }))
  on('prompt.submit', (_, e) => { w.submitted.push(e); return { text: e.text, context: e.context } })
  on('session.messages', () => w.messagesDenied ? { deny: 'not available' } : { value: w.api })
  on('turn.start', (_, e) => ({ turnId: e.turnId }))
  on('turn.complete', (_, e) => ({ text: e.answer }))
  on('ui.invalidate', () => { w.redraws++; emit(); return { value: undefined } })
  on('ui.place', (_, e) => {
    if (!w.placeAllowed) return { value: { isPlaced: false, reason: 'test surface unavailable' } }
    w.placed.push(e.id)
    emit()
    return { value: { isPlaced: true, requestId: `placed-${e.id}` } }
  })
  return w
}

test('load and controls make no model calls, and invalid controls preserve state', async ($, on) => {
  const w = world($, on)
  await w.start()
  expect(w.registered[0]).toMatchObject({ name: 'translate', immediate: true })
  expect(w.logs).toHaveLength(1)
  expect((await w.command('')).text).toContain('Korean translation: on')
  expect((await w.command('off')).text).toContain('Korean translation: off')
  expect(await w.command('bogus')).toEqual({ text: 'Usage: /translate on|off|status' })
  expect((await w.command('status')).text).toContain('Korean translation: off')
  expect((await w.command(' on ')).text).toContain('Korean translation: on')
  expect(w.model).toHaveLength(0)
})

test('clear/resume resets a disabled toggle and announces it', async ($, on) => {
  const w = world($, on)
  await w.start()
  await w.command('off')
  await w.end('resume')
  expect((await w.command('status')).text).toContain('Korean translation: on')
  expect(w.logs.at(-1)).toContain('on again')
})

test('input is submitted once with attachments intact and the original in its own display-only row', async ($, on) => {
  const w = world($, on, () => answered('Fix this function'))
  await w.start()
  const attachments = [{ type: 'image', id: 'img-1' }]
  const result = await w.submit(KOREAN, { attachments, context: ['kept'] })
  expect(result.text).toBe('Fix this function')
  expect(w.submitted).toHaveLength(1)
  expect(w.submitted[0]).toMatchObject({ attachments, origin: { kind: 'composer' }, wait: false })
  expect(w.submitted[0].context[0]).toBe('kept')
  expect(w.submitted[0].context[1]).toStartWith(TAG)
  expect(w.model[0]).toMatchObject({ model: 'haiku', prompt: KOREAN })
  expect(w.model[0].attachments).toBeUndefined()
  expect(await w.row(0)).toBe(`**입력 원문**\n\n${KOREAN}`)
  expect(w.session.appended()).toHaveLength(0)
  expect((await w.command('status')).text).toEndWith('pending: no')
})

test('identical English translations retain distinct Korean originals', async ($, on) => {
  const w = world($, on, () => answered('Fix this function'))
  await w.start()
  await w.submit('함수를 고쳐줘', {}, 't1')
  await w.submit('이 함수 수정해줘', {}, 't2')
  expect(w.placed[0]).not.toBe(w.placed[1])
  expect(await w.row(0)).toContain('함수를 고쳐줘')
  expect(await w.row(1)).toContain('이 함수 수정해줘')
})

test('mixed input keeps literals out of Haiku and intact after restoration', async ($, on) => {
  const w = world($, on)
  await w.start()
  const result = await w.submit('`foo($&)` 를 ./src/a.ts 에서 고치고 https://example.com/x?y=1 참고해 and keep going\n```ts\nconst 값 = 1\n```\n')
  for (const literal of ['foo(', './src/a.ts', 'https://', 'const 값']) expect(w.model[0].prompt).not.toContain(literal)
  expect(result.text).toBe('`foo($&)` word ./src/a.ts word word https://example.com/x?y=1 word and keep going\n```ts\nconst 값 = 1\n```\n')
})

for (const text of ['fix the build', 'rename `함수` in ./src/a.ts\n```\n주석\n```']) {
  test(`no input request is needed for ${JSON.stringify(text)}`, async ($, on) => {
    const w = world($, on)
    await w.start()
    expect((await w.submit(text)).text).toBe(text)
    expect(w.submitted[0].context[0]).toStartWith(TAG)
    expect(w.model).toHaveLength(0)
    expect(w.placed).toHaveLength(0)
  })
}

for (const over of [{ command: { name: 'review', args: KOREAN } }, { isMeta: true }, ...['sdk', 'bridge', 'scheduled-trigger', 'task-notification', 'peer'].map(kind => ({ origin: { kind } }))]) {
  test(`excluded prompt ${JSON.stringify(over)} is neither rewritten nor marked`, async ($, on) => {
    const w = world($, on)
    await w.start()
    await w.submit(KOREAN, { context: ['kept'], ...over })
    await w.complete()
    await w.idle()
    expect(w.submitted[0]).toMatchObject({ text: KOREAN, context: ['kept'] })
    expect(w.model).toHaveLength(0)
  })
}

for (const session of [{ isInteractive: false }, { surface: 'desktop' }, { surface: null }]) {
  test(`unsupported session ${JSON.stringify(session)} performs no translation`, async ($, on) => {
    const w = world($, on)
    await w.start(session)
    await w.submit(KOREAN)
    await w.complete()
    expect(w.model).toHaveLength(0)
    expect(w.registered).toHaveLength(0)
    expect(w.logs).toHaveLength(0)
  })
}

test('off leaves input and output unchanged without markers', async ($, on) => {
  const w = world($, on)
  await w.start()
  await w.command('off')
  await w.submit(KOREAN)
  await w.complete()
  expect(w.submitted[0].context).toBeUndefined()
  expect(w.model).toHaveLength(0)
  await w.command('on')
  expect(w.model).toHaveLength(0)
})

const FAILURES: [string, () => unknown, string][] = [
  ['API error', () => failed({ reason: 'api-error', error: SECRET }), 'provider'],
  ['abort', () => failed({ reason: 'aborted' }), 'deadline'],
  ['empty', () => failed({ reason: 'empty-reply' }), 'empty'],
  ['blank', () => answered(' \n '), 'malformed'],
  ['output limit', () => answered('Fix this', 32000), 'output-limit'],
  ['refusal', () => ({ deny: SECRET }), 'refused'],
]
for (const [name, reply, code] of FAILURES) {
  test(`input ${name} restores the draft and reports a sanitized reason without retry`, async ($, on) => {
    const w = world($, on, reply)
    await w.start()
    const result = await w.submit(KOREAN)
    expect(result.drop).toContain(`[${code}]`)
    expect(result.drop).not.toContain(SECRET)
    expect(w.submitted).toHaveLength(0)
    expect(w.model).toHaveLength(1)
    expect(w.fills[0].text).toBe(KOREAN)
    expect((await w.command('status')).text).toContain(`last failure: input: failed [${code}]`)
    expect(w.placed).toHaveLength(0)
  })
}

test('input failure preserves a newer draft and catches failed editor restoration', async ($, on) => {
  const w = world($, on, () => failed({ reason: 'empty-reply' }))
  await w.start()
  w.box = 'newer draft'
  await w.submit(KOREAN)
  expect(w.fills).toHaveLength(0)
  w.isReadDenied = true
  expect((await w.submit(KOREAN)).drop).toContain('[unexpected]')
  expect(w.submitted).toHaveLength(0)
})

test('one placeholder retry uses the original input and repairs the translation', async ($, on) => {
  const w = world($, on, (e, call) => call === 1 ? answered('Fix the function') : translator(e))
  await w.start()
  expect((await w.submit('`foo()` 함수를 고쳐줘')).text).toBe('`foo()` word word')
  expect(w.model).toHaveLength(2)
  expect(w.model[1].prompt).toBe(w.model[0].prompt)
  expect(w.model[1].system).toContain('validation (placeholders)')
  expect(w.model[1].timeoutMs).toBeLessThanOrEqual(w.model[0].timeoutMs)
})

test('different validation failures still allow only two attempts', async ($, on) => {
  const w = world($, on, (e, call) => call === 1 ? answered(e.prompt) : answered('Fix the function'))
  await w.start()
  expect((await w.submit('`foo()` 함수를 고쳐줘')).drop).toContain('[placeholders]')
  expect(w.model).toHaveLength(2)
  expect(w.model[1].system).toContain('validation (unchanged)')
  expect(w.submitted).toHaveLength(0)
})

test('UTF-8 source limit rejects oversize input without a model request', async ($, on) => {
  const w = world($, on, () => answered('ok then'))
  await w.start()
  const atLimit = `${'가'.repeat(21845)}a`
  expect((await w.submit(atLimit + 'a')).drop).toContain('[source-limit]')
  expect(w.model).toHaveLength(0)
  await w.submit(atLimit)
  expect(w.model[0].prompt).toBe(atLimit)
})

for (const replaceSession of [false, true]) {
  test(`pending input is cancelled safely; session replacement=${replaceSession}`, async ($, on) => {
    const g = gate()
    const w = world($, on, async () => { await g.opened; return answered('Fix this function') })
    await w.start()
    const pending = w.submit(KOREAN)
    await w.until(() => w.model.length === 1)
    if (replaceSession) await w.end()
    else await w.command('off')
    g.open()
    expect((await pending).drop).toContain('[deadline]')
    expect(w.submitted).toHaveLength(0)
    expect(w.fills).toHaveLength(replaceSession ? 0 : 1)
  })
}

for (const prefix of ['', '한국어 번역\n\n']) {
  test(`English copy with ${JSON.stringify(prefix)} is never displayed as Korean`, async ($, on) => {
    const w = world($, on, e => answered(prefix + e.prompt))
    await w.start()
    await w.submit('fix the build')
    await w.complete()
    await w.idle()
    expect(w.model).toHaveLength(2)
    expect(await w.row(0)).toContain('failed [unchanged]')
    expect(await w.row(0)).not.toContain('**한국어 번역**')
    expect((await w.command('status')).text).toContain('attempts: 2')
  })
}

test('output is placed pending after the answer and updates in place, without changing messages', async ($, on) => {
  const g = gate()
  const w = world($, on, async e => { await g.opened; return translator(e) })
  await w.start()
  await w.submit('fix the build')
  const answer = 'Run `npm test` first.\n\n- It passed.\n- See ./src/a.ts'
  expect(await w.complete({ answer })).toEqual({ text: answer })
  await w.until(() => w.model.length === 1)
  expect(await w.row(0)).toContain('translating')
  g.open()
  await w.idle()
  expect(await w.row(0)).toBe('`npm test`를 먼저 실행하세요.\n\n- 검사가 통과했습니다.\n- ./src/a.ts를 확인하세요.')
  expect(await w.row(0, 'another-plugin')).toBeUndefined()
  expect(w.placed).toHaveLength(1)
  expect(w.session.appended()).toHaveLength(0)
  expect(w.submitted).toHaveLength(1)
  await w.row(0)
  expect(w.model).toHaveLength(1)
})

for (const over of [{ agentId: 'a1' }, { reason: 'aborted', isAborted: true }, { reason: 'error' }, { reason: 'refusal', refusal: { category: null } }, { answer: '' }, { answer: '```sh\nnpm test\n```' }, { answer: '빌드가 통과했습니다.' }]) {
  test(`completion ${JSON.stringify(over)} is not translated`, async ($, on) => {
    const w = world($, on)
    await w.start()
    await w.submit('fix the build')
    await w.complete(over)
    expect(w.model).toHaveLength(0)
    expect(w.placed).toHaveLength(0)
  })
}

test('duplicate completion and completion with no eligible start do not make extra calls', async ($, on) => {
  const w = world($, on)
  await w.start()
  await w.complete()
  expect(w.model).toHaveLength(0)
  await w.submit('fix the build')
  await w.complete()
  await w.complete()
  await w.idle()
  await w.complete()
  expect(w.model).toHaveLength(1)
})

test('identical answers have separate rows and a new turn does not cancel older translation', async ($, on) => {
  const g = gate()
  const w = world($, on, async (_, call) => { if (call === 1) await g.opened; return answered(call === 1 ? '첫 번째 빌드가 통과했습니다.' : '두 번째 빌드가 통과했습니다.') })
  await w.start()
  await w.submit('fix the build', {}, 't1')
  await w.complete()
  await w.until(() => w.model.length === 1)
  await w.submit('fix the build', {}, 't2')
  await w.complete({ turnId: 't2' })
  expect(w.model).toHaveLength(1)
  expect(await w.row(1)).toContain('pending')
  g.open()
  await w.idle()
  expect(w.placed[0]).not.toBe(w.placed[1])
  expect(await w.row(0)).toBe('첫 번째 빌드가 통과했습니다.')
  expect(await w.row(1)).toBe('두 번째 빌드가 통과했습니다.')
})

for (const action of ['off', 'clear', 'aborted']) {
  test(`${action} cancels pending output and rejects late results`, async ($, on) => {
    const g = gate()
    const w = world($, on, async e => { await g.opened; return translator(e) })
    await w.start()
    await w.submit('fix the build')
    await w.complete()
    await w.until(() => w.model.length === 1)
    if (action === 'off') await w.command('off')
    else if (action === 'clear') await w.end()
    else await w.complete({ turnId: 't2', reason: 'aborted', isAborted: true })
    g.open()
    await w.idle()
    expect(await w.row(0)).not.toBe('빌드가 통과했습니다.')
    expect(w.logs.some(line => line.includes('failed'))).toBe(false)
  })
}

test('queued unmarked prompts cannot change an already started turn eligibility', async ($, on) => {
  const w = world($, on)
  await w.start()
  await w.submit('fix the build', {}, 't1')
  await w.submit('task done', { origin: { kind: 'task-notification' } }, null)
  await w.complete()
  await w.idle()
  expect(w.model).toHaveLength(1)
  await w.begin('t2')
  await w.complete({ turnId: 't2' })
  expect(w.model).toHaveLength(1)
})

test('queued identical prompts are distinguished by request nonces', async ($, on) => {
  const w = world($, on)
  await w.start()
  await w.submit('fix the build', {}, null)
  await w.submit('fix the build', { origin: { kind: 'peer' } }, null)
  await w.begin('t1', 0)
  await w.complete()
  await w.idle()
  await w.begin('t2', 1)
  await w.complete({ turnId: 't2' })
  expect(w.model).toHaveLength(1)
})

test('output provider failure leaves English intact and shows a sanitized diagnostic', async ($, on) => {
  const w = world($, on, () => failed({ reason: 'api-error', error: SECRET }))
  await w.start()
  await w.submit('fix the build')
  expect(await w.complete()).toEqual({ text: 'The build passed.' })
  await w.idle()
  expect(await w.row(0)).toContain('failed [provider]')
  expect(w.logs.join('\n')).not.toContain(SECRET)
  expect(w.model).toHaveLength(1)
})

test('placement failure spends no translation call and does not guess a matching answer row', async ($, on) => {
  const w = world($, on)
  await w.start()
  await w.submit('fix the build')
  w.placeAllowed = false
  await w.complete()
  expect(w.model).toHaveLength(0)
  expect(w.logs.at(-1)).toContain('failed')
})

test('an unavailable conversation context produces an explicit diagnostic', async ($, on) => {
  const w = world($, on)
  await w.start()
  w.messagesDenied = true
  await w.submit('fix the build')
  await w.complete()
  expect(w.model).toHaveLength(0)
  expect(w.logs.at(-1)).toContain('[turn-context]')
})

test('new-session work resumes after an old cancelled provider call returns', async ($, on) => {
  const g = gate()
  const w = world($, on, async (e, call) => { if (call === 1) await g.opened; return translator(e) })
  await w.start()
  await w.submit('fix the build')
  await w.complete()
  await w.until(() => w.model.length === 1)
  await w.end()
  await w.submit('fix the next build', {}, 't2')
  await w.complete({ turnId: 't2' })
  g.open()
  await w.idle()
  expect(await w.row(0)).toBeUndefined()
  expect(await w.row(1)).toBe('빌드가 통과했습니다.')
  expect(w.model).toHaveLength(2)
})

test('a subagent start does not affect pending parent translation', async ($, on) => {
  const g = gate()
  const w = world($, on, async e => { await g.opened; return translator(e) })
  await w.start()
  await w.submit('fix the build')
  await w.complete()
  await w.until(() => w.model.length === 1)
  await $.turn.start({ text: 'explore', turnId: 'a1', agentId: 'agent-1' })
  g.open()
  await w.idle()
  expect(await w.row(0)).toBe('빌드가 통과했습니다.')
})

test('a second input cannot race the in-flight input translator', async ($, on) => {
  const g = gate()
  const w = world($, on, async () => { await g.opened; return answered('Fix this function') })
  await w.start()
  const pending = w.submit(KOREAN)
  await w.until(() => w.model.length === 1)
  expect((await w.submit('다음 함수도 고쳐줘')).drop).toContain('already running')
  g.open()
  await pending
  expect(w.model).toHaveLength(1)
  expect(w.submitted).toHaveLength(1)
})

test('list damage is retried and the displayed body uses Markdown beside a fixed response gutter', async ($, on) => {
  const answer = '- First option.\n  - Child option.\n- Second option.'
  const translated = '- 첫 번째 옵션입니다.\n  - 하위 옵션입니다.\n- 두 번째 옵션입니다.'
  const w = world($, on, (_, call) => answered(call === 1 ? '옵션을 확인하세요.' : translated))
  await w.start()
  await w.submit('explain the options')
  await w.complete({ answer })
  await w.idle()
  expect(w.model).toHaveLength(2)
  expect(w.model[1].system).toContain('validation (structure)')
  expect(w.placed).toHaveLength(1)
  const id = w.placed[0]!
  const ui = await $.ui.mount({ plugin: 'korean-translation', surface: 'terminal', component: 'Placed', requestId: 'format-check', props: { id, plugin: 'korean-translation' } })
  expect((await ui.find({ key: 'translation-body' }))?.text).toBe(translated)
  expect(await ui.find({ type: 'Text', text: '⏺' })).toBeDefined()
  const tree = await ui.drawn() as any
  expect(tree.children[1]).toMatchObject({ type: 'Box', props: { flexDirection: 'row' } })
  expect(tree.children[1].children[0]).toMatchObject({ type: 'Box', props: { width: 2, flexShrink: 0 } })
  expect(tree.children[1].children[1]).toMatchObject({ type: 'Box', props: { flexDirection: 'column', flexGrow: 1, flexShrink: 1 } })
  await ui.unmount()
})

test('output queue is bounded while older work remains pending', async ($, on) => {
  const g = gate()
  const w = world($, on, async e => { await g.opened; return translator(e) })
  await w.start()
  for (let i = 0; i < 5; i++) {
    await w.submit('fix the build', {}, `t${i}`)
    await w.complete({ turnId: `t${i}` })
  }
  expect(w.model).toHaveLength(1)
  expect(w.placed).toHaveLength(4)
  expect(w.logs.at(-1)).toContain('[capacity]')
  g.open()
  await w.idle()
  expect(w.model).toHaveLength(4)
})
