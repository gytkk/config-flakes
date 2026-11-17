import { expect, test } from 'claude-code/testing'
import { DEADLINE_MS, MAX_TOKENS, protect, TranslationRun, validateTranslation } from '../hooks/translate'

const source = 'The build passed. Run `npm test` first.'

for (const prefix of ['', '한국어 번역\n\n', '**한국어 번역**\n\n', '번역 ']) {
  test(`rejects an English copy with prefix ${JSON.stringify(prefix)}`, () => {
    const guarded = protect(source)
    expect(validateTranslation(guarded, prefix + guarded.text, 'Korean')).toEqual({ ok: false, code: 'unchanged' })
  })
}

test('a Korean label or protected Korean code does not establish the target language', () => {
  const guarded = protect('Review `함수` and explain the error.')
  const placeholder = guarded.text.match(/\{\{KEEP_[^}]+\}\}/)![0]
  for (const label of ['한국어 번역\n\n', '번역 ', '한국어 번역: ']) {
    expect(validateTranslation(guarded, `${label}Check ${placeholder} before proceeding.`, 'Korean')).toEqual({ ok: false, code: 'language' })
  }
})

test('real Korean technical prose restores code, paths, and links exactly', () => {
  const guarded = protect('Check `API_URL` in ./src/api.ts and read https://example.com/docs.', 'fixed')
  const reply = '설정은 {{KEEP_fixed_0}}에 있습니다. {{KEEP_fixed_2}}에서 값을 확인하고 {{KEEP_fixed_1}} 문서를 읽으세요.'
  // Inline code is protected first, then URLs, then paths.
  expect(validateTranslation(guarded, reply, 'Korean')).toEqual({
    ok: true,
    text: '설정은 `API_URL`에 있습니다. ./src/api.ts에서 값을 확인하고 https://example.com/docs. 문서를 읽으세요.',
  })
})

test('rejects a long untranslated sentence even after a translated paragraph', () => {
  const sentence = 'The view stores the filter and layout settings together.'
  const guarded = protect(`Create a view.\n\n${sentence}`)
  expect(validateTranslation(guarded, `뷰를 만드세요.\n\n${sentence}`, 'Korean')).toEqual({ ok: false, code: 'coverage' })
})

test('a Korean copy cannot be submitted as an English translation', () => {
  const guarded = protect('이 오류를 고쳐주세요.')
  expect(validateTranslation(guarded, '영어 번역\n\n이 오류를 고쳐주세요.', 'English')).toEqual({ ok: false, code: 'unchanged' })
  expect(validateTranslation(guarded, '문제를 해결해 주세요.', 'English')).toEqual({ ok: false, code: 'language' })
  expect(validateTranslation(guarded, 'Please fix this error.', 'English')).toEqual({ ok: true, text: 'Please fix this error.' })
})

test('different validation failures share one retry and keep the original protected input', () => {
  const run = new TranslationRun(source, 'Korean')
  const first = run.request()
  expect(run.accept(first.prompt, 10)).toBeUndefined()
  const second = run.request()
  expect(second.prompt).toBe(first.prompt)
  expect(second.system).toContain('validation (unchanged)')
  expect(() => run.accept('빌드가 통과했습니다.', 10)).toThrow('protected code')
  expect(run.attempts).toBe(2)
  expect(() => run.request()).toThrow()
})

test('a repaired reply succeeds after a placeholder failure', () => {
  const run = new TranslationRun('Run `npm test`.', 'Korean')
  run.request()
  expect(run.accept('테스트를 실행하세요.', 10)).toBeUndefined()
  const request = run.request()
  expect(request.system).toContain('validation (placeholders)')
  const token = request.prompt.match(/\{\{KEEP_[^}]+\}\}/)![0]
  expect(run.accept(`${token}를 실행하세요.`, 10)).toBe('`npm test`를 실행하세요.')
  expect(() => run.request()).toThrow()
})

test('retry uses the remaining deadline and a late provider success is rejected', () => {
  let now = 100
  const run = new TranslationRun('The build passed.', 'Korean', () => now)
  const first = run.request()
  now += 500
  expect(run.accept(first.prompt, 10)).toBeUndefined()
  expect(run.request().timeoutMs).toBe(DEADLINE_MS - 500)
  now += DEADLINE_MS
  expect(() => run.accept('빌드가 통과했습니다.', 10)).toThrow('timed out')
})

test('empty, oversize, and output-limit replies fail without a validation retry', () => {
  for (const [text, tokens, reason] of [['', 1, 'malformed'], ['가'.repeat(90000), 1, 'malformed'], ['x'.repeat(99001), 1, 'output limit'], ['빌드가 통과했습니다.', MAX_TOKENS, 'output limit']] as const) {
    const run = new TranslationRun('The build passed.', 'Korean')
    run.request()
    expect(() => run.accept(text, tokens)).toThrow(reason)
    expect(run.attempts).toBe(1)
    expect(() => run.request()).toThrow()
  }
})

for (const [name, source, reply] of [
  ['dropped bullets', 'Options:\n- State belongs to a project.\n- Created by needs members.', '옵션:\nState는 프로젝트에 속합니다.\nCreated by에는 구성원이 필요합니다.'],
  ['collapsed items', '- First option.\n- Second option.', '- 첫 번째 옵션과 두 번째 옵션입니다.'],
  ['flattened nesting', '- Parent option.\n  - Child option.', '- 상위 옵션입니다.\n- 하위 옵션입니다.'],
  ['changed numbering', '1. Open the view.\n2. Save the view.', '1. 뷰를 여세요.\n1. 뷰를 저장하세요.'],
  ['lost task state', '- [x] Tests passed.\n- [ ] Deploy later.', '- [ ] 테스트가 통과했습니다.\n- [ ] 나중에 배포하세요.'],
  ['lost heading', '## Checks\nThe build passed.', '검증\n빌드가 통과했습니다.'],
  ['lost blockquote', '> Check the results.', '결과를 확인하세요.'],
  ['broken table', '| Option | Status |\n| --- | --- |\n| Build | Passed |', '| 옵션 | 상태 |\n| --- | --- |\n빌드가 통과했습니다.'],
] as const) {
  test(`rejects Markdown structure damage: ${name}`, () => {
    expect(validateTranslation(protect(source), reply, 'Korean')).toEqual({ ok: false, code: 'structure' })
  })
}

test('nested lists, task states, headings, quotes, tables, and fenced code survive translation', () => {
  const source = '## Checks\n\n- [x] Parent option\n  1. Child option\n\n> Check results.\n\n| Option | Status |\n| --- | :---: |\n| Build | Passed |\n\n```sh\n# Code, not a heading\n- literal\n```'
  const guarded = protect(source)
  const reply = guarded.text.replaceAll('Checks', '검증').replaceAll('Parent option', '상위 옵션').replaceAll('Child option', '하위 옵션').replaceAll('Check results.', '결과를 확인하세요.').replaceAll('Option', '옵션').replaceAll('Status', '상태').replaceAll('Build', '빌드').replaceAll('Passed', '통과')
  const result = validateTranslation(guarded, reply, 'Korean')
  expect(result.ok).toBe(true)
  if (result.ok) expect(result.text).toContain('```sh\n# Code, not a heading\n- literal\n```')
})

test('a structure failure is repaired within the shared retry limit', () => {
  const run = new TranslationRun('- First option.\n- Second option.', 'Korean')
  run.request()
  expect(run.accept('첫 번째 옵션과 두 번째 옵션입니다.', 10)).toBeUndefined()
  expect(run.request().system).toContain('validation (structure)')
  expect(run.accept('- 첫 번째 옵션입니다.\n- 두 번째 옵션입니다.', 10)).toBe('- 첫 번째 옵션입니다.\n- 두 번째 옵션입니다.')
  expect(run.attempts).toBe(2)
})

test('leading indentation on the first list item is not trimmed away', () => {
  const run = new TranslationRun('  - Child option.', 'Korean')
  run.request()
  expect(run.accept('  - 하위 옵션입니다.', 10)).toBe('  - 하위 옵션입니다.')
})

test('completion freezes elapsed time for later status displays', () => {
  let now = 0
  const run = new TranslationRun('The build passed.', 'Korean', () => now)
  run.request()
  now = 25
  expect(run.accept('빌드가 통과했습니다.', 10)).toBe('빌드가 통과했습니다.')
  now = 1000
  expect(run.elapsedMs).toBe(25)
})
