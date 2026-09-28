use std::path::PathBuf;

use async_walkdir::WalkDir;
use futures::{StreamExt, future::join_all};

use super::error::Error;
use super::event::Event;
use super::model::{Direction, Directory, File, Root};
use crate::manifest::{ChildItem, Item, Manifest};
use tokio::sync::mpsc;

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

pub struct Planner {
    direction: Direction,
    event_sender: mpsc::Sender<Event>,
}

impl Planner {
    pub fn new(direction: Direction, event_sender: mpsc::Sender<Event>) -> Self {
        Self {
            direction,
            event_sender,
        }
    }

    pub async fn plan(&self, manifests: impl IntoIterator<Item = Manifest>) -> Vec<Root> {
        todo!()
    }

    async fn plan_manifests(
        &self,
        path_parent: PathBuf,
        shed_parent: PathBuf,
        manifests: impl IntoIterator<Item = Manifest>,
    ) -> Option<PlannedOperations> {
        let plans = join_all(manifests.into_iter().map(|manifest| {
            self.plan_manifest(path_parent.clone(), shed_parent.clone(), manifest)
        }))
        .await;

        consolidate_plans(plans)
    }

    async fn plan_manifest(
        &self,
        path_parent: PathBuf,
        shed_parent: PathBuf,
        manifest: Manifest,
    ) -> Option<PlannedOperations> {
        let child_items = manifest.items.into_iter().map(|item| {
            ChildItem::Item(Item {
                path: item.path,
                shed: item.shed,
                items: item.items,
            })
        });

        let (manifest_plan, item_plan) = futures::join!(
            self.plan_manifests(path_parent.clone(), shed_parent.clone(), manifest.manifests),
            self.plan_child_items(path_parent, shed_parent, child_items.collect()),
        );

        consolidate_plans([manifest_plan, item_plan])
    }

    async fn plan_child_items(
        &self,
        path_parent: PathBuf,
        shed_parent: PathBuf,
        child_items: Vec<ChildItem>,
    ) -> Option<PlannedOperations> {
        let plans = join_all(child_items.into_iter().map(|child_item| {
            self.plan_child_item(path_parent.clone(), shed_parent.clone(), child_item)
        }))
        .await;

        consolidate_plans(plans)
    }

    async fn plan_child_item(
        &self,
        path_parent: PathBuf,
        shed_parent: PathBuf,
        child_item: ChildItem,
    ) -> Option<PlannedOperations> {
        match child_item {
            ChildItem::Path(path) => {
                self.plan_item(
                    path_parent,
                    shed_parent,
                    Item {
                        path,
                        shed: None,
                        items: None,
                    },
                )
                .await
            }
            ChildItem::Item(item) => self.plan_item(path_parent, shed_parent, item).await,
        }
    }

    async fn plan_item(
        &self,
        path_parent: PathBuf,
        shed_parent: PathBuf,
        item: Item,
    ) -> Option<PlannedOperations> {
        let path = path_parent.join(&item.path);
        let shed = shed_parent.join(item.shed.as_ref().unwrap_or(&item.path));
        if let Some(child_items) = item.items {
            return self.plan_child_items(path, shed, child_items).await;
        }
        let (src, dst) = self.resolve_direction(path, shed);
        match (src.is_dir(), dst.is_dir()) {
            (true, true) => match plan_directory_item(src, dst.clone()).await {
                Ok(file_copies) => Some(PlannedOperations {
                    directory_removals: Some(vec![PlannedDirectoryRemoval { dst }]),
                    file_copies: Some(file_copies),
                }),
                Err(error) => self.error_plan(error).await,
            },
            (true, false) => self.error_plan(Error::SrcDirDstNot).await,
            (false, true) => self.error_plan(Error::DstDirSrcNot).await,
            (false, false) => Some(PlannedOperations {
                directory_removals: None,
                file_copies: Some(vec![PlannedFileCopy { src, dst }]),
            }),
        }
    }

    fn resolve_direction(&self, path: PathBuf, shed: PathBuf) -> (PathBuf, PathBuf) {
        match &self.direction {
            Direction::Put => (path, shed),
            Direction::Get => (shed, path),
        }
    }

    async fn error_plan(&self, error: Error) -> Option<PlannedOperations> {
        self.send_event(Event::Error { error }).await;
        None
    }

    async fn send_event(&self, event: Event) {
        let _ = self.event_sender.send(event).await;
    }
}

fn consolidate_plans(
    plans: impl IntoIterator<Item = Option<PlannedOperations>>,
) -> Option<PlannedOperations> {
    let mut directory_removals = Vec::new();
    let mut file_copies = Vec::new();

    for plan in plans.into_iter().flatten() {
        if let Some(mut removals) = plan.directory_removals {
            directory_removals.append(&mut removals);
        }
        if let Some(mut copies) = plan.file_copies {
            file_copies.append(&mut copies);
        }
    }

    if directory_removals.is_empty() && file_copies.is_empty() {
        None
    } else {
        Some(PlannedOperations {
            directory_removals: (!directory_removals.is_empty()).then_some(directory_removals),
            file_copies: (!file_copies.is_empty()).then_some(file_copies),
        })
    }
}

async fn plan_directory_item(src: PathBuf, dst: PathBuf) -> Result<Vec<PlannedFileCopy>, Error> {
    let mut entries = WalkDir::new(&src);
    let mut file_copies = Vec::new();

    while let Some(entry) = entries.next().await {
        let entry = entry.map_err(|source| Error::WalkDirectory { source })?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .await
            .map_err(|source| Error::InspectSrcPath { source })?;

        if !file_type.is_dir() {
            let relative = path
                .strip_prefix(&src)
                .expect("a walked path is beneath its source directory");
            let destination = dst.join(relative);
            file_copies.push(PlannedFileCopy {
                src: path,
                dst: destination,
            });
        }
    }

    Ok(file_copies)
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

        testing_m::assert_events_contain_no_errors(&resolve_events);
        testing_m::assert_events_contain_done_last(&resolve_events);

        let (actual, _) = testing_p::plan(Direction::Get, manifests).await;

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

        testing_m::assert_events_contain_no_errors(&resolve_events);
        testing_m::assert_events_contain_done_last(&resolve_events);

        let (actual, _) = testing_p::plan(Direction::Get, manifests).await;

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

        testing_m::assert_events_contain_no_errors(&resolve_events);
        testing_m::assert_events_contain_done_last(&resolve_events);

        let (actual, _) = testing_p::plan(Direction::Get, manifests).await;

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

        testing_m::assert_events_contain_no_errors(&resolve_events);
        testing_m::assert_events_contain_done_last(&resolve_events);

        let (actual, _) = testing_p::plan(Direction::Get, manifests).await;

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
