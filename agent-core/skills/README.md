# Shared skills

This directory owns local shared skills and retains legacy vendored files during host migration. `agent-core/manifest.toml` declares local and external sources and CLI target selections. See [installation and migration status](../README.md) before removing legacy sources.

New installations acquire the Matt skills selected under the external source in `agent-core/manifest.toml` and Vercel's `find-skills` directly from the latest default-branch SHA resolved once per operation. `resolving-merge-conflicts` was removed upstream and is excluded from new installations with user approval. Imported directories still selected from external sources remain for legacy rendering and recovery until all host gates pass. Locally selected skills remain canonical here. Each external installation preserves its own revision's root license separately; the legacy license below does not replace those notices.

[`hunk-review`](https://github.com/modem-dev/hunk/tree/main/.agents/skills/hunk-review) is acquired directly from `modem-dev/hunk` for Codex/Pi and Claude. It delegates to the installed Hunk CLI's version-matched instructions for inspecting and annotating live diff review sessions. The installer resolves the latest default-branch SHA once per operation and preserves upstream's MIT license; no vendored copy is maintained here.

The skills imported from [mattpocock/skills](https://github.com/mattpocock/skills) are `code-review`, `codebase-design`, `diagnosing-bugs`, `domain-modeling`, `grilling`, `prototype`, `research`, `resolving-merge-conflicts`, `tdd`, `wizard`, and `writing-for-agents`.

- Upstream version: `1.2.3`
- Upstream commit: `6654f6b60cd9d5be8b54c6fafe44346dabeb3b76`
- Retrieved: `2026-08-29`

The imported selection excludes user-invoked wrappers and setup workflows. Each retained directory contains its `SKILL.md` and any file that it directly references. Plugin manifests and runtime-specific UI metadata are excluded; required discovery metadata remains in `SKILL.md` frontmatter. `code-review` discovers repository configuration without the removed setup skill. The upstream MIT license is included in `LICENSE` and applies to the imported skills.

`pi-agent` is a separate import with its own [Pi skill license](pi-agent/LICENSE.md). The other skills are maintained locally.

[korean](korean/SKILL.md) restates the previous assistant answer in natural Korean while preserving its meaning, uncertainty, code, and links.

[diagnosing-bugs](diagnosing-bugs/SKILL.md) is maintained locally to support evidence-based investigation when reproduction is unavailable, bounded intermittent and performance checks, and diagnosis-only requests. Existing installations recorded under the `matt` source need the reviewed ownership handoff described in [Agent core](../README.md#skill-설치와-복구) before they can use the local source.

[codebase-design](codebase-design/SKILL.md) is maintained locally to connect caller evidence to module designs, preserve behavior during deepening, and compare alternatives in proportion to the decision. It also serves vocabulary-only requests from other skills. For example, ask it to find repeated caller coordination in a named module, compare two interface designs, or identify a seam that would make failure behavior testable. Existing `matt` installations require the same reviewed ownership handoff.

[research](research/SKILL.md) is maintained locally to follow runtime delegation limits and return findings in the requested form. It does not require background execution or repository notes. Existing `matt` installations require the reviewed ownership handoff described above. The shared selection applies this workflow to Codex, Claude, and Pi; each runtime retains its own delegation policy.

## Quokka adaptations

The following guidance is independently rewritten from workflow ideas reviewed in [devsisters/quokka](https://github.com/devsisters/quokka/tree/4e39746192faed7e106f4d446884a4314bccbf37) on 2026-10-07. The reviewed snapshot is commit `4e39746192faed7e106f4d446884a4314bccbf37` on `develop`. No Quokka implementation, bundled library, third-party skill text, or company schema is copied.

| Source at the reviewed commit | Local selection and reason |
| --- | --- |
| `src/quokka/agents/main_agent_prompts.py`, `agent_context/main/README.md`, `agent_context/skills/core-kpi/SKILL.md` | [data-analysis](data-analysis/SKILL.md): define metric grain and base components, check joins and distinct counts, and verify coverage and totals. These checks add a data workflow beyond general evidence rules. |
| `agent_context/skills/causal-inference/references/reporting-guide.md` | `data-analysis`: distinguish invalid comparisons from inconclusive effects and disclose changes to the analyzed population. The private estimator, installation procedure, and fixed statistical thresholds are excluded. |
| `agent_context/skills/data-visualization/SKILL.md` | [data-visualization](data-visualization/SKILL.md): plot from source data, check scales and units, and inspect fonts and final rendering. Fixed palettes, sandbox paths, and automatic replacement of requested chart types are excluded. |
| `.agents/skills/quokka-skill-author/references/trigger-eval.md`, `.agents/skills/quokka-eval-creator/SKILL.md` | [writing-for-agents validation](writing-for-agents/BEHAVIOR-VALIDATION.md): positive and near-miss cases, successful load evidence, setup failures, and impact on existing cases. The Quokka CLI harness and live-query side effects are excluded. |

All three selections are exposed to Codex, Claude, and Pi through the manifest and CLI installation. They add no always-loaded operating rules, runtime dependencies, credentials, or mutable workspace configuration.

The selection excludes game and company data skills, Quokka administration and deployment skills, and library-specific frontend skills because their tools and schemas do not apply generally. PR and commit workflows overlap existing operating rules. Usage-report analysis overlaps `user-friction-review` and requires Quokka's thread APIs. Mandatory designer interviews and fixed report sections are not adopted because they would impose unnecessary steps on unrelated work.

Shared skills describe required capabilities instead of naming one agent runtime. Runtime modules own packages, settings, plugins, and installation behavior. Agent-core validates source declarations and the pinned CLI acquires and places skill files.
