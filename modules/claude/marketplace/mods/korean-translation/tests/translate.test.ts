import { expect, test } from 'claude-code/testing'
import { byteLength, hasHangul, MAX_BYTES, needsTranslation, protect } from '../hooks/translate'

const ID = 'abc'
const keep = (n: number, prefix = `KEEP_${ID}_`) => `{{${prefix}${n}}}`

// Literals with characters that String.replace would expand if restore used a replacement string.
const FENCE = '```sh\necho "$&" \'$1\' $$ 한글\n\ttab  \n```\n'
const SOURCE = `먼저 \`a$&b\` 를 보고 https://example.com/a?b=1&c=한 그리고 ./src/main.ts 와 ~/.config/x.toml 수정\n${FENCE}끝`

test('fenced code, inline code, URLs and paths leave the prose', () => {
  const guarded = protect(SOURCE, ID)
  expect(guarded.text).toBe(`먼저 ${keep(1)} 를 보고 ${keep(2)} 그리고 ${keep(3)} 와 ${keep(4)} 수정\n${keep(0)}끝`)
  expect(guarded.prose).toBe('먼저  를 보고  그리고  와  수정\n끝')
})

test('restore puts every literal back byte for byte, in any order', () => {
  const guarded = protect(SOURCE, ID)
  expect(guarded.restore(guarded.text)).toBe(SOURCE)
  expect(guarded.restore(`${keep(4)} ${keep(0)} ${keep(3)} ${keep(2)} ${keep(1)}`)).toBe(
    `~/.config/x.toml ${FENCE} ./src/main.ts https://example.com/a?b=1&c=한 \`a$&b\``,
  )
})

test('an unclosed fence is protected to the end of the text', () => {
  const source = '설명\n```\ncode 한글'
  const guarded = protect(source, ID)
  expect(guarded.text).toBe(`설명\n${keep(0)}`)
  expect(guarded.restore(guarded.text)).toBe(source)
})

test('a missing, duplicated, unknown or altered placeholder is rejected', () => {
  const guarded = protect('`a` 그리고 `b`', ID)
  expect(guarded.restore(`${keep(0)} and ${keep(1)}`)).toBe('`a` and `b`')
  expect(guarded.restore(`${keep(0)} and`)).toBeUndefined()
  expect(guarded.restore(`${keep(0)} and ${keep(0)}`)).toBeUndefined()
  expect(guarded.restore(`${keep(0)} and ${keep(1)} ${keep(1)}`)).toBeUndefined()
  expect(guarded.restore(`${keep(0)} and ${keep(2)}`)).toBeUndefined()
  expect(guarded.restore(`${keep(0)} and {{keep_${ID}_1}}`)).toBeUndefined()
  expect(guarded.restore('a and b')).toBeUndefined()
})

test('text without literals restores only a reply without placeholders', () => {
  const guarded = protect('안녕하세요', ID)
  expect(guarded.text).toBe('안녕하세요')
  expect(guarded.restore('Hello')).toBe('Hello')
  expect(guarded.restore(`Hello ${keep(0)}`)).toBeUndefined()
})

test('a source that already contains the prefix gets a different one', () => {
  const source = `${keep(0)} 는 그대로 두고 \`x\` 만 바꿔`
  const guarded = protect(source, ID)
  const fresh = keep(0, `KEEP_${ID}0_`)
  expect(guarded.text).toBe(`${keep(0)} 는 그대로 두고 ${fresh} 만 바꿔`)
  // The person's own look-alike is prose: the translator may keep it, and it is never treated as a placeholder.
  expect(guarded.restore(`Leave ${keep(0)} alone and change only ${fresh}`)).toBe(`Leave ${keep(0)} alone and change only \`x\``)
  expect(guarded.restore(`Leave ${keep(0)} alone and change only ${keep(0)}`)).toBeUndefined()
})

test('the default placeholder id differs between calls', () => {
  expect(protect('`a`').text).not.toBe(protect('`a`').text)
})

test('Hangul detection covers syllables and Jamo', () => {
  expect(hasHangul('가')).toBe(true)
  expect(hasHangul('ㅋㅋ')).toBe(true)
  expect(hasHangul('가')).toBe(true)
  expect(hasHangul('hello 日本語 123')).toBe(false)
})

test('only unprotected prose decides whether a text needs translation', () => {
  expect(needsTranslation('이 함수를 고쳐줘', 'English')).toBe(true)
  expect(needsTranslation('fix `함수` please', 'English')).toBe(false)
  expect(needsTranslation('```\n한글\n```\n', 'English')).toBe(false)
  expect(needsTranslation('fix this', 'English')).toBe(false)
  expect(needsTranslation('The build passed.', 'Korean')).toBe(true)
  expect(needsTranslation('`npm test`\n```\nok done\n```\n', 'Korean')).toBe(false)
  expect(needsTranslation('', 'Korean')).toBe(false)
  expect(needsTranslation('42.', 'Korean')).toBe(false)
})

test('the size limit counts UTF-8 bytes, not characters', () => {
  expect(MAX_BYTES).toBe(65536)
  expect(byteLength('가')).toBe(3)
  expect(byteLength('😀')).toBe(4)
  expect(byteLength('a'.repeat(10))).toBe(10)
})
