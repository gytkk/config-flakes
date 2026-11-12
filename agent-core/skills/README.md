# Shared skills

This directory owns local shared skills and retains legacy vendored files during host migration. `agent-core/manifest.toml` declares local and external sources and CLI target selections. See [installation and migration status](../README.md) before removing legacy sources.

New installations acquire the Matt skills selected under the external source in `agent-core/manifest.toml` and Vercel's `find-skills` directly from the latest default-branch SHA resolved once per operation. `resolving-merge-conflicts` was removed upstream and is excluded from new installations with user approval. Imported directories still selected from external sources remain for legacy rendering and recovery until all host gates pass. Locally selected skills remain canonical here. Each external installation preserves its own revision's root license separately; the legacy license below does not replace those notices.

[`impeccable`](https://github.com/pbakaus/impeccable/tree/main/.agents/skills/impeccable) is acquired directly from `pbakaus/impeccable` for the shared Codex/Pi/OMP pool. Claude uses its separate official marketplace plugin, so the Claude CLI target excludes this skill. The installer resolves and records its revision and preserves that revision's license; no vendored copy is maintained here.

[`hunk-review`](https://github.com/modem-dev/hunk/tree/main/.agents/skills/hunk-review) is acquired directly from `modem-dev/hunk` for Codex/Pi/OMP and Claude. It delegates to the installed Hunk CLI's version-matched instructions for inspecting and annotating live diff review sessions. The installer resolves the latest default-branch SHA once per operation and preserves upstream's MIT license; no vendored copy is maintained here.

## Import provenance

The legacy import from [mattpocock/skills](https://github.com/mattpocock/skills) contains `code-review`, `codebase-design`, `diagnosing-bugs`, `domain-modeling`, `grilling`, `prototype`, `research`, `resolving-merge-conflicts`, `tdd`, `wizard`, and `writing-for-agents`. This is an import record, not the current external installation selection.

- Upstream version: `1.2.3`
- Upstream commit: `6654f6b60cd9d5be8b54c6fafe44346dabeb3b76`
- Retrieved: `2026-08-29`

The original import excluded user-invoked wrappers, setup workflows, plugin manifests, and runtime-specific UI metadata. It retained each selected `SKILL.md` and directly referenced files, with discovery metadata in frontmatter. Subsequent local changes are canonical for the skills selected under `skill_sources.local`; external installations use the selected upstream revision, including its metadata, rather than this legacy snapshot. `code-review` discovers repository configuration without the removed setup skill. The upstream MIT license is included in [LICENSE](LICENSE) and applies to the imported skills and their local adaptations.

`pi-agent` is an active local-source import from [K-Dense-AI/scientific-agent-skills](https://github.com/K-Dense-AI/scientific-agent-skills/tree/ef1b2d8026bdccccf8a72f22bf3f2ea9eda76df4/skills/pi-agent), with metadata version `1.2` and its own [K-Dense MIT license](pi-agent/LICENSE.md). The manifest selects it for the shared CLI target and the legacy Pi renderer; it is not a retired external fallback. No original import commit or retrieval date is recorded. Commit `ef1b2d8026bdccccf8a72f22bf3f2ea9eda76df4` is a historical comparison baseline. The migration audit found 33 of 34 reference documents byte-identical at that time; the local skill moved Node.js/npm requirements into the body and used updated model examples. Its license matched the upstream repository's root license. The later audited snapshot [`a92663a0214b3b9fa1ba6b958284ffd919c00a0c`](https://github.com/K-Dense-AI/scientific-agent-skills/tree/a92663a0214b3b9fa1ba6b958284ffd919c00a0c/skills/pi-agent) changed the skill and reference set; adopting it would be a separate content upgrade. These historical comparisons and reference package versions do not describe the current runtime pins; use the Pi package and module sources for deployed versions. Other skills are maintained locally unless the manifest declares an external source.

## Current-use corrections

The all-host migration gate preserves legacy-rendered resources and their existing hashes until approved cleanup. It does not make every local skill immutable or establish that retained API and runtime claims are current. In particular, `pi-agent` and `openai-decisions` are active local sources that also participate in legacy rendering. These notes qualify known stale snapshot claims without changing their protected renderer output.

- **Decisions image inputs:** The base64-only restriction in [openai-decisions](openai-decisions/SKILL.md#request-and-answer-contract) is obsolete. The [current official Decisions guide](https://developers.openai.com/api/docs/guides/decisions) accepts inline base64 data URLs or publicly accessible HTTP or HTTPS URLs; it does not support `file_id` inputs. Follow that guide and the consuming project's SDK version when constructing requests.
- **Pi runtime authority:** The retained `pi-agent` reference set includes older package baselines and `pi-mono` source paths. The deployed Pi package comes from `earendil-works/pi`, as declared in [the package definition](../../packages/apps/pi/package.nix); extension versions and rollout policy come from [the Pi module](../../modules/pi/README.md) and [settings](../../modules/pi/files/settings.json). Use the selected package's source and documentation for current behavior rather than treating the imported snapshot's package examples or old repository paths as current configuration.

## Local adaptations

[korean](korean/SKILL.md) restates the previous assistant answer in natural Korean while preserving its meaning, uncertainty, code, and links.

[diagnosing-bugs](diagnosing-bugs/SKILL.md) is maintained locally to support evidence-based investigation when reproduction is unavailable, bounded intermittent and performance checks, and diagnosis-only requests. The next successful switch backs up and replaces installations still recorded under `matt` with this local source. First-time legacy path ownership still requires the explicit handoff described in [Agent core](../README.md#skill-설치와-복구).

[codebase-design](codebase-design/SKILL.md) is maintained locally to connect caller evidence to module designs, preserve behavior during deepening, and compare alternatives in proportion to the decision. It also serves vocabulary-only requests from other skills. For example, ask it to find repeated caller coordination in a named module, compare two interface designs, or identify a seam that would make failure behavior testable. The same switch-time backup and replacement applies to existing `matt` installations.

[research](research/SKILL.md) is maintained locally to follow runtime delegation limits and return findings in the requested form. It does not require background execution or repository notes. The same switch-time backup and replacement applies to existing `matt` installations. The shared selection applies this workflow to Codex, Claude, Pi, and OMP; each runtime retains its own delegation policy.

## Quokka adaptations

The following guidance is independently rewritten from workflow ideas reviewed in [devsisters/quokka](https://github.com/devsisters/quokka/tree/4e39746192faed7e106f4d446884a4314bccbf37) on 2026-10-07. The reviewed snapshot is commit `4e39746192faed7e106f4d446884a4314bccbf37` on `develop`. No Quokka implementation, bundled library, third-party skill text, or company schema is copied.

| Source at the reviewed commit | Local selection and reason |
| --- | --- |
| `src/quokka/agents/main_agent_prompts.py`, `agent_context/main/README.md`, `agent_context/skills/core-kpi/SKILL.md` | [data-analysis](data-analysis/SKILL.md): define metric grain and base components, check joins and distinct counts, and verify coverage and totals. These checks add a data workflow beyond general evidence rules. |
| `agent_context/skills/causal-inference/references/reporting-guide.md` | `data-analysis`: distinguish invalid comparisons from inconclusive effects and disclose changes to the analyzed population. The private estimator, installation procedure, and fixed statistical thresholds are excluded. |
| `agent_context/skills/data-visualization/SKILL.md` | [data-visualization](data-visualization/SKILL.md): plot from source data, check scales and units, and inspect fonts and final rendering. Fixed palettes, sandbox paths, and automatic replacement of requested chart types are excluded. |
| `.agents/skills/quokka-skill-author/references/trigger-eval.md`, `.agents/skills/quokka-eval-creator/SKILL.md` | [writing-for-agents validation](writing-for-agents/BEHAVIOR-VALIDATION.md): positive and near-miss cases, successful load evidence, setup failures, and impact on existing cases. The Quokka CLI harness and live-query side effects are excluded. |

All three selections are exposed to Codex, Claude, Pi, and OMP through the manifest and CLI installation. They add no always-loaded operating rules, runtime dependencies, credentials, or mutable workspace configuration.

The selection excludes game and company data skills, Quokka administration and deployment skills, and library-specific frontend skills because their tools and schemas do not apply generally. PR and commit workflows overlap existing operating rules. Usage-report analysis overlaps `user-friction-review` and requires Quokka's thread APIs. Mandatory designer interviews and fixed report sections are not adopted because they would impose unnecessary steps on unrelated work.

Shared skills describe required capabilities instead of naming one agent runtime. Runtime modules own packages, settings, plugins, and installation behavior. Agent-core validates source declarations and the pinned CLI acquires and places skill files.
