# Releasing Shed

Shed uses cargo-dist 0.32.0 to build and host releases. An annotated `vX.Y.Z`
tag is the only release trigger, and `X.Y.Z` must equal the version in
`Cargo.toml`.

## One-time setup

1. Create a public Cachix cache named `deemson-shed`.
2. Put its public URL and signing key in `flake.nix` under
   `nixConfig.extra-substituters` and
   `nixConfig.extra-trusted-public-keys`.
3. Add the cache's write token to `deemson/shed` as the Actions secret
   `CACHIX_AUTH_TOKEN`.
4. Create a fine-grained GitHub token with Contents: read/write access only to
   `deemson/homebrew-tap`, then add it to `deemson/shed` as
   `HOMEBREW_TAP_TOKEN`.
5. After CI has completed successfully once, protect `main`. Require these
   checks for pull requests without requiring an approving review:
   - `Lint`
   - `Test (Linux)`
   - `Test (macOS)`
   - `Nix`
   Block force-pushes and deletion.

The cache URL and public key are public configuration. Never commit either
write token or expose it to pull-request jobs.

## Changing release configuration

`dist-workspace.toml` is the source of truth for the generated release
workflow. Install the pinned dist version and regenerate after changing it:

```sh
curl --proto '=https' --tlsv1.2 -LsSf \
  https://github.com/axodotdev/cargo-dist/releases/download/v0.32.0/cargo-dist-installer.sh \
  | sh
dist generate
dist generate --check
dist plan --tag=v0.1.0
```

Do not hand-edit `.github/workflows/release.yml`. Custom behavior belongs in
the reusable workflows beside it. Action pins are configured in
`dist-workspace.toml`; review generated workflow changes as dependency
changes.

For initial setup, and after material release-pipeline changes, temporarily set
`pr-run-mode = "upload"`, regenerate, and let the pull request build all four
targets without publishing. Inspect the archives and checks, then restore
`pr-run-mode = "plan"`, regenerate again, and merge only the final plan-mode
configuration.

## Cutting a release

1. Update `package.version` in `Cargo.toml` and update `Cargo.lock` if needed.
2. Update user-facing documentation for the release.
3. Run the local checks:

   ```sh
   cargo fmt --all -- --check
   cargo clippy --locked --all-targets --all-features -- -D warnings
   cargo test --locked --all-targets --all-features
   dist generate --check
   dist plan --tag=vX.Y.Z
   ```

4. Merge the release commit into `main` and wait for required CI to pass.
5. From an up-to-date `main`, create and push an annotated tag:

   ```sh
   git tag -a vX.Y.Z -m "Release vX.Y.Z"
   git push origin vX.Y.Z
   ```

The preflight job rejects lightweight tags, non-`vX.Y.Z` tags, version
mismatches, and commits outside `main` history.

## Expected output and ordering

Release CI builds these archives on native runners:

- `shed-aarch64-apple-darwin.tar.gz`
- `shed-x86_64-apple-darwin.tar.gz`
- `shed-aarch64-unknown-linux-musl.tar.gz`
- `shed-x86_64-unknown-linux-musl.tar.gz`

Each archive has a SHA-256 sidecar. The release also contains a unified
`sha256.sum`, source archive, dist manifest, and GitHub provenance
attestations.

Publication is intentionally ordered as follows:

1. Build and verify all archives, including static Linux linkage and versions.
2. Build and smoke-test both native Nix packages.
3. Create the public GitHub Release, which is the canonical artifact host.
4. Upload, pin, and read back both Nix closures from Cachix.
5. Generate and test the macOS-only formula on Apple Silicon and Intel.
6. Commit `Formula/shed.rb` directly to `deemson/homebrew-tap`.
7. Append GitHub-generated change notes to cargo-dist's release body.

The GitHub Release necessarily becomes public before Cachix and Homebrew are
updated. A downstream failure therefore leaves a visible partial release and a
failed workflow.

## Recovery

Fix the workflow or credential and rerun the failed workflow for the same tag.
Publication steps are designed to be idempotent. Do not move the tag, delete a
public release to hide a failure, or replace already-public assets. If an asset
itself is invalid, publish a new patch version.

A missing secret or unavailable cache/tap must fail visibly. Rotate a leaked
credential at its provider and update the corresponding repository secret
before rerunning.
