use indoc::formatdoc;
use std::{
    fs,
    os::unix::fs::{PermissionsExt, symlink},
    path::{Path, PathBuf},
    process::{Command, ExitStatus},
};
use tempfile::TempDir;

struct Fixture {
    _temp_dir: TempDir,
    home: PathBuf,
    repo: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let temp_dir = TempDir::new().unwrap();
        let root = fs::canonicalize(temp_dir.path()).unwrap();
        let home = root.join("home");
        let repo = root.join("repo");
        fs::create_dir_all(&home).unwrap();
        fs::create_dir_all(&repo).unwrap();

        Self {
            _temp_dir: temp_dir,
            home,
            repo,
        }
    }

    fn manifest(&self, name: &str, body: &str) -> PathBuf {
        let path = self.repo.join(name).with_extension("yaml");
        write(&path, body);
        path
    }

    fn root_manifest(&self, items: &str) -> PathBuf {
        let home = self.home.display();
        self.manifest(
            "shed",
            &formatdoc! {"
              path: {home}
              items:
              {items}
            "},
        )
    }
}

fn write(path: &Path, contents: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, contents).unwrap();
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap()
}

fn put(manifest: &Path) -> (ExitStatus, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_shed"))
        .arg("put")
        .arg(manifest)
        .env_remove("SHED_TEST_UNSET")
        .output()
        .unwrap();
    (output.status, String::from_utf8(output.stdout).unwrap())
}

#[test]
fn copies_every_item_and_exits_with_success_without_errors() {
    let fixture = Fixture::new();
    write(&fixture.home.join("good/a"), "good");
    let manifest = fixture.root_manifest("  - good");

    let (status, _) = put(&manifest);

    assert!(status.success());
    assert_eq!(read(&fixture.repo.join("good/a")), "good");
}

#[test]
fn copies_the_healthy_item_when_a_source_is_absent() {
    let fixture = Fixture::new();
    write(&fixture.home.join("good/a"), "good");
    let manifest = fixture.root_manifest("  - missing\n  - good");

    let (status, report) = put(&manifest);

    assert_eq!(status.code(), Some(1));
    assert_eq!(read(&fixture.repo.join("good/a")), "good");
    assert!(!fixture.repo.join("missing").exists());
    assert!(report.contains(&format!(
        "failed to read metadata at {}",
        fixture.home.join("missing").display()
    )));
}

#[test]
fn copies_the_healthy_item_when_a_source_is_a_symlink() {
    let fixture = Fixture::new();
    write(&fixture.home.join("good/a"), "good");
    symlink(fixture.home.join("good/a"), fixture.home.join("link")).unwrap();
    let manifest = fixture.root_manifest("  - link\n  - good");

    let (status, report) = put(&manifest);

    assert_eq!(status.code(), Some(1));
    assert_eq!(read(&fixture.repo.join("good/a")), "good");
    assert!(!fixture.repo.join("link").exists());
    assert!(report.contains(&format!(
        "unsupported filesystem type at {}",
        fixture.home.join("link").display()
    )));
}

#[test]
fn leaves_a_directory_item_untouched_when_one_entry_fails_to_scan() {
    let fixture = Fixture::new();
    write(&fixture.home.join("good/a"), "good");
    write(&fixture.home.join("bad/a"), "new a");
    write(&fixture.home.join("bad/locked/x"), "new x");
    write(&fixture.repo.join("bad/a"), "old a");
    write(&fixture.repo.join("bad/locked/x"), "old x");
    let locked = fixture.home.join("bad/locked");
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
    let manifest = fixture.root_manifest("  - bad\n  - good");

    let (status, report) = put(&manifest);
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();

    assert_eq!(status.code(), Some(1));
    assert_eq!(read(&fixture.repo.join("good/a")), "good");
    assert_eq!(read(&fixture.repo.join("bad/a")), "old a");
    assert_eq!(read(&fixture.repo.join("bad/locked/x")), "old x");
    assert!(report.contains("failed to scan some entries"));
}

#[test]
fn copies_every_item_after_an_include_cycle() {
    let fixture = Fixture::new();
    write(&fixture.home.join("good/a"), "good");
    let home = fixture.home.display();
    let manifest = fixture.manifest(
        "one",
        &formatdoc! {"
          include:
            - two
          path: {home}
          items:
            - good
        "},
    );
    fixture.manifest("two", "include:\n  - one\n");

    let (status, report) = put(&manifest);

    assert_eq!(status.code(), Some(1));
    assert_eq!(read(&fixture.repo.join("good/a")), "good");
    assert!(report.contains("include cycle detected"));
}

#[test]
fn skips_overlapping_items_and_copies_the_others() {
    let fixture = Fixture::new();
    write(&fixture.home.join("good/a"), "good");
    write(&fixture.home.join("overlap/f"), "overlap");
    let manifest = fixture.root_manifest("  - overlap\n  - overlap/f\n  - good");

    let (status, report) = put(&manifest);

    assert_eq!(status.code(), Some(1));
    assert_eq!(read(&fixture.repo.join("good/a")), "good");
    assert!(!fixture.repo.join("overlap").exists());
    assert!(report.contains("overlap #1"));
}

#[test]
fn copies_nothing_when_a_destination_is_unknown() {
    let fixture = Fixture::new();
    write(&fixture.home.join("good/a"), "good");
    write(&fixture.home.join("bad/a"), "bad");
    let manifest = fixture.root_manifest("  - path: bad\n    shed: $SHED_TEST_UNSET/bad\n  - good");

    let (status, report) = put(&manifest);

    assert_eq!(status.code(), Some(1));
    assert!(!fixture.repo.join("good").exists());
    assert!(report.contains("failed to expand shed"));
}

#[test]
fn reports_an_absent_include_and_copies_nothing() {
    let fixture = Fixture::new();
    write(&fixture.home.join("good/a"), "good");
    let home = fixture.home.display();
    let manifest = fixture.manifest(
        "root",
        &formatdoc! {"
          include:
            - absent
          path: {home}
          items:
            - good
        "},
    );

    let (status, report) = put(&manifest);

    assert_eq!(status.code(), Some(1));
    assert!(!fixture.repo.join("good").exists());
    assert!(report.contains("manifest was not found"));
}
