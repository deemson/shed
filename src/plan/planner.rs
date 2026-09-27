use std::path::PathBuf;

use super::model::{Direction, Directory, File, Root};
use crate::manifest::{ChildItem, Item, Manifest};

pub struct Planner {
    direction: Direction,
}

struct PlannedOperations {
    directory_removals: Option<Vec<PlannedDirectoryRemoval>>,
    file_copies: Option<Vec<PlannedFileCopy>>,
}

struct PlannedDirectoryRemoval {
    dst: PathBuf,
}

struct PlannedFileCopy {
    src: PathBuf,
    dst: PathBuf,
}

impl Planner {
    pub fn new(direction: Direction) -> Self {
        Self { direction }
    }

    pub async fn plan(&self, manifests: impl IntoIterator<Item = Manifest>) -> Vec<Root> {
        todo!()
    }

    async fn plan_manifests(
        &self,
        manifests: impl IntoIterator<Item = Manifest>,
    ) -> Vec<(Vec<Directory>, Vec<File>)> {
        todo!()
    }

    async fn plan_manifest(&self, manifest: Manifest) -> (Vec<Directory>, Vec<File>) {
        todo!()
    }

    async fn plan_child_items(
        &self,
        path_parent: PathBuf,
        shed_parent: PathBuf,
        child_items: Vec<ChildItem>,
    ) -> PlannedOperations {
        todo!()
    }

    async fn plan_child_item(&self, child_item: ChildItem) -> Root {
        todo!()
    }

    async fn plan_item(
        &self,
        path_parent: PathBuf,
        shed_parent: PathBuf,
        item: Item,
    ) -> PlannedOperations {
        let path = path_parent.join(&item.path);
        let shed = shed_parent.join(item.shed.as_ref().unwrap_or(&item.path));
        if let Some(child_items) = item.items {
            return self.plan_child_items(path, shed, child_items).await;
        }
        let (src, dst) = self.resolve_direction(path, shed);
        todo!()
    }

    fn resolve_direction(&self, path: PathBuf, shed: PathBuf) -> (PathBuf, PathBuf) {
        match &self.direction {
            Direction::Put => (path, shed),
            Direction::Get => (shed, path),
        }
    }
}

async fn plan_directory_item(src: PathBuf, dst: PathBuf) -> Vec<PlannedFileCopy> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::super::testing as testing_p;
    use crate::{
        manifest::Input,
        manifest::testing as testing_m,
        plan::model::{Direction, Directory, File, Root},
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

        testing_m::write(
            &shed_dir.path().join("intermediate-dir/sub-dir/file1"),
            "file1 contents",
        );
        testing_m::write(
            &shed_dir.path().join("intermediate-dir/sub-dir/file2"),
            "file2 contents",
        );

        let (manifests, resolve_events) = testing_m::resolve(vec![Input {
            name: "manifest".into(),
            path: manifest_path,
        }])
        .await;
        let actual = testing_p::plan(Direction::Get, manifests).await;

        testing_m::assert_events_contain_no_errors(&resolve_events);
        testing_m::assert_events_contain_done_last(&resolve_events);

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

        assert_eq!(actual, expected)
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

        testing_m::write(&shed_dir.path().join("file1"), "file1 contents");
        testing_m::write(&shed_dir.path().join("file2"), "file2 contents");

        let (manifests, resolve_events) = testing_m::resolve(vec![Input {
            name: "manifest".into(),
            path: manifest_path,
        }])
        .await;
        let actual = testing_p::plan(Direction::Get, manifests).await;

        testing_m::assert_events_contain_no_errors(&resolve_events);
        testing_m::assert_events_contain_done_last(&resolve_events);

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

        assert_eq!(actual, expected)
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

        testing_m::write(&shed_dir.path().join("sub-dir/file1"), "file1 contents");
        testing_m::write(&shed_dir.path().join("sub-dir/file2"), "file2 contents");

        let (manifests, resolve_events) = testing_m::resolve(vec![Input {
            name: "manifest".into(),
            path: manifest_path,
        }])
        .await;
        let actual = testing_p::plan(Direction::Get, manifests).await;

        testing_m::assert_events_contain_no_errors(&resolve_events);
        testing_m::assert_events_contain_done_last(&resolve_events);

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

        assert_eq!(actual, expected)
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

        testing_m::write(&shed_dir.path().join("shed-dir/file1"), "file1 contents");
        testing_m::write(&shed_dir.path().join("shed-dir/file2"), "file2 contents");

        let (manifests, resolve_events) = testing_m::resolve(vec![Input {
            name: "manifest".into(),
            path: manifest_path,
        }])
        .await;
        let actual = testing_p::plan(Direction::Get, manifests).await;

        testing_m::assert_events_contain_no_errors(&resolve_events);
        testing_m::assert_events_contain_done_last(&resolve_events);

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

        assert_eq!(actual, expected)
    }
}
