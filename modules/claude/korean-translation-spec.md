# Korean conversation translation mod specification

Status: implemented as the `korean-translation` plugin in the `gytkk` marketplace and checked with stubbed mod tests and an isolated PTY smoke run that uses a stub model. Live model behavior is unverified: the live terminal acceptance below has not been run. This document specifies a Claude Code mod and does not authorize deployment; [the Claude module README](README.md#korean-translation-mod) documents the implemented mod. The implementation target is Claude Code 2.1.325, matching the installed CLI and the pin in `packages/apps/claude-code/package.nix` when this specification was written.

## Outcome and scope

In an interactive parent terminal session, the user writes Korean, Sonnet translates the request into English, and the working model receives the English request. The working model's English answer streams normally. After the turn completes, Sonnet translates the final answer into Korean and the interface shows it below the English original. Translation does not change the selected working model.

The design follows the context separation of the [Pi translation extension](../pi/README.md#korean-conversation-translation), but uses Claude Code's native mod APIs and existing session credentials. It does not import Pi code or read Pi credentials or session state.

The first release covers ordinary typed prose in the parent terminal session and the final conversational answer. It excludes intermediate commentary, thinking, tool calls/results, file contents, system instructions, subagents, teammates, scheduled or injected prompts, and historical-message backfill. Images and attachments remain attached to the working-model request but are not sent to Sonnet. Desktop, IDE panels, Remote Control-specific presentation, cloud sessions, Agent SDK, and non-interactive runs are outside the initial supported surface; the mod must not translate unattended sessions merely because its plugin loads there.

## User-visible behavior

### Input

1. Detect Hangul syllables and Jamo in unprotected prose. English-only input and input consisting only of protected literals pass through without a translation request.
2. Show a cancellable translation-in-progress indication while withholding the prompt from the working model.
3. Translate Korean and mixed Korean/English prose into English. Preserve meaning, uncertainty, constraints, and requested artifact languages. Treat the supplied text as translation data, not instructions for the translator to execute.
4. Preserve the Korean original in a separate display-only entry, then forward the English text using `next({ ...e, text: translatedText })`. Preserve all non-text event fields.
5. Keep the native user message as English. The original display entry must not enter model context or compaction input. Document that native prompt history contains the rewritten English text; display preservation does not preserve the original editor-history entry.

For the first release, slash commands and shell-mode input pass through unchanged, including their arguments. Command parsing and skill expansion are not translation responsibilities. Prompts whose origin cannot be distinguished from automation must not be rewritten; establishing a reliable origin filter is a release gate.

### Output

After a successful parent turn with non-empty final prose, translate `turn.complete`'s `e.answer` into Korean once. Keep the English assistant answer unchanged and append a readable Korean display entry beneath it. A divider may separate the languages; native plugin attribution is acceptable. Do not replace the English answer or submit the translation as another prompt.

Translate the final answer even when the user's input was English. Skip interrupted turns, subagent turns, empty answers, and answers containing no translatable prose. Use the enable state captured for the originating turn, except that turning translation off cancels pending work and prevents its display. Turning translation on does not backfill an earlier turn.

Display text must wrap correctly and preserve paragraphs and lists. Long answers must remain readable in classic and fullscreen terminal rendering. A dim log line is acceptable for short status messages, but is not sufficient evidence that long translation output renders correctly.

### Controls and lifecycle

Expose `/translate on`, `/translate off`, and `/translate status` without starting a working-model turn. With no argument, report status; invalid arguments show usage without changing state. Status identifies whether translation is enabled, the `sonnet` model alias, the supported terminal scope, and whether a translation is pending. Resolve any command-name collision before release rather than replacing another plugin's command.

The first release defaults to enabled when the mod loads in a supported interactive session. The toggle is memory-only and returns to enabled when the mod reloads or the session is replaced. Announce the effective state on load so reload does not silently undo an opt-out. Durable per-session preferences and replay of saved translations are deferred; do not promise Pi's branch-persistent behavior.

On interruption, disabling, reload, clear, resume, branch change, or session end, invalidate pending work and discard late results. An old translation must never appear under a new session or a different turn. Re-rendering the UI must not make new translation calls. Use session/turn identity and a generation token for pending operations, with at most one input translation in flight per session. Output translations run one at a time from a per-session queue holding at most four pending answers; a new submission does not cancel an earlier answer's translation, because each translation is displayed in its own row tied to that answer. Interruption, disabling, and session end cancel the queue.

## Native mod integration

| Surface | Responsibility | Constraint |
| --- | --- | --- |
| `session.start` | Register controls and report effective state | Loading the mod does not make a model call |
| `prompt.submit` | Filter eligible input, translate it, and rewrite only its text | Call `next` once on success; do not re-submit through `$.prompt.submit` |
| `$.model.complete` | Translate with `model: 'sonnet'` | Use a standalone prompt and translator system instruction, not `$.model.fork` or conversation history |
| `turn.complete` | Translate the eligible parent turn's final `e.answer` | Skip events with `e.agentId` or `e.isAborted`; do not trigger another working-model turn |
| `$.ui.place` with a `ui.render` handler for `Placed` | Preserve original input and show Korean output in display-only rows | Verify context exclusion and long-text rendering on the target build |
| `command.run` | Handle translation controls | No model calls for controls |
| `session.end` and hook cancellation | Invalidate work and clear pending state | Handle replacement and reload as well as process exit |

The documented `turn.complete` result `{ text }` is a candidate for displaying the translation beneath the answer. Confirm its context and persistence behavior before selecting it. `$.ui.log` is explicitly documented as unread by Claude, but its dim-line presentation may require a different documented UI surface for long text. Do not fall back to editing transcript JSONL, writing terminal escape sequences, injecting assistant messages, or using `Stop` feedback to make the main model translate.

Traditional `UserPromptSubmit` settings hooks add context or block a prompt; they do not document an original-prompt replacement field. `MessageDisplay` transforms streaming display batches rather than defining the desired once-per-turn workflow. Neither is the primary integration for this mod.

Keep mods API calls statically discoverable by Claude's validator. Use literal event names, explicit `$.namespace.method` calls, and plugin-local imports. Pure protection/validation helpers can be separate modules, but an imported helper must not receive the mods API object in violation of the host's static-analysis rules. Do not add a bundler or a separate Claude subprocess for translation.

## Translation contract and failures

Use the `sonnet` alias through `$.model.complete`, with session-native authentication and an explicit output-token limit sufficient for the supported input size. If Sonnet is unavailable or organization policy denies it, report the failure without switching models or extracting credentials. Translation consumes the user's plan/API/cloud-provider usage. Do not claim it is free or that native cost totals include it unless verified on the target build.

Before calling Sonnet, replace fenced code, inline code, recognized URLs, and recognized path tokens with collision-safe placeholders. Restore each literal byte-for-byte only after every expected placeholder appears exactly once and no unexpected placeholder appears. Keep identifiers and commands inside code formatting unchanged. Document the limits of path recognition, especially paths containing spaces or non-ASCII characters. These checks protect literal bytes, not translation quality or the relationship between prose and literals.

Each direction accepts at most 64 KiB of UTF-8 source text and has a 60-second total deadline, including any retry. Do not truncate source text to fit. Configure the host hook deadline so its default timeout does not preempt the translation deadline. Propagate `next.signal` through supported cancellation mechanisms and reject late results even if the provider cannot cancel the remote request.

Check both rejected calls and `isAnswered: false` results. Reject empty, detectably truncated, or malformed responses. Validate target-language prose as well as placeholder integrity: after excluding protected literals and translation labels, reject an English-to-Korean result that copies the English source unchanged or contains no Hangul despite a source containing English conversational sentences. A Korean heading alone does not satisfy this check. Do not impose a fixed Hangul percentage, since valid technical explanations can contain many English identifiers. Also reject an English-to-Korean result that leaves an English sentence of eight or more words unchanged, and a result in either direction whose Markdown block structure differs from the source: list markers, ordered-list numbers, checkbox states, nesting indentation, heading levels, blockquote depth, table rows and columns, and code fences must match. These deterministic checks catch obvious non-translations and flattened structure; they do not prove that every sentence was translated or that its meaning is correct.

One retry is allowed for a placeholder or structure failure or an obvious non-translation, starting from the original protected source with a reminder about the failed requirement and using the remaining deadline. When only placeholders failed and the reply has the same number of non-blank lines as the source, the retry sends only the lines whose placeholders were wrong, names those placeholders, and keeps the other translated lines. A retried line that is still wrong stays in the source language while the rest of the translation is accepted; if no retried line is repaired, the translation fails. The retry limit is shared across both validation failures, not one retry per failure type. Other failures are not retried. If the retry still fails, follow the input/output failure behavior below rather than displaying the unchanged English body as a Korean translation. Output limits that cannot accommodate the full translation must produce an explicit failure, not a partial translation presented as complete.

| Failure | Required result |
| --- | --- |
| Input timeout, provider error, invalid translation, or oversize source | Drop submission with a short explanation and restore the Korean draft; the user can disable translation and resubmit |
| Input cancellation | Do not send a request to the working model; preserve the draft without overwriting newer user edits |
| Output timeout, provider error, invalid translation, or oversize source | Keep the English answer and show a bounded warning; do not ask the working model to repair it |
| Output cancellation or stale session/turn | Keep the English answer and suppress the obsolete translation |
| Hook throw or host timeout | Use the documented hook `.catch` behavior to prevent untranslated input from being sent; do not rely solely on an inner `try/catch` |
| Mod unavailable, disabled by policy, or not loaded | Leave ordinary Claude behavior intact; no guarantee of translation applies |

Do not log raw prompts, answers, credentials, or provider error payloads for diagnostics. Send only the protected text and translation instructions to Sonnet. Keep pending text in memory and avoid a separate translation database. Display-only means excluded from model context, not necessarily absent from Claude's saved transcript or logs; verify and document actual retention behavior before release.

## Response-language policy

The current `agent-core/adapters/claude.md` requires the working model to produce English plus Korean for longer messages. Enabling this mod without coordinating that instruction would duplicate translations.

The implementation must revise the canonical Claude adapter, not generated runtime files, so the existing bilingual rule remains the fallback when no translation-layer marker is present. For eligible turns while the mod is enabled, add an explicit context marker saying that a separate display layer translates conversational responses and the working model should write them in English. Preserve languages explicitly requested for files, quotations, and artifacts.

The active/inactive state must be unambiguous on every relevant turn so an old marker cannot keep English-only mode active after `/translate off`. The inactive state restores ordinary language policy. Unsupported surfaces and unloaded mods must retain the fallback rule. Validate this transition in a multi-turn session; a single successful English answer is not sufficient evidence.

## Repository ownership and rollout

The mod belongs to the existing `gytkk` marketplace under `modules/claude/marketplace/`, not to the shared skill catalog. The proposed plugin name is `korean-translation`, installed as `korean-translation@gytkk`. Its directory will contain the standard plugin manifest, hooks module declaration, TypeScript implementation, and native mod tests. Exact new file paths are implementation details, not existing files referenced by this specification.

A later implementation change must coordinate:

- The marketplace declaration in `modules/claude/marketplace/.claude-plugin/marketplace.json` and the plugin's versioned manifest.
- The install selection in `modules/claude/default.nix` and enable selection in `modules/claude/files/settings.json`, preserving unrelated user edits and existing hooks/plugins.
- The conditional language policy in `agent-core/adapters/claude.md`, with the agent-core checks and intentional render updates required by `agent-core/README.md`.
- Maintained usage, limitations, supported Claude version, and test commands in `modules/claude/README.md`.

First validate the mod in an isolated directory/session using the target CLI, then integrate it with the existing marketplace activation. An implementation commit must not apply Home Manager, install into the user's active session, change organization controls, or push without the appropriate approval. The user runs Home Manager when ready to roll out.

Disabling the plugin stops translation without deleting sessions or credentials. The canonical fallback language policy must remain usable without the mod. Do not uninstall unrelated plugins or change the selected working model as part of rollout or rollback.

## Verification and release gates

These checks are requirements for the future implementation, not results of this documentation task.

### Automated checks

Use `claude plugin validate` against the plugin directory and the native `claude plugin test` runner with stubbed model calls and UI effects. Check the target CLI's help for invocation and directory options. Generated type declarations from the exact CLI version take precedence over online examples; obtain them in an isolated development copy so generated files do not contaminate the managed source.

Tests must cover:

- Korean and mixed-language input rewriting; English-only, protected-only, command, automation, unsupported-surface, and subagent pass-through.
- Exactly one working-model submission, preservation of non-text fields, exclusion of attachments/history from Sonnet, and absence of translation recursion.
- Placeholder restoration, collisions, missing/duplicated/unknown placeholders, Unicode byte limits, incomplete output, and the single bounded retry.
- Markdown structure: dropped or changed bullets, collapsed nesting, changed ordered numbers or checkbox states, heading, quote, and table changes are rejected, while a faithful translation of nested lists, headings, quotes, tables, and fenced code passes.
- Non-translation detection with an unchanged English answer, an English copy prefixed by a Korean translation heading, and English prose containing only protected Korean literals. A valid Korean explanation containing English technical identifiers must pass. Verify that mixed validation failures still allow only one retry.
- Input restoration and output preservation after errors, cancellation, host timeout, or a denied model call.
- Parent-turn filtering, duplicate completion delivery, no translation on UI redraw, and stale-result rejection after lifecycle changes or new input.
- Control validation, default state, reload reset announcement, and active/inactive response-language markers.
- No secrets or raw content in diagnostic messages and no mutation of unrelated hooks/settings.

### Reported failure fixture

A user-provided screenshot shows an English answer about Plane's PQL views, followed by a `한국어 번역` heading and the same English paragraphs, numbered steps, caveats, and source list. This confirms untranslated prose in a section presented as Korean; it does not establish which runtime instruction or model behavior caused it. The proposed mod is not implemented, so the screenshot is not evidence of a mod execution failure.

Use a small synthetic answer with the same structure as a regression fixture, without copying private hostnames or storing the screenshot in the repository. A stubbed translator that returns the English body unchanged, with or without the Korean heading, must trigger validation and the bounded retry. A second unchanged result must leave the native English answer visible and show a translation-failure warning, with no purported Korean translation entry.

### Live terminal acceptance

With approval for a live model call, use a disposable Claude session and check:

1. A Korean request containing code and a path shows its original separately and submits an English native user message. Attached images remain available to the working model but are absent from the translation request.
2. The selected main model streams English normally. After tool execution and the final answer, exactly one complete Korean translation appears beneath the original, including readable multiline lists and code. Compare every prose paragraph, numbered step, caveat, and ordinary heading with the English source; a Korean label or one translated sentence does not establish full coverage. Keep code, exact identifiers, URLs, and official names unchanged where required.
3. The working model's next request and compaction input exclude original-input display entries and Korean output translations. Verify request/context data rather than asking the model whether it saw them.
4. `/translate off`, `/translate on`, interruption, reload, clear, and resume produce the specified behavior without duplicate entries or changes to a newer draft.
5. Non-interactive runs and subagents make no translation calls. Existing Herdr hooks and other installed mods continue to operate.
6. Classic and fullscreen rendering preserve readable English and Korean. Inspect what remains after resume and document actual display-entry retention without making backfill calls.

Before implementation is considered complete, resolve the target build's eligible-input origin fields, interactive-surface detection, full-text display API, cancellation/deadline controls, lifecycle identifiers, and model-result truncation signals. If any required distinction or behavior is unavailable, report that specific limitation and seek a scope decision rather than claiming Pi-equivalent behavior or adding an undocumented workaround.

## Sources

These official pages establish the proposed mechanisms, not proof that this mod works. The earlier Context7 lookup was quota-limited; direct Anthropic documentation was used instead.

- [Mod creation, version-matched types, validation, and static-analysis restrictions](https://code.claude.com/docs/en/plugins/mods/create)
- [Prompt rewriting, turn completion, and failure handling](https://code.claude.com/docs/en/plugins/mods/events)
- [Independent model calls, session credentials, display-only logging, and cancellation](https://code.claude.com/docs/en/plugins/mods/api)
- [Event and API reference](https://code.claude.com/docs/en/plugins/mods/reference)
- [Native mod tests and model stubs](https://code.claude.com/docs/en/plugins/mods/test)
- [Traditional hook contracts, including UserPromptSubmit and MessageDisplay](https://code.claude.com/docs/en/hooks)
