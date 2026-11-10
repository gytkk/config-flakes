# Review Rubrics

Per-pass evaluation criteria for the devil's advocate multi-pass code review.

## Standards Discovery (Pre-Review)

Before evaluation begins, the orchestrator discovers project standards:

| Source                | What to Extract                                                          |
| --------------------- | ------------------------------------------------------------------------ |
| CLAUDE.md / AGENTS.md | Documented coding standards, architectural rules, conventions            |
| ADR files             | Active architecture decisions and their rationale                        |
| Dominant patterns     | If 5+ instances follow the same convention → that IS the standard        |
| Boundary markers      | Barrel exports, API clients, repository patterns, service layers         |

Deviations from discovered standards are findings — even if the code is
"correct" in isolation. The codebase's conventions take precedence over
textbook best practices.

## Pass 1: Architecture & Design Fitness

| Criterion              | What to Check                                                            |
| ---------------------- | ------------------------------------------------------------------------ |
| Dependency direction   | Dependencies flow inward; no circular dependencies introduced            |
| Abstraction level      | Not over-engineered (YAGNI) and not under-abstracted (copy-paste)        |
| Pattern consistency    | Follows patterns established in THIS codebase, not textbook patterns     |
| Module boundaries      | Changes respect existing module boundaries; no boundary violations        |
| API contracts          | Public interfaces remain stable; breaking changes are intentional        |
| Separation of concerns | Each module/function has one clear responsibility                        |
| Standards compliance   | Matches discovered conventions, ADR decisions, and dominant patterns      |

Target: identify at least 2 defensible findings. Re-examine once if fewer are
found; do not fabricate findings.

## Pass 2: Future Maintainability & Tech Debt

| Criterion            | What to Check                                                            |
| -------------------- | ------------------------------------------------------------------------ |
| 6-month test         | Will a new team member understand this code in 6 months?                 |
| Coupling             | What breaks if requirements change? How many files must change together? |
| Cohesion             | Does each unit do one thing well, or is it a grab bag?                   |
| Open-closed          | Can the change be extended without modifying existing code?              |
| Implicit assumptions | What must remain true for this code to work? Is it documented?           |
| Magic values         | Hardcoded strings, numbers, or config that should be parameterized       |
| Error messages       | Do error messages help debugging, or are they generic/missing?           |

Target: identify at least 2 defensible findings. Re-examine once if fewer are
found; do not fabricate findings.

## Pass 3: Hidden Assumptions & Edge Cases

| Criterion            | What to Check                                                            |
| -------------------- | ------------------------------------------------------------------------ |
| Implicit contracts   | What must callers/callees guarantee that is not enforced in code?         |
| Error scenarios      | What errors are unhandled? What happens on partial failure?              |
| Concurrency          | Race conditions, deadlocks, ordering assumptions under parallel exec     |
| Input validation     | What inputs are assumed valid but not validated?                          |
| External failures    | What if a network call, file operation, or DB query fails?               |
| Resource leaks       | Are files, connections, locks, and transactions properly cleaned up?      |
| Boundary conditions  | Off-by-one, empty collections, null/undefined, max-size inputs           |

Completion target: identify at least 1 edge-case finding when defensible.
Re-examine once if none is found. If no edge-case finding is defensible, report
that the pass did not produce material findings and list the concrete
edge-case areas that remain unverified rather than inventing one.

## Verdict Criteria

| Verdict    | When to Use                                                                          |
| ---------- | ------------------------------------------------------------------------------------ |
| approve    | All findings are minor/info, confidence >= 8, and all passes completed               |
| needs_work | Default. Any major finding, confidence < 8, incomplete context, or missed pass target |
| reject     | Any high-confidence critical finding, or a fundamental design flaw                   |

Default verdict is `needs_work`. The reviewer must argue FOR approval, not against it.
A completed pass may have no findings only when the report states the re-check
and remaining unverified areas.
