# Shed Development Guidelines

## Refactor Context

Before assisting with this repository, read [`REFACTORING.md`](./REFACTORING.md). It explains the purpose of this branch, where to find the `0.2.0` reference implementation, and the guidance-first collaboration approach requested by the repository owner.

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
