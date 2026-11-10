# devils-advocate

Anti-sycophantic multi-pass code review plugin for Claude Code.

## Overview

Three sequential review passes with enforced skeptical re-checks.
Cannot say "looks good" — the default verdict is `needs_work`, and the
reviewer must argue FOR approval without fabricating findings.

### Key Mechanisms

- **Standards Discovery**: Reads CLAUDE.md, ADRs, and detects dominant patterns (5+ instances) before review
- **Context Gate**: Refuses to review when context is insufficient — no low-confidence reviews
- **Anti-sycophancy**: Required re-checks, banned praise phrases, default-deny verdict
- **Unverified section**: Each pass must admit what it did NOT check — mandatory honesty

## Command

```text
/devils-advocate:review [files or description] [--base <branch>] [--quick]
```

### Flags

| Flag              | Description                                          |
| ----------------- | ---------------------------------------------------- |
| `--base <branch>` | Compare HEAD against a base branch                   |
| `--quick`         | Run Pass 1 only (architecture) with target 1 finding   |

### Review Passes

| Pass | Focus                    | Target          |
| ---- | ------------------------ | --------------- |
| 1    | Architecture & Design    | 2 (1 if quick)  |
| 2    | Maintainability          | 2               |
| 3    | Edge Cases & Assumptions | 1 if defensible |

### Output

- **Verdict**: `approve` | `needs_work` | `reject`
- **Per-pass findings** with severity, `file:line`, risk, impact, and recommendation
- **Top concerns** prioritized by severity
- **Improvements** prioritized by impact
- **Not Verified** — aggregated blind spots from all passes
- Saved to `~/.ai/review-{SESSION_ID}-result.json`

## Prerequisites

None. Uses Claude-native tools only (Read, Grep, Glob, Bash, Agent).

## Structure

```text
skills/devils-advocate/
├── .claude-plugin/plugin.json
├── commands/review.md
├── agents/devils-advocate-agents.md
└── references/
    ├── review-schema.json
    └── rubrics.md
```
