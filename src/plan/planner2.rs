use super::model::{Direction, Directory, File, Root};
use crate::manifest::{ChildItem, Manifest, RootItem};
use std::{
    future::Future,
    path::{Path, PathBuf},
    pin::Pin,
};

pub struct Planner {
    direction: Direction,
}

struct PlannedChildren {
    directories: Vec<Directory>,
    files: Vec<File>,
}

impl Planner {
    pub fn new(direction: Direction) -> Self {
        Self { direction }
    }

    pub async fn plan(&self, manifests: impl IntoIterator<Item = Manifest>) -> Vec<Root> {
        let mut manifests_in_order = Vec::new();
        for manifest in manifests {
            flatten_manifest(manifest, &mut manifests_in_order);
        }

        let mut roots = Vec::new();
        for manifest in manifests_in_order {
            let manifest_directory = manifest
                .path
                .parent()
                .expect("a resolved manifest path has a parent");

            for item in manifest.items {
                roots.push(self.plan_root(manifest_directory, item).await);
            }
        }
        roots
    }

    async fn plan_root(&self, manifest_directory: &Path, item: RootItem) -> Root {
        let system = PathBuf::from(item.path);
        let shed = match item.shed {
            Some(path) if Path::new(&path).is_absolute() => PathBuf::from(path),
            Some(path) => manifest_directory.join(path),
            None => manifest_directory.to_path_buf(),
        };
        let (source, destination) = self.endpoints(system.clone(), shed.clone());

        match item.items {
            Some(items) => {
                let children = self.plan_items(system, shed, items).await;
                Root {
                    dst: destination,
                    is_clean_dst: false,
                    directories: nonempty(children.directories),
                    files: nonempty(children.files),
                }
            }
            None if source.is_dir() => {
                let children = plan_entire_directory(source).await;
                Root {
                    dst: destination,
                    is_clean_dst: true,
                    directories: nonempty(children.directories),
                    files: nonempty(children.files),
                }
            }
            None => {
                let dst = destination
                    .file_name()
                    .expect("a file destination has a name")
                    .to_owned();
                let destination_directory = destination
                    .parent()
                    .expect("a file destination has a parent")
                    .to_path_buf();
                Root {
                    dst: destination_directory,
                    is_clean_dst: false,
                    directories: None,
                    files: Some(vec![File { src: source, dst }]),
                }
            }
        }
    }

    fn plan_items(
        &self,
        system_parent: PathBuf,
        shed_parent: PathBuf,
        items: Vec<ChildItem>,
    ) -> Pin<Box<dyn Future<Output = PlannedChildren> + '_>> {
        Box::pin(async move {
            let mut planned = PlannedChildren {
                directories: Vec::new(),
                files: Vec::new(),
            };

            for item in items {
                let (path, shed, children) = match item {
                    ChildItem::Path(path) => (path.clone(), path, None),
                    ChildItem::Item(item) => {
                        let shed = item.shed.unwrap_or_else(|| item.path.clone());
                        (item.path, shed, item.items)
                    }
                };

                let system = system_parent.join(&path);
                let shed_path = shed_parent.join(&shed);
                let (source, _) = self.endpoints(system.clone(), shed_path.clone());
                let destination_name = match &self.direction {
                    Direction::Put => PathBuf::from(shed).into_os_string(),
                    Direction::Get => PathBuf::from(path).into_os_string(),
                };

                match children {
                    Some(children) => {
                        let children = self.plan_items(system, shed_path, children).await;
                        planned.directories.push(Directory {
                            dst: destination_name,
                            is_clean_dst: false,
                            directories: nonempty(children.directories),
                            files: nonempty(children.files),
                        });
                    }
                    None if source.is_dir() => {
                        let children = plan_entire_directory(source).await;
                        planned.directories.push(Directory {
                            dst: destination_name,
                            is_clean_dst: true,
                            directories: nonempty(children.directories),
                            files: nonempty(children.files),
                        });
                    }
                    None => planned.files.push(File {
                        src: source,
                        dst: destination_name,
                    }),
                }
            }

            planned
        })
    }

    fn endpoints(&self, system: PathBuf, shed: PathBuf) -> (PathBuf, PathBuf) {
        match &self.direction {
            Direction::Put => (system, shed),
            Direction::Get => (shed, system),
        }
    }
}

fn flatten_manifest(mut manifest: Manifest, manifests_in_order: &mut Vec<Manifest>) {
    for included in std::mem::take(&mut manifest.manifests) {
        flatten_manifest(included, manifests_in_order);
    }
    manifests_in_order.push(manifest);
}

fn plan_entire_directory(source: PathBuf) -> Pin<Box<dyn Future<Output = PlannedChildren>>> {
    Box::pin(async move {
        let mut read_directory = tokio::fs::read_dir(&source)
            .await
            .unwrap_or_else(|error| panic!("failed to read source directory {source:?}: {error}"));
        let mut entries = Vec::new();
        while let Some(entry) = read_directory
            .next_entry()
            .await
            .unwrap_or_else(|error| panic!("failed to read source directory {source:?}: {error}"))
        {
            entries.push(entry);
        }
        entries.sort_by_key(|entry| entry.file_name());

        let mut planned = PlannedChildren {
            directories: Vec::new(),
            files: Vec::new(),
        };
        for entry in entries {
            let name = entry.file_name();
            let path = entry.path();
            let file_type = entry
                .file_type()
                .await
                .unwrap_or_else(|error| panic!("failed to inspect source path {path:?}: {error}"));

            if file_type.is_dir() {
                let children = plan_entire_directory(path).await;
                planned.directories.push(Directory {
                    dst: name,
                    is_clean_dst: true,
                    directories: nonempty(children.directories),
                    files: nonempty(children.files),
                });
            } else {
                planned.files.push(File {
                    src: path,
                    dst: name,
                });
            }
        }
        planned
    })
}

fn nonempty<T>(items: Vec<T>) -> Option<Vec<T>> {
    (!items.is_empty()).then_some(items)
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
        let shed_dir_path = manifest_path.parent().unwrap().to_path_buf();

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
                            src: shed_dir_path.join("intermediate-dir/sub-dir/file1"),
                            dst: "file1".into(),
                        },
                        File {
                            src: shed_dir_path.join("intermediate-dir/sub-dir/file2"),
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
        let shed_dir_path = manifest_path.parent().unwrap().to_path_buf();

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
                    src: shed_dir_path.join("file1"),
                    dst: "file1".into(),
                }]),
            },
            Root {
                dst: path_dir2.path().into(),
                is_clean_dst: false,
                directories: None,
                files: Some(vec![File {
                    src: shed_dir_path.join("file2"),
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
        let shed_dir_path = manifest_path.parent().unwrap().to_path_buf();

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
                        src: shed_dir_path.join("sub-dir/file1"),
                        dst: "file1".into(),
                    },
                    File {
                        src: shed_dir_path.join("sub-dir/file2"),
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
        let shed_dir_path = manifest_path.parent().unwrap().to_path_buf();

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
                    src: shed_dir_path.join("shed-dir/file1"),
                    dst: "file1".into(),
                },
                File {
                    src: shed_dir_path.join("shed-dir/file2"),
                    dst: "file2".into(),
                },
            ]),
        }];

        assert_eq!(actual, expected)
    }
}
