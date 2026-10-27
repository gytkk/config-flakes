# Design it twice

Use this reference when the user asks for alternative interfaces or materially different designs remain plausible. Use the vocabulary in [SKILL.md](SKILL.md) and preserve the repository's domain terminology.

## Fix the problem before varying the design

State the caller scenarios, invariants, dependencies, compatibility constraints, and the specific uncertainty alternatives should resolve. Anchor existing behavior to code locations. Each design must solve the same problem with the same required behavior.

## Explore meaningful alternatives

Compare the current design with two plausible approaches when that helps the decision. Vary responsibility or seam placement, such as keeping caller orchestration versus moving an invariant into a cohesive operation. Do not invent extension requirements merely to make an alternative different.

Use independent subagents when independent proposals could change the decision and useful comparison work can proceed concurrently. Give each a bounded, read-only brief with the same constraints, relevant paths, and a different design question. Use only as many workers as the unresolved questions justify. If delegation is unavailable or adds little value, explore the alternatives sequentially and identify them as one agent's analysis.

For each approach, provide:

- An interface sketch and representative caller example.
- The responsibility it hides and the knowledge callers still need.
- Dependency control, state ownership, and relevant failure behavior.
- Migration scope and the tests that would distinguish it from the other choices.

## Recommend with evidence

Compare caller knowledge, locality of changes, seam placement, and migration cost. A compact table helps when the same criteria apply to every alternative. Prefer the smallest design that satisfies the actual scenarios, including keeping the current design when change brings no demonstrated benefit.

End with a recommendation and any unresolved assumption that could change it. Propose a focused check for that assumption instead of treating an untested sketch as proof. A request to compare designs ends with the comparison; implementation follows only within the user's requested scope.
