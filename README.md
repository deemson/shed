# Shed

Shed copies dotfiles between their live system paths and a local directory that you can version-control.

- `put` copies from the system into the shed.
- `get` copies from the shed back to the system.

A shed is an ordinary directory, typically a dotfiles repository. Shed does not run Git, manage remotes, or provide cloud storage.

## Installation

### Homebrew (macOS)

The Homebrew tap provides native binaries for Apple Silicon and Intel Macs:

```sh
brew install deemson/tap/shed
```

Homebrew is the supported binary installation route on macOS. The raw macOS archives attached to GitHub releases are unsigned and are not intended for direct browser installation.

### NixOS

Official tags expose a flake package for x86_64 and aarch64 Linux. Release outputs are served by the public `deemson-shed` Cachix cache, with a local source build as the automatic fallback:

```sh
nix profile install github:deemson/shed/v0.2.0 --accept-flake-config
```

`--accept-flake-config` explicitly accepts the public cache URL and signing key declared by the flake. Replace `v0.2.0` with the release you want; pinning the tag keeps installations reproducible.

Static musl archives that run directly on NixOS are also available from each [GitHub release](https://github.com/deemson/shed/releases):

- `shed-x86_64-unknown-linux-musl.tar.gz`
- `shed-aarch64-unknown-linux-musl.tar.gz`

Verify the adjacent `.sha256` file before extracting an archive and placing `shed` on your `PATH`.

### Build from source

Building from source requires Rust 1.97.1 and Cargo:

```sh
cargo install --git https://github.com/deemson/shed.git --tag v0.2.0 --locked
```

To install from a local checkout instead:

```sh
git clone https://github.com/deemson/shed.git
cd shed
cargo install --path . --locked
```

## Quick start

Create a manifest inside a dotfiles repository:

```sh
mkdir -p ~/dotfiles
cd ~/dotfiles
```

Create `workstation.yaml`:

```yaml
items:
  - path: $HOME/.bashrc
    shed: shell/bashrc
```

Preview the copy first:

```sh
shed put --dry workstation.yaml
```

> **Warning:** Shed overwrites existing destination files. It does not delete stale files or detect and resolve content conflicts. Review a dry run before copying in either direction, especially before using `get` against live dotfiles.

Copy the file into the shed:

```sh
shed put workstation.yaml
```

The repository now contains `shell/bashrc`. On another system, clone or pull the repository, preview the reverse copy, and apply it:

```sh
shed get --dry workstation.yaml
shed get workstation.yaml
```

A typical multi-system workflow is: `put`, commit and push with your version-control system, clone or pull elsewhere, then `get`. Version-control operations remain separate from Shed.

## Manifests

Shed 0.2 uses one recursive YAML manifest format. A manifest can include other manifests, declare items, contain both sections, or be intentionally empty:

```yaml
include:
  - shared/shell
items:
  - path: $HOME/.config/editor
    shed: config/editor
    items:
      - init.lua
      - plugins
      - path: local-settings.json
        shed: settings.json
```

Unknown fields are errors. `include` and `items` are always lists, even when they contain one entry.

### Includes

Each `include` entry names another manifest. A relative include is resolved from the directory containing the manifest that declares it. An extensionless include gains `.yaml`; an include with an extension is used verbatim. Absolute include paths are also allowed.

Includes are processed depth-first in listed order before local items. A manifest reached more than once is processed at its first occurrence only. Include cycles are errors.

You can also pass several root manifests. Shed composes them in command-line order and performs one validation, plan, and sync operation:

```sh
shed put machine.yaml private.yaml
```

### Items

Every top-level item is an object with two required fields:

- `path`: an absolute live-system path. A complete leading `$HOME` is expanded.
- `shed`: an absolute path or a path relative to the declaring manifest's directory.

An item with no `items` field is a sync endpoint. An item with a nonempty child `items` list is a path prefix; only its leaves are synced. An explicit empty child list is invalid.

Children can be objects or strings. Child paths must be relative and cannot contain `..`. A child string uses the same suffix on both sides:

```yaml
- path: $HOME/.config/editor
  shed: config/editor
  items:
    - init.lua
```

This resolves to `$HOME/.config/editor/init.lua` and `config/editor/init.lua` relative to the manifest.

A child object can rename or relocate the shed side. If `shed` is omitted, it defaults to the child's `path`:

```yaml
- path: $HOME/.config/editor
  shed: config/editor
  items:
    - path: local-settings.json
      shed: settings.json
    - path: lua
      items:
        - init.lua
```

Top-level strings are not allowed because they cannot identify both endpoint roots unambiguously.

### Path validation

Shed resolves and validates all manifests before writing anything.

- `$HOME` expansion is supported only at the beginning of top-level `path` and `shed` values. Shed does not expand `~`, other variables, shell expressions, include paths, or child paths.
- A used `HOME` value must be nonempty and absolute.
- Top-level paths are normalized lexically. Relative shed roots may use `..`; child paths may not escape their parents.
- A mapping from a path onto itself is rejected.
- Exact duplicate endpoint pairs are copied once. Other equal or parent/descendant overlaps on either side are rejected because they could overwrite each other in `put` or `get`.

Conflict checks do not follow symlinks, so differently written paths that alias through a symlink are not detected as overlaps.

## Commands

### Sync commands

- `shed put [OPTIONS] <MANIFEST>...` copies system paths into the shed.
- `shed get [OPTIONS] <MANIFEST>...` copies shed paths back to the system.

Each command requires at least one exact manifest file path. Relative command-line paths are resolved from the current directory. Unlike includes, root paths do not gain an implicit `.yaml` extension.

Sync options appear after `put` or `get`:

- `--dry` prints planned copies without creating directories or writing files.
- `-v, --verbose` prints each copy operation.
- `--no-progress` disables the interactive progress display.
- `-h, --help` prints command help.

Top-level `-V, --version` prints the release version. Development builds also include the commits since the latest tag, the Git commit, and a `dirty` suffix when the working tree has changes.

### Manifest schema

```sh
shed manifest schema > manifest.schema.json
```

The generated JSON Schema describes the strict root and child YAML shapes. Filesystem-dependent rules such as endpoint absoluteness, expansion, containment, includes, and cross-item conflicts are also checked by Shed at runtime.

### Exit codes

- `0`: completed without warnings or errors.
- `1`: completed with one or more warnings, such as a missing source.
- `2`: manifest, setup, validation, or copy error.
- `130`: cancelled with Ctrl-C.

## Progress and sync behavior

The interactive progress display preserves the authored item hierarchy. Branch rows aggregate descendant items and files; a leaf that points to a directory aggregates files discovered recursively. Included manifests are shown as headings. Exact duplicate leaves remain visible but are marked as duplicates and do not add work twice.

Dry and verbose runs use plain output with indented leaf headings and copy operations. A successful non-verbose run with redirected output remains quiet.

Shed creates destination directories and recursively scans directory sources. Existing destination files are replaced; files present only at the destination are left in place.

Missing sources produce warnings and are skipped. The first Ctrl-C requests graceful cancellation. A second Ctrl-C exits immediately.

Shed does not preserve symlink objects: file symlinks are copied as regular file contents. Symlinked directories encountered inside a copied directory tree are created at the destination but are not traversed.

## Migrating from 0.1

Version 0.2 deliberately removes the collection/item split and its compatibility vocabulary.

- Replace `shed --collection workstation.yaml put` with `shed put workstation.yaml`.
- Remove `--items-dir`; every relative include and shed path is based on its declaring manifest.
- Replace `inherit` with `include`. Remove `abstract`.
- Replace each collection item reference with an `include` entry.
- Wrap endpoint definitions in a manifest `items` list, rename `system` to `path`, and make top-level shed paths point directly to their storage location.
- Use nested items to share path prefixes.
- Replace `shed config schema collections` and `shed config schema items` with `shed manifest schema`.

Version 0.2 does not parse 0.1 documents or accept old CLI aliases. Migrate manifests and invocation scripts together.

## License

Shed is available under the [MIT License](LICENSE).
