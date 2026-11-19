# Prose quality

Apply these rules to documentation, comments, reports, prompts, commit messages, and other maintained prose.

## Accuracy and voice

- Preserve the source meaning, uncertainty, exact identifiers, official names, and quotations. Add facts or specifics only when evidence supports them.
- Prefer the author's established voice when a sample exists. Do not replace uncertainty with invented precision.
- Explain enough context that a reader can identify the actor, action, condition, and outcome without the immediate conversation.

## Clear prose

- State the main point first. Use complete sentences with concrete subjects and verbs.
- Remove filler, repeated conclusions, vague abstractions, promotional language, excessive hedging, and jargon that hides the mechanism.
- Prefer neutral, direct wording. Define unfamiliar abbreviations and use one term consistently for each concept.
- Use lists only for genuinely parallel items. Avoid decorative headings, forced groupings, rhetorical questions, and formulaic contrasts.

## Technical writing

- Describe current system behavior. Reserve change narration for changelogs, release notes, and migration guides.
- Write a one- or two-line comment or docstring only when it is necessary to explain a reason, constraint, unit, invariant, or behavior the code cannot reveal.
- Keep documentation close to the maintained source of truth. Do not copy details that readers can obtain cheaply from current configuration or command output.

## Punctuation and formatting

- Use ASCII punctuation in prose. Use straight quotes and rewrite em dashes, en dashes, and double hyphens as sentence punctuation.
- Preserve established spelling and required hyphens in official names, identifiers, URLs, commands, and quotations.
- Keep each Markdown paragraph on one source line. Add line breaks only for Markdown structure or meaning.
- In conversational responses, write each URL as a Markdown link with a short label, such as `[devsisters/ark#422](https://github.com/devsisters/ark/pull/422)`. A terminal or translation layer decides where a bare URL ends from the characters after it, and a following period or Korean particle becomes part of the link. Where a bare URL is required, follow it with a space or line break, never with punctuation or a word.
- Use sentence case for English headings and avoid decorative formatting or emojis.

## Korean prose

Write Korean so that a first-time reader or a later agent can pin down the meaning from the sentence alone. Spell out an unfamiliar compressed word or an awkward translation even when the sentence gets longer, and add the English original in parentheses when it helps.

Word choice:

- Write the predicate out instead of compressing it into a prefixed or Sino-Korean coined noun, such as `내장되어 있지 않은` instead of `미내장`, `값이 달라지지 않는다` instead of `무영향`, `범위를 정한다` instead of `획정`, and `현행 여부` instead of `현행성`. Keep words already in everyday use, such as `미확인` and `비정상`, and the PR title form specified below.
- Follow established usage for English terms. Keep established technical terms and official names in English, such as `dot notation` instead of `점접근`. Use the common Korean word when one exists, such as `모범 사례` instead of `exemplar`, and write a Korean word plus the original instead of an unfamiliar phonetic transcription, such as `비정상(anomaly)` instead of `어노말리`.
- Write in Hangul without Hanja, such as `재시도`.
- Use precise verbs such as `명시하다`, `포함하다`, `고정하다`, and `저장하다` instead of informal `박다` expressions.
- Describe what actually happens to documents, links, data, and checks instead of personifying them or treating them as containers: `링크가 끊어진다` instead of `링크가 죽는다`, `원본이 바뀌어도 사본은 갱신되지 않는다` instead of `사본이 낡는다`, `결과에 포함한다` instead of `결과에 담는다`, and `허용 범위가 넓은 기준` instead of `느슨한 기준`. Describe system behavior without dramatizing adverbs such as `조용히`.
- Name a criterion, layer, or convention by what it is: `분류 기준` instead of `분류 축`, `기존 네이밍 관행` instead of `레포 결`, `컬럼 comment` instead of `comment 층`, `원본` or `기준 문서` instead of `정본`, and `메트릭 기록` instead of `계측`.
- Use `관측` only when contrasting actual data with defined or expected behavior. Otherwise state what the data records or what actually happened.

Sentences:

- Write complete sentences with clear particles and predicates instead of noun chains such as `실패 깨움 기반 동시성 적응 복원`.
- State policy and architecture through their mechanism: who or what does which action under which condition. Metaphors such as `확장의 무게는 ~에 싣는다` and a bare `구조적 위험` hide that mechanism.
- Join clauses that carry cause, contrast, or condition with connective endings such as `~므로`, `~지만`, and `~는데`, such as `일별로 적재되지만 지연이 있을 수 있으므로 조회할 때 주의한다` instead of three short `~다.` sentences. Put independent parallel items in a bullet list.
- Bullet items and table cells may end in a noun form such as `건너뜀` or `후보는 유지`. Keep that ending when editing the phrase.
- Give each of two contrasted subjects its own bullet or sentence instead of the `A는 B, C는 D` pattern, and write conditional branches as one bullet per condition.
- State a count only when it supports a reason, constraint, or decision, or when it helps the reader gauge a large set. Otherwise name the items directly, such as `생성 시점과 수정 시점을 기준으로 사용한다` instead of `기준은 둘이다`.
- When no one in the text claimed A, write the reason B holds and the decision that follows instead of `A가 아니라 B다`. Keep the contrast when it defines two things readers actually confuse.
- When creating a PR with a Korean title, end the title with a concise action noun such as `분리`, `추가`, `수정`, or `제거`, rather than a sentence ending such as `분리한다`. Attach Korean particles directly to the preceding word, including English identifiers. Example: `feat(core)!: schema와 composite의 summary를 doc에서 분리`.

## Final review

- Review substantive prose once for accuracy, clarity, and consistency, then stop polishing unless a material issue remains.
