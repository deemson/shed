# Post-0.2.0 Refactor Notes

This branch is a deliberate clean-slate refactor after the `0.2.0` release. The application source was intentionally deleted; it is not missing by accident.

## Historical reference implementation

The released `0.2.0` implementation is available at `../shed`. Treat it as read-only historical context and a source of implementation ideas. It is **not** the behavioral or manifest-format specification for this branch: the refactor may deliberately redesign or remove `0.2.0` behavior. Do not modify the reference repository, and do not simply restore or copy it wholesale into this repository.

The current branch—especially its current domain model, tests, and the repository owner's stated intent—is the source of truth. Do not characterize a deliberate difference from `0.2.0` as incorrect merely because it is incompatible with the release.

## Goal

The main problem with the `0.2.0` implementation is that these concerns are too tightly interconnected:

- application/domain logic
- manifest deserialization
- Ratatui user-interface code

The refactor should establish clear boundaries between them so that core behavior can be understood and tested without deserialization or terminal UI concerns.

## How to collaborate

The repository owner wants to perform the refactor themselves as a way to learn Rust. They are not yet confident with Rust and want ongoing, step-by-step guidance.

When assisting:

1. Explain the relevant Rust concepts and architectural tradeoffs in approachable terms.
2. Work in small, reviewable steps rather than rebuilding the application all at once.
3. Inspect `../shed` only when historical behavior or an implementation idea is useful; do not treat it as authoritative for the refactor.
4. Prefer suggesting a next step and helping the owner implement it; do not take over the refactor unless explicitly asked.
5. Explain unfamiliar syntax, compiler errors, ownership/borrowing choices, traits, and testing patterns rather than assuming prior knowledge.
6. Follow the behavior expressed by current tests and the repository owner's decisions. Do not assume compatibility with `0.2.0` unless compatibility is explicitly requested.
7. Ask questions when product behavior or an architectural choice is ambiguous instead of resolving ambiguity in favor of `0.2.0`.

A likely direction is to keep domain types and operations independent, put parsing/deserialization behind a boundary that produces domain values, and have Ratatui call an application-facing API. This is guidance, not a fixed design; evolve it collaboratively as the refactor proceeds.
