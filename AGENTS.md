# Shed Development Guidelines

## Architecture

A run has four phases. Each phase is a module in `src/` and takes plain data from the phase before it:

1. `manifest` resolves the root manifests and their includes into `manifest::Manifest` values. `manifest::yaml` holds the serde types and the JSON schema.
2. `traversal` turns manifests into a `traversal::Plan`: a tree of destination directories and leaves, plus the plan errors.
3. `scanning` reads the source metadata and turns the plan into `scanning::Node` values.
4. `execution` creates directories and copies files.

`manifest`, `scanning`, and `execution` send events to an `mpsc` channel. `main.rs` collects the events of each phase and gives them to `orchestration`, which sorts the errors of the phase into a `PhaseOutcome`:

- `BlockingErrors`: an error hides where an item writes, so the overlap check is incomplete. `main.rs` stops the run, prints the report, and exits with code 1.
- `NonBlockingErrors`: every error affects only its own items. The phases leave those items out of their output, and the run continues.
- `NoErrors`: the phase reported no error.

Each `orchestration` function matches every error variant without a `_` arm, so a new variant needs a decision at compile time. After execution, `main.rs` exits with code 1 if any phase reported an error.

`reporting::summary` builds the report from the manifests, the plan, and the events. The report takes ownership of the manifests and of the events, and it does not decide whether a run failed.

Positional indexes connect the phases. `manifest::Index` locates an item by its manifest index and item index. `scanning::Index` adds an entry index inside a directory item.

## Commit Messages

Follow [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>[optional scope]: <description>

[optional body]

[optional footer(s)]
```

Types: `feat`, `fix`, `docs`, `style`, `refactor`, `perf`, `test`, `build`, `ci`, `chore`, `revert`

Examples:
- `feat: add collection inheritance`
- `fix: handle missing source paths gracefully`
- `docs: update README with usage examples`
- `refactor(sync): extract directory traversal logic`

## Claims About the Compiler

Do not state that the compiler rejects or needs some code before you build it. Remove the code in question, run `cargo build`, and quote the result.

The most frequent false claim concerns `move` on a closure or an async block. Without `move`, a closure or an async block captures each variable in the weakest mode that its body needs. A body that passes a variable by value captures that variable by value. So `move` changes only the variables that the body uses by reference.
