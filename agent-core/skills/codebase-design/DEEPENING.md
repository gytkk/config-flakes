# Deepening

Use this reference when consolidating related modules or changing dependency seams. It uses the vocabulary in [SKILL.md](SKILL.md).

## Select what belongs together

Start from a caller's task and the invariant it must preserve. Move repeated orchestration behind an interface only when the operations share responsibility and change together. Keep independently owned policies, resource lifetimes, and deployment constraints explicit. Being in-process alone does not justify merging modules.

Show the caller before and after. Name the decisions the new module owns, the decisions callers retain, and any changed failure or transaction behavior. Consolidation should remove coordination, not merely move it into a larger file.

## Choose dependency treatment

Use the repository's existing integration and test facilities before adding ports or substitutes. Choose based on the behavior the test must establish:

| Dependency | Starting point | Verification limit |
| --- | --- | --- |
| In-process computation or state | Exercise the new interface directly; retain focused algorithm tests where useful. | Shared state, concurrency, and lifecycle constraints still matter. |
| Filesystem or database | Use a temporary real resource or an existing compatible local test service. | An in-memory substitute may differ in transactions, queries, locking, permissions, or durability. Verify relied-on semantics against the real dependency. |
| Remote service owned by the team | Reuse its client or transport seam; inject it if tests need controlled responses or failures. | A fake verifies consumer logic, not wire compatibility or the deployed service. Keep contract or integration checks for that boundary. |
| Third-party service | Isolate the operations the module uses behind an existing client seam or a narrow port when needed. | Mocks establish behavior under assumed responses. Verify serialization and failure mapping with existing fixtures, contract checks, or an authorized sandbox. |

Keep dependency wiring at the existing composition point. An internal seam can support tests without becoming an extra choice every public caller must configure. A port is justified by the control or isolation it provides; creating a second adapter solely to justify it adds no value.

## Preserve behavior during migration

1. Identify affected callers and existing tests for the behavior being moved, including errors and dependency interactions. Note any coverage gaps before removing code.
2. Add or adapt tests through the proposed interface for the preserved contract. Include failure, state, or concurrency cases when the moved responsibility depends on them.
3. Migrate callers in a bounded change and remove wrappers made obsolete by it. Preserve public compatibility when required; identify any necessary contract change explicitly.
4. Delete an old test only when its behavior is covered at the new interface or the behavior was intentionally removed. Keep unique regression cases, complex algorithm tests, and adapter integration coverage. A test that changes during refactoring is a reason to inspect its purpose, not automatic evidence that it is waste.

Finish with evidence that callers no longer coordinate the moved responsibility and that the preserved contracts still hold. If real dependency checks cannot run, distinguish what substitutes verified from what remains unverified.
