use crate::manifest::{FullIndex as ManifestIndex, Manifest};

use super::model::{Direction, NestedDirectory, NestedLeaf, Root, RootDirectory, RootLeaf};

pub struct Planner {
    direction: Direction,
}

impl Planner {
    pub fn new(direction: Direction) -> Self {
        Self { direction }
    }

    pub fn plan(&self, manifests: impl IntoIterator<Item = Manifest>) -> Vec<Root> {
        todo!()
    }
}

pub fn plan(direction: Direction, manifests: impl IntoIterator<Item = Manifest>) -> Vec<Root> {
    Planner::new(direction).plan(manifests)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{Input, testing as testing_m};
    use indoc::formatdoc;
    use tempfile::TempDir;

    // #[tokio::test]
    async fn plans_nested_explicit_leaves_for_get() {
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
                    - path: dir1
                      items:
                        - path: dir2
                          items:
                            - leaf1
                            - leaf2
            "},
        );

        let (manifests, resolve_events) = testing_m::resolve(vec![Input {
            name: "manifest".into(),
            path: manifest_path,
        }])
        .await;

        testing_m::assert_events_contain_no_errors(&resolve_events);
        testing_m::assert_events_contain_done_last(&resolve_events);

        let actual = plan(Direction::Get, manifests);
        let expected = vec![Root::Directory(RootDirectory {
            manifest_indexes: [([0], [0]).into()].into(),
            dst: path_dir.path().into(),
            directories: vec![NestedDirectory {
                manifest_indexes: [([0], [0, 0]).into()].into(),
                dst: "dir1".into(),
                directories: vec![NestedDirectory {
                    manifest_indexes: [([0], [0, 0, 0]).into()].into(),
                    dst: "dir2".into(),
                    directories: Vec::new(),
                    leaves: vec![
                        NestedLeaf {
                            manifest_index: ([0], [0, 0, 0, 0]).into(),
                            src: shed_dir.path().join("dir1/dir2/leaf1"),
                            dst: "leaf1".into(),
                        },
                        NestedLeaf {
                            manifest_index: ([0], [0, 0, 0, 1]).into(),
                            src: shed_dir.path().join("dir1/dir2/leaf2"),
                            dst: "leaf2".into(),
                        },
                    ],
                }],
                leaves: Vec::new(),
            }],
            leaves: Vec::new(),
        })];

        assert_eq!(actual, expected);
    }

    // #[tokio::test]
    async fn plans_two_root_leaves_for_get() {
        let path_dir = TempDir::new().unwrap();
        let shed_dir = TempDir::new().unwrap();

        let path_dir_display = path_dir.path().display();
        let manifest_path = testing_m::write_yaml_manifest(
            shed_dir.path(),
            "manifest",
            formatdoc! {"
              items:
                - path: {path_dir_display}/leaf1
                  shed: leaf1
                - path: {path_dir_display}/leaf2
                  shed: leaf2
            "},
        );

        let (manifests, resolve_events) = testing_m::resolve(vec![Input {
            name: "manifest".into(),
            path: manifest_path,
        }])
        .await;

        testing_m::assert_events_contain_no_errors(&resolve_events);
        testing_m::assert_events_contain_done_last(&resolve_events);

        let actual = plan(Direction::Get, manifests);
        let expected = vec![
            Root::Leaf(RootLeaf {
                manifest_index: ([0], [0]).into(),
                src: shed_dir.path().join("leaf1"),
                dst: path_dir.path().join("leaf1"),
            }),
            Root::Leaf(RootLeaf {
                manifest_index: ([0], [1]).into(),
                src: shed_dir.path().join("leaf2"),
                dst: path_dir.path().join("leaf2"),
            }),
        ];

        assert_eq!(actual, expected);
    }
}
