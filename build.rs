use std::{env, path::Path, process::Command};

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("Cargo sets CARGO_MANIFEST_DIR");
    let cargo_version = env::var("CARGO_PKG_VERSION").expect("Cargo sets CARGO_PKG_VERSION");

    let manifest_dir = Path::new(&manifest_dir);
    let version = if manifest_dir.join(".git").exists() {
        register_git_inputs(manifest_dir);
        git_description(manifest_dir, &cargo_version)
    } else {
        None
    }
    .unwrap_or_else(|| cargo_version.clone());

    println!("cargo:rustc-env=SHED_VERSION={version}");
}

fn git_description(manifest_dir: &Path, cargo_version: &str) -> Option<String> {
    let description = git(
        manifest_dir,
        &[
            "describe", "--tags", "--match", "v[0-9]*", "--always", "--dirty",
        ],
    )?;

    if let Some(version) = description.strip_prefix('v') {
        Some(version.to_owned())
    } else {
        Some(format!("{cargo_version}-0-g{description}"))
    }
}

fn register_git_inputs(manifest_dir: &Path) {
    // Recompute the version when tracked files, HEAD, the index, or tags change.
    if let Some(files) = git(manifest_dir, &["ls-files"]) {
        for file in files.lines() {
            println!("cargo:rerun-if-changed={file}");
        }
    }

    for path in ["HEAD", "index", "packed-refs", "refs/tags"] {
        if let Some(path) = git(manifest_dir, &["rev-parse", "--git-path", path]) {
            println!("cargo:rerun-if-changed={path}");
        }
    }

    if let Some(head) = git(manifest_dir, &["symbolic-ref", "-q", "HEAD"])
        && let Some(path) = git(manifest_dir, &["rev-parse", "--git-path", &head])
    {
        println!("cargo:rerun-if-changed={path}");
    }
}

fn git(manifest_dir: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(manifest_dir)
        .args(args)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    let output = String::from_utf8(output.stdout).ok()?;
    let output = output.trim();
    (!output.is_empty()).then(|| output.to_owned())
}
