---
name: codebase-design
description: Design module interfaces and seams from concrete callers and dependencies. Use to find deepening opportunities, compare interface designs, or improve structural testability and code navigation. Also supplies shared deep-module vocabulary to other skills. Routine fixes and mechanical refactors do not need this workflow unless the module's shape is in question.
---

# Codebase design

Design deep modules: useful behavior behind an interface that callers can learn and use without reconstructing the implementation. Judge a design by the knowledge and coordination it removes from callers, not by method counts or implementation size.

## Choose the scope

- For a vocabulary question or a reference from another skill, consult the glossary and relevant principles only. Continue the calling workflow without starting a design investigation.
- For an existing module or a search for opportunities, trace a representative caller through the implementation, dependencies, and tests. Start in the requested area and expand only along relevant connections.
- For a new module, ground the design in required caller scenarios and existing neighboring conventions. Label proposed behavior separately from observed behavior.
- For a design-only request, finish with a recommendation. For an implementation request, carry the chosen design through affected callers and checks within the user's authorized scope.

## Work from evidence to a design

1. **Locate the cost.** Read applicable context and domain terminology, then follow the relevant call path. Identify what callers repeatedly coordinate: ordering, configuration, data conversion, error handling, or shared invariants. Record concrete symbols and paths. A file count or a preference for fewer layers is not evidence of a problem.
2. **Describe the current interface.** Include inputs, results, state ownership, side effects, failures, and any ordering or performance constraints that matter to callers. Identify who owns each dependency and how tests exercise it. For opportunity searches, rank candidates by demonstrated caller burden and change scope; stop when there is enough evidence to recommend a bounded candidate.
3. **Choose the smallest useful change.** Compare keeping the current design with the proposed change. Try deleting a redundant wrapper, moving an invariant to its owner, or combining operations that callers must always coordinate before adding a new abstraction. Explain which knowledge disappears from callers and which remains explicit. Read [DEEPENING.md](DEEPENING.md) when consolidating a cluster or changing dependency seams. Read [DESIGN-IT-TWICE.md](DESIGN-IT-TWICE.md) when the user requests alternatives or materially different designs remain plausible.
4. **Make the proposal reviewable.** Show a representative caller before and after, using a compact signature or pseudocode when helpful. State the hidden responsibility, dependency strategy, observable behavior to preserve, affected callers, and tests that would verify the change. Recommend keeping the current design if no candidate reduces the demonstrated cost.

A design recommendation is complete when it connects a concrete problem to an interface, shows how a caller uses it, and identifies the migration and verification needed. Cite code locations for observed claims and label unresolved assumptions. Scale the answer to the task; a single-function decision does not need a full architecture report.

## Glossary

Use these terms consistently in design reasoning while retaining the repository's domain names and exact identifiers.

| Term | Meaning |
| --- | --- |
| Module | Anything with an interface and an implementation: a function, class, package, or larger cohesive unit. |
| Interface | Everything a caller must know to use a module correctly, including invariants, ordering, errors, configuration, and relevant performance behavior. A language-level signature is only part of it. |
| Implementation | The code and internal collaboration behind an interface. |
| Depth | Useful behavior available relative to the knowledge a caller must learn. A shallow module exposes nearly as much complexity as it handles. |
| Seam | A place where behavior can be substituted without editing the code that uses it. Seam placement and interface design are related but separate decisions. |
| Adapter | A concrete implementation that satisfies an interface at a seam, such as a production transport or a test substitute. |
| Leverage | Capability callers gain from learning an interface. |
| Locality | How well related knowledge, changes, and verification concentrate in one place. |

## Design principles

- **Count caller knowledge, not methods.** One method with hidden ordering rules can be harder to use than several explicit operations. A larger implementation does not make a module deeper.
- **Apply the deletion test.** If removing a wrapper removes complexity without scattering its responsibilities, remove it. If its invariants would reappear across callers, keep those responsibilities together. A small adapter can still earn its place by translating a protocol or isolating a dependency.
- **Hide mechanisms, preserve meaningful choices.** Keep policy and invariants with their owner. Do not hide required error handling, transaction boundaries, resource lifetime, or latency behind an apparently simple interface.
- **Justify seams with a concrete need.** Production substitution, controlled failure tests, or an existing ownership boundary can justify a seam. A second adapter is evidence of variation, not a quota to fill with a needless fake.
- **Keep test setup local.** Separate deterministic decisions from I/O where useful, and accept dependencies where control is needed. Reuse existing injection or configuration patterns; callers should not have to assemble internal collaborators merely to make tests possible.
- **Test observable contracts.** Public behavior is the primary test surface. Focused tests of complex internal algorithms and real adapter contracts can remain valuable. Fewer parameters or methods do not imply fewer behaviors to verify.
