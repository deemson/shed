# Shed Development Guidelines

## Architecture

A run has four phases. Each phase is a module in `src/` and takes plain data from the phase before it:

1. `manifest` resolves the root manifests and their includes into `manifest::Manifest` values. `manifest::yaml` holds the serde types and the JSON schema.
2. `traversal` turns manifests into a `traversal::Plan`: a tree of destination directories and leaves, plus the plan errors.
3. `scanning` reads the source metadata and turns the plan into `scanning::Node` values.
4. `execution` creates directories and copies files.

`manifest`, `scanning`, and `execution` send events to an `mpsc` channel. `main.rs` collects the events of each phase and checks them for errors. If a phase reports an error, `main.rs` stops the run, prints the report, and exits with code 1.

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
