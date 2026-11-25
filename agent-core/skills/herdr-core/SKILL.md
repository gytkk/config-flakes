---
name: herdr-core
description: Start, message, wait on, and read programs and coding agents in persistent herdr-core terminals with `hc`. Use when the user explicitly requests Herdr or hc, or when the runtime's instructions select hc for parallel tasks.
---

# herdr-core

This file is a pointer. The manual lives in the installed `hc` so it always matches that version. Load it before acting:

```bash
hc --skill
```

Run `herdr-app --skill` as well when the task changes what the person's Herdr window shows.

`$HC_TERMINAL_ID` is your terminal's address. If it is unset, or `PI_SUBAGENT_CHILD=1`, you have no herdr-core terminal of your own: stop and report that to the user.

## Precedence

The manual covers mechanics: how to start, message, wait on, and read terminals. Your runtime's instructions decide whether to delegate and what a child may do, and they win wherever the two disagree. For agents started with `hc`:

- Make `hc` the only launcher for the task, so `hc wait --children` sees every child.
- Write each child's task from `references/contracts.md` in the `parallel-research-merge` skill directory, and tell the child to start no further agents.
- Give concurrent implementers separate worktrees, with one writer per checkout.
- Account for every child as completed, blocked, failed, or cancelled before integrating its work.
