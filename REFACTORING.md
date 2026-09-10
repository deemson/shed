# Post-0.2.0 Refactor Notes

This branch is a deliberate clean-slate refactor after the `0.2.0` release. The application source was intentionally deleted; it is not missing by accident.

## Reference implementation

The released implementation is available at `../shed`. Treat it as read-only reference material for behavior, requirements, and implementation ideas. Do not modify it, and do not simply restore or copy it wholesale into this repository.

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
3. Inspect `../shed` when existing behavior needs to be understood.
4. Prefer suggesting a next step and helping the owner implement it; do not take over the refactor unless explicitly asked.
5. Explain unfamiliar syntax, compiler errors, ownership/borrowing choices, traits, and testing patterns rather than assuming prior knowledge.
6. Preserve released behavior intentionally, using focused tests where practical, while improving separation of concerns.
7. Ask questions when product behavior or an architectural choice is ambiguous.

A likely direction is to keep domain types and operations independent, put parsing/deserialization behind a boundary that produces domain values, and have Ratatui call an application-facing API. This is guidance, not a fixed design; evolve it collaboratively as the refactor proceeds.
