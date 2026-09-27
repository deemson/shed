pub struct Planner {}

impl Planner {}

#[cfg(test)]
mod tests {
    use crate::{
        manifest::testing as testing_m,
        plan::model::{Directory, File, Root},
    };
    use indoc::formatdoc;
    use tempfile::TempDir;

    #[tokio::test]
    async fn plans_nested_explicit_files_without_cleaning_destinations() {
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
        let expected = [Root {
            dst: path_dir.path().into(),
            is_clean_dst: false,
            directories: Some(vec![Directory {
                dst: "intermediate-dir".into(),
                is_clean_dst: false,
                directories: Some(vec![Directory {
                    dst: "sub-dir".into(),
                    is_clean_dst: false,
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
                }]),
                files: None,
            }]),
            files: None,
        }];
    }

    #[tokio::test]
    async fn plans_multiple_roots_separately() {
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
            Root {
                dst: path_dir1.path().into(),
                is_clean_dst: false,
                directories: None,
                files: Some(vec![File {
                    src: shed_dir.path().join("file1"),
                    dst: "file1".into(),
                }]),
            },
            Root {
                dst: path_dir2.path().into(),
                is_clean_dst: false,
                directories: None,
                files: Some(vec![File {
                    src: shed_dir.path().join("file2"),
                    dst: "file2".into(),
                }]),
            },
        ];
    }

    #[tokio::test]
    async fn plans_an_entire_directory_and_marks_its_destination_for_cleaning() {
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
        let expected = [Root {
            dst: path_dir.path().into(),
            is_clean_dst: false,
            directories: Some(vec![Directory {
                dst: "sub-dir".into(),
                is_clean_dst: true,
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
            }]),
            files: None,
        }];
    }

    #[tokio::test]
    async fn plans_an_entire_root_directory_and_marks_its_destination_for_cleaning() {
        let path_dir = TempDir::new().unwrap();
        let shed_dir = TempDir::new().unwrap();

        let path_dir_display = path_dir.path().display();
        let manifest_path = testing_m::write_yaml_manifest(
            shed_dir.path(),
            "manifest",
            formatdoc! {"
              items:
                - path: {path_dir_display}
                  shed: shed-dir
            "},
        );

        let expected = [Root {
            dst: path_dir.path().into(),
            is_clean_dst: true,
            directories: None,
            files: Some(vec![
                File {
                    src: shed_dir.path().join("shed-dir/file1"),
                    dst: "file1".into(),
                },
                File {
                    src: shed_dir.path().join("shed-dir/file2"),
                    dst: "file2".into(),
                },
            ]),
        }];
    }
}
