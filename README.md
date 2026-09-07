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

Homebrew is the supported binary installation route on macOS. The raw macOS
archives attached to GitHub releases are unsigned and are not intended for
direct browser installation.

### NixOS

Official tags expose a flake package for x86_64 and aarch64 Linux. Release
outputs are served by the public `deemson-shed` Cachix cache, with a local
source build as the automatic fallback:

```sh
nix profile install github:deemson/shed/v0.1.0 --accept-flake-config
```

`--accept-flake-config` explicitly accepts the public cache URL and signing key
declared by the flake. Replace `v0.1.0` with the release you want; pinning the
tag keeps installations reproducible.

Static musl archives that run directly on NixOS are also available from each
[GitHub release](https://github.com/deemson/shed/releases):

- `shed-x86_64-unknown-linux-musl.tar.gz`
- `shed-aarch64-unknown-linux-musl.tar.gz`

Verify the adjacent `.sha256` file before extracting an archive and placing
`shed` on your `PATH`.

### Build from source

Building from source requires Rust 1.97.1 and Cargo:

```sh
cargo install --git https://github.com/deemson/shed.git --tag v0.1.0 --locked
```

To install from a local checkout instead:

```sh
git clone https://github.com/deemson/shed.git
cd shed
cargo install --path . --locked
```

## Quick start

Create a shed with a collection and an item definition:

```sh
mkdir -p ~/dotfiles/shell
cd ~/dotfiles
```

Create `workstation.yaml`:

```yaml
items:
  - shell/bash
```

Create `shell/bash.yaml`:

```yaml
- system: $HOME/.bashrc
  shed: bashrc
```

The collection selects the `shell/bash` item. That item maps the live `$HOME/.bashrc` file to `shell/bash/bashrc` inside the shed.

Preview the copy first:

```sh
shed --collection workstation.yaml --dry put
```

> **Warning:** Shed overwrites existing destination files. It does not delete stale files or detect and resolve conflicts. Review a dry run before copying in either direction, especially before using `get` against live dotfiles.

Copy the file into the shed:

```sh
shed --collection workstation.yaml put
```

The directory now has this layout:

```text
dotfiles/
├── workstation.yaml
└── shell/
    ├── bash.yaml
    └── bash/
        └── bashrc
```

The `system` path may also point to a directory; Shed copies directory contents recursively.

On another system, make the repository available, change into it, preview the reverse copy, and then apply it:

```sh
shed --collection workstation.yaml --dry get
shed --collection workstation.yaml get
```

A typical multi-system workflow is: `put`, commit and push with your version-control system, clone or pull elsewhere, then `get`. Version-control operations remain separate from Shed.

## Configuration

Shed uses two YAML document types: collections choose what to sync, and items define where files live.

### Collections

A collection supports these fields:

- `items`: item references relative to the configuration directory, without the `.yaml` extension.
- `inherit`: collection names to include before this collection, also without `.yaml`.
- `abstract`: when `true`, prevents the collection from being synced directly while allowing other collections to inherit it.

For example, `base.yaml` can hold shared items:

```yaml
abstract: true
items:
  - shell/bash
```

A machine collection can inherit them in `workstation.yaml`:

```yaml
inherit:
  - base
```

Inherited collections are processed first. Repeated item references are included once, inheritance cycles are rejected, and an abstract root collection cannot be used with `put` or `get`.

### Items

An item file contains a list of mappings:

```yaml
- system: $HOME/.bashrc
  shed: bashrc
- system: $HOME/.config/bash
  shed: config
```

Each mapping has two required fields:

- `system`: an absolute live-system path. `$HOME` is expanded when it appears at the start of the value.
- `shed`: a relative path within the item's storage directory.

Given item reference `shell/bash`, sync directory `~/dotfiles`, and shed path `config`, the stored path is `~/dotfiles/shell/bash/config`.

### Path resolution

The configuration directory defaults to the directory containing the root collection file. Shed resolves item YAML files and inherited collection files from this directory. Use `--items-dir` to select a different configuration directory.

The sync directory is the optional `DIR` passed to `put` or `get`; it defaults to the current directory. The directory must already exist.

## Commands

Top-level options must appear before `put`, `get`, or `config`. The global `--no-progress` option may also appear after a subcommand.

### Sync commands

- `shed [OPTIONS] put [DIR]` copies system paths into the shed. `DIR` defaults to the current directory.
- `shed [OPTIONS] get [DIR]` copies shed paths back to the system. `DIR` defaults to the current directory.

Both sync commands require `--collection`.

### Configuration commands

- `shed config schema collections` prints the JSON Schema for collection YAML.
- `shed config schema items` prints the JSON Schema for item YAML.

### Options

- `-c, --collection <COLLECTION>` selects the root collection YAML for `put` or `get`.
- `-i, --items-dir <ITEMS_DIR>` sets the configuration directory used to resolve items and inherited collections.
- `--dry` prints planned copies without creating directories or writing files.
- `-v, --verbose` prints each copy operation.
- `--no-progress` disables the interactive progress display.
- `-h, --help` prints command help.
- `-V, --version` prints the installed version.

Run `shed --help` or `shed <COMMAND> --help` for generated command help.

### Exit codes

- `0`: completed without warnings or errors.
- `1`: completed with one or more warnings, such as a missing source.
- `2`: configuration, setup, or copy error.
- `130`: cancelled with Ctrl-C.

## Sync behavior and limitations

Shed creates destination directories and recursively scans directory sources. Existing destination files are replaced; files present only at the destination are left in place.

Missing sources produce warnings and are skipped. A normal interactive run displays inline progress. Dry runs, verbose runs, redirected output, and `--no-progress` use plain output instead.

The first Ctrl-C requests graceful cancellation. A second Ctrl-C exits immediately.

Shed does not preserve symlink objects: file symlinks are copied as regular file contents. Symlinked directories encountered inside a copied directory tree are created at the destination but are not traversed.

## JSON Schemas

Shed can generate JSON Schemas for editor integration or external validation:

```sh
shed config schema collections > collection.schema.json
shed config schema items > items.schema.json
```

## License

Shed is available under the [MIT License](LICENSE).
