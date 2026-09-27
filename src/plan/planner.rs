use super::model::{Directory, File};

pub struct Planner {}

impl Planner {}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::manifest::{testing as testing_m};
    use indoc::formatdoc;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test1() {
        let path_dir = TempDir::new().unwrap();
        let shed_dir = TempDir::new().unwrap();

        let path_dir_display = path_dir.path().display();
        let manifest_path = testing_m::write_yaml_manifest(
            shed_dir.path(),
            "manifest",
            formatdoc! {"
              items:
                - path: {path_dir_display}
                  items:
                    - path: intermediate-dir
                      items:
                        - path: sub-dir
                          items:
                            - file1
                            - file2
            "},
        );
        let expected = [Directory {
            dst: path_dir.path().join("intermediate-dir/sub-dir"),
            is_to_be_cleaned: false,
            directories: None,
            files: Some(vec![
                File {
                    src: shed_dir.path().join("intermediate-dir/sub-dir/file1"),
                    dst: "file1".into(),
                },
                File {
                    src: shed_dir.path().join("intermediate-dir/sub-dir/file2"),
                    dst: "file2".into(),
                },
            ]),
        }];
    }

    #[tokio::test]
    async fn test2() {
        let path_dir1 = TempDir::new().unwrap();
        let path_dir2 = TempDir::new().unwrap();
        let shed_dir = TempDir::new().unwrap();

        let path_dir1_display = path_dir1.path().display();
        let path_dir2_display = path_dir2.path().display();
        let manifest_path = testing_m::write_yaml_manifest(
            shed_dir.path(),
            "manifest",
            formatdoc! {"
              items:
                - path: {path_dir1_display}
                  items:
                    - file1
                - path: {path_dir2_display}
                  items:
                    - file2
            "},
        );
        let expected = [
            Directory {
                dst: path_dir1.path().into(),
                is_to_be_cleaned: false,
                directories: None,
                files: Some(vec![File {
                    src: shed_dir.path().into(),
                    dst: "file1".into(),
                }]),
            },
            Directory {
                dst: path_dir2.path().into(),
                is_to_be_cleaned: false,
                directories: None,
                files: Some(vec![File {
                    src: shed_dir.path().into(),
                    dst: "file2".into(),
                }]),
            },
        ];
    }

    #[tokio::test]
    async fn test3() {
        let path_dir = TempDir::new().unwrap();
        let shed_dir = TempDir::new().unwrap();

        let path_dir_display = path_dir.path().display();
        let manifest_path = testing_m::write_yaml_manifest(
            shed_dir.path(),
            "manifest",
            formatdoc! {"
              items:
                - path: {path_dir_display}
                  items:
                    - sub-dir
            "},
        );
        let expected = [Directory {
            dst: path_dir.path().join("sub-dir"),
            is_to_be_cleaned: true,
            directories: None,
            files: Some(vec![
                File {
                    src: shed_dir.path().join("sub-dir/file1"),
                    dst: "file1".into(),
                },
                File {
                    src: shed_dir.path().join("sub-dir/file2"),
                    dst: "file2".into(),
                },
            ]),
        }];
    }

    #[test]
    fn iradix_demo() {
        use iradix::sync::Radix;
        use std::{
            ffi::OsString,
            path::{Path, PathBuf},
        };

        let empty: Radix<OsString, &str> = Radix::new();
        let mut transaction = empty.txn();

        let config_dir = PathBuf::from("home/alice/.config");
        transaction.insert(&config_dir, "config");
        transaction.insert(Path::new("home/alice/.config/shed"), "shed");

        let paths = transaction.commit();

        assert_eq!(empty.get(config_dir.as_path()), None);
        assert_eq!(paths.get(config_dir.as_path()), Some(&"config"));
        assert_eq!(
            paths.get_ancestor(Path::new("home/alice/.config/shed/manifest.yaml")),
            Some(&"shed")
        );
        assert_eq!(
            paths.get_ancestor(Path::new("home/alice/.configuration")),
            None
        );
    }

    #[test]
    fn iradix_unsync_demo() {
        use iradix::unsync::Radix;
        use std::{
            ffi::OsString,
            path::{Path, PathBuf},
        };

        let mut paths: Radix<OsString, &str> = Radix::new();

        let config_dir = PathBuf::from("home/alice/.config");
        paths.insert(&config_dir, "config");
        paths.insert(Path::new("home/alice/.config/shed"), "shed");

        assert_eq!(paths.get(config_dir.as_path()), Some(&"config"));
        assert_eq!(
            paths.get_ancestor(Path::new("home/alice/.config/shed/manifest.yaml")),
            Some(&"shed")
        );
        assert_eq!(
            paths.get_ancestor(Path::new("home/alice/.configuration")),
            None
        );
    }

    #[test]
    fn iradix_transactions_from_the_same_snapshot_do_not_conflict() {
        use iradix::sync::Radix;
        use std::{ffi::OsString, path::PathBuf};

        let base: Radix<OsString, &str> = Radix::new();
        let key = PathBuf::from("home/alice/.config/shed");

        let mut first_transaction = base.txn();
        let mut second_transaction = base.txn();

        first_transaction.insert(key.as_path(), "first value");
        second_transaction.insert(key.as_path(), "second value");

        let first_snapshot = first_transaction.commit();
        let second_snapshot = second_transaction.commit();

        assert_eq!(base.get(key.as_path()), None);
        assert_eq!(first_snapshot.get(key.as_path()), Some(&"first value"));
        assert_eq!(second_snapshot.get(key.as_path()), Some(&"second value"));
    }

    #[test]
    fn iradix_unsync_clones_from_the_same_snapshot_remain_isolated() {
        use iradix::unsync::Radix;
        use std::{ffi::OsString, path::PathBuf};

        let base: Radix<OsString, &str> = Radix::new();
        let key = PathBuf::from("home/alice/.config/shed");

        let mut first_snapshot = base.clone();
        let mut second_snapshot = base.clone();

        first_snapshot.insert(key.as_path(), "first value");
        second_snapshot.insert(key.as_path(), "second value");

        assert_eq!(base.get(key.as_path()), None);
        assert_eq!(first_snapshot.get(key.as_path()), Some(&"first value"));
        assert_eq!(second_snapshot.get(key.as_path()), Some(&"second value"));
    }
}
