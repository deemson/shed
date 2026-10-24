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
nix profile install github:deemson/shed/v0.3.0 --accept-flake-config
```

`--accept-flake-config` explicitly accepts the public cache URL and signing key declared by the flake. Replace `v0.3.0` with the release you want; pinning the tag keeps installations reproducible.

Static musl archives that run directly on NixOS are also available from each [GitHub release](https://github.com/deemson/shed/releases):

- `shed-x86_64-unknown-linux-musl.tar.gz`
- `shed-aarch64-unknown-linux-musl.tar.gz`

Verify the adjacent `.sha256` file before extracting an archive and placing `shed` on your `PATH`.

### Build from source

Building from source requires Rust 1.97.1 and Cargo:

```sh
cargo install --git https://github.com/deemson/shed.git --tag v0.3.0 --locked
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

> **Warning:** Shed replaces destinations. A file copy overwrites the destination file. A directory copy deletes the destination directory first, so files that exist only at the destination are lost. Review a dry run before you copy in either direction, especially before you run `get` against live dotfiles.

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

A manifest is a YAML file with four optional fields:

```yaml
path: $HOME/.config
shed: config
include:
  - shared/shell
items:
  - nvim
  - path: git/config
    shed: gitconfig
```

- `path` is the system directory for the items of the manifest.
- `shed` is the shed directory for the items of the manifest. A relative value starts at the directory of the manifest. The default is the directory of the manifest.
- `include` lists other manifests.
- `items` lists the files and directories to copy.

The example copies `$HOME/.config/nvim` to `config/nvim` and `$HOME/.config/git/config` to `config/gitconfig`.

Unknown fields are errors. An empty manifest (`{}`) is valid.

### Items

An item is a string or an object with `path`, `shed`, and `items` fields. A string is a short form for an object with only `path`.

```yaml
items:
  - path: $HOME/.config/editor
    shed: editor
    items:
      - init.lua
      - path: local-settings.json
        shed: settings.json
      - path: lua
        items:
          - plugins.lua
```

A relative `path` starts at the path of the parent item. For a top-level item, the parent path is the `path` of the manifest. An absolute `path` replaces the parent path.

A relative `shed` starts at the shed path of the parent item in the same way. An item without `shed` uses its `path` as its shed path. The example above copies these files:

| system | shed |
|---|---|
| `$HOME/.config/editor/init.lua` | `editor/init.lua` |
| `$HOME/.config/editor/local-settings.json` | `editor/settings.json` |
| `$HOME/.config/editor/lua/plugins.lua` | `editor/lua/plugins.lua` |

An item with `items` is a parent. Shed copies only the leaves, the items without `items`. A leaf can be a file or a directory. A parent with an empty `items` list copies no files.

A top-level item with a relative `path` in a manifest without `path` starts at the current directory. Give such an item an absolute path, or give the manifest a `path`.

### Path rules

Shed expands a leading `~`, `$VAR`, and `${VAR}` in every `path` and `shed` value, at every level. An undefined variable is an error. Shed does not expand include names.

Shed removes `.` and `..` components lexically. Shed does not resolve symlinks when it compares paths.

The planner reports these errors:

- An item has an absolute `path` and no `shed`. The item does not show where its files belong in the shed.
- A nested item has an absolute `path` and an absolute `shed`.
- A leaf destination is outside its destination root. The destination root is the destination side of the manifest when the manifest sets `path` or `shed`. Otherwise, the destination root is the destination side of the top-level parent item.
- Two leaves have the same destination, or one destination is inside the other. Shed reports all of the affected leaves as one overlap group.

The destination side is the shed for `put` and the system for `get`. Shed compares only destinations, so one source can go to two destinations.

### Includes

Each `include` entry names another manifest without its extension. Shed looks for `<name>.yaml`, then for `<name>.yml`. A relative name starts at the directory of the manifest that declares the include. An absolute name is also allowed.

Write include names without the extension. Shed always adds one, so `common.yaml` refers to `common.yaml.yaml`.

Include cycles are errors. Shed resolves a manifest once for each include of it. If two manifests include the same manifest, its leaves appear twice and cause an overlap error.

You can also pass several root manifests. Shed plans and copies them together:

```sh
shed put machine.yaml private.yaml
```

## Commands

### Sync commands

- `shed put [OPTIONS] <MANIFEST>...` copies system paths into the shed.
- `shed get [OPTIONS] <MANIFEST>...` copies shed paths back to the system.

Each command needs at least one exact manifest file path. A relative path starts at the current directory. Root manifest paths do not get an implicit extension.

Sync options come after `put` or `get`:

- `--dry` runs every check but creates no directories and writes no files.
- `-v, --verbose` adds the files and directories inside each directory item to the report.
- `-h, --help` prints command help.

Top-level `-V, --version` prints the release version. Development builds also include the commits since the latest tag, the Git commit, and a `dirty` suffix when the working tree has changes.

### Manifest schema

```sh
shed manifest schema > manifest.schema.json
```

The generated JSON Schema describes the YAML shape of a manifest. Shed checks the path rules, includes, and overlaps at runtime.

### Exit codes

- `0`: every phase completed without errors.
- `1`: a phase reported one or more errors.

## How a run works

A run has four phases:

1. **Resolve** reads the root manifests and their includes.
2. **Plan** expands paths, checks the path rules, and builds the list of copies.
3. **Scan** reads the metadata of each source and lists the contents of each source directory.
4. **Copy** creates the destination directories and copies the files.

Shed stops after the first phase that reports an error. Shed then prints the report and exits with code `1`. So Shed changes no files unless resolve, plan, and scan all succeed. An error in the copy phase does not stop the copies of other items.

At the end of a run, Shed prints a report as a tree of manifests and items. An arrow shows the direction: `←` for `put` and `→` for `get`. Each error appears in red below its item. A `--dry` run prints the same report as a real run.

Copy rules:

- For a file source, Shed replaces the destination file. A destination that exists and is not a regular file is an error.
- For a directory source, Shed deletes the destination directory and copies the source tree into a new one.
- Shed creates the parent directories of each destination.
- A source that does not exist is an error.
- A symlink in a source is an error, at the top level and inside a directory.

Shed does not handle Ctrl-C. An interrupt stops the process immediately and can leave a copy incomplete.

## Migrating from 0.2

The manifest format and the copy rules changed after 0.2.

- A manifest can set `path` and `shed` for all of its items.
- Top-level items can be strings.
- Shed expands `~` and every environment variable in every `path` and `shed` value. Version 0.2 expanded only a leading `$HOME` in top-level values.
- Child paths can be absolute and can contain `..`, inside the destination root.
- An item with an absolute `path` now needs a `shed`.
- Shed always adds an extension to include names and also tries `.yml`. Remove the extension from include names that contain one.
- A manifest that two manifests include now causes an overlap error. Version 0.2 processed it once.
- An empty `items` list is no longer an error.
- A directory copy now deletes files that exist only at the destination. Version 0.2 left them in place.
- A source that does not exist is now an error that stops the run. Version 0.2 skipped it with a warning.
- A symlink in a source is now an error. Version 0.2 copied the content of a file symlink.
- `--no-progress` and the progress display are gone. Shed prints one report at the end of the run. `-v` now adds directory contents to the report.
- Every error exits with code `1`. Codes `2` and `130` are gone.

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
