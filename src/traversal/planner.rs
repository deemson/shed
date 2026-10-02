use crate::manifest::{FullIndex as ManifestIndex, Manifest};

use super::error::Error;
use super::model::{Direction, NestedDirectory, NestedLeaf, Plan, Root, RootDirectory, RootLeaf};

pub struct Planner {
    direction: Direction,
}

impl Planner {
    pub fn new(direction: Direction) -> Self {
        Self { direction }
    }

    pub fn plan(&self, manifests: impl IntoIterator<Item = Manifest>) -> Plan {
        todo!()
    }
}

pub fn plan(direction: Direction, manifests: impl IntoIterator<Item = Manifest>) -> Plan {
    Planner::new(direction).plan(manifests)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{Input, testing as testing_m};
    use indoc::formatdoc;
    use tempfile::TempDir;

    #[tokio::test]
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
        let expected = Plan {
            roots: vec![Root::Directory(RootDirectory {
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
            })],
            errors: Vec::new(),
        };

        assert_eq!(actual, expected);
    }

    #[tokio::test]
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
        let expected = Plan {
            roots: vec![
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
            ],
            errors: Vec::new(),
        };

        assert_eq!(actual, expected);
    }

    #[tokio::test]
    async fn plans_bare_leaves_under_a_manifest_level_path_for_get() {
        let path_dir = TempDir::new().unwrap();
        let shed_dir = TempDir::new().unwrap();

        let path_dir_display = path_dir.path().display();
        let manifest_path = testing_m::write_yaml_manifest(
            shed_dir.path(),
            "manifest",
            formatdoc! {"
              path: {path_dir_display}
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
        let expected = Plan {
            roots: vec![Root::Directory(RootDirectory {
                manifest_indexes: [([0], []).into()].into(),
                dst: path_dir.path().into(),
                directories: Vec::new(),
                leaves: vec![
                    NestedLeaf {
                        manifest_index: ([0], [0]).into(),
                        src: shed_dir.path().join("leaf1"),
                        dst: "leaf1".into(),
                    },
                    NestedLeaf {
                        manifest_index: ([0], [1]).into(),
                        src: shed_dir.path().join("leaf2"),
                        dst: "leaf2".into(),
                    },
                ],
            })],
            errors: Vec::new(),
        };

        assert_eq!(actual, expected);
    }

    #[tokio::test]
    async fn plans_overlapping_manifest_paths_with_traversal_dependencies_for_get() {
        let path_dir = TempDir::new().unwrap();
        let shed_dir = TempDir::new().unwrap();

        let path_dir_display = path_dir.path().display();
        let main_manifest_path = testing_m::write_yaml_manifest(
            shed_dir.path(),
            "main-manifest",
            formatdoc! {"
              include:
                - manifest1
                - manifest2
            "},
        );
        let _ = testing_m::write_yaml_manifest(
            shed_dir.path(),
            "manifest1",
            formatdoc! {"
              path: {path_dir_display}/sub-dir1/sub-dir2
              items:
                - sub-dir-leaf
            "},
        );
        let _ = testing_m::write_yaml_manifest(
            shed_dir.path(),
            "manifest2",
            formatdoc! {"
              path: {path_dir_display}
              items:
                - top-leaf
            "},
        );

        let (manifests, resolve_events) = testing_m::resolve(vec![Input {
            name: "main-manifest".into(),
            path: main_manifest_path,
        }])
        .await;

        testing_m::assert_events_contain_no_errors(&resolve_events);
        testing_m::assert_events_contain_done_last(&resolve_events);

        let actual = plan(Direction::Get, manifests);
        let expected = Plan {
            roots: vec![Root::Directory(RootDirectory {
                manifest_indexes: [([0, 0], []).into(), ([0, 1], []).into()].into(),
                dst: path_dir.path().into(),
                directories: vec![NestedDirectory {
                    manifest_indexes: [([0, 0], []).into()].into(),
                    dst: "sub-dir1".into(),
                    directories: vec![NestedDirectory {
                        manifest_indexes: [([0, 0], []).into()].into(),
                        dst: "sub-dir2".into(),
                        directories: Vec::new(),
                        leaves: vec![NestedLeaf {
                            manifest_index: ([0, 0], [0]).into(),
                            src: shed_dir.path().join("sub-dir-leaf"),
                            dst: "sub-dir-leaf".into(),
                        }],
                    }],
                    leaves: Vec::new(),
                }],
                leaves: vec![NestedLeaf {
                    manifest_index: ([0, 1], [0]).into(),
                    src: shed_dir.path().join("top-leaf"),
                    dst: "top-leaf".into(),
                }],
            })],
            errors: Vec::new(),
        };

        assert_eq!(actual, expected);
    }

    #[tokio::test]
    async fn does_not_merge_manifest_paths_that_only_share_a_common_ancestor_for_get() {
        let path_dir = TempDir::new().unwrap();
        let shed_dir = TempDir::new().unwrap();

        let path_dir_display = path_dir.path().display();
        let main_manifest_path = testing_m::write_yaml_manifest(
            shed_dir.path(),
            "main-manifest",
            formatdoc! {"
              include:
                - manifest1
                - manifest2
            "},
        );
        let _ = testing_m::write_yaml_manifest(
            shed_dir.path(),
            "manifest1",
            formatdoc! {"
              path: {path_dir_display}/sub-dir1
              items:
                - sub-dir1-leaf
            "},
        );
        let _ = testing_m::write_yaml_manifest(
            shed_dir.path(),
            "manifest2",
            formatdoc! {"
              path: {path_dir_display}/sub-dir2
              items:
                - sub-dir2-leaf
            "},
        );

        let (manifests, resolve_events) = testing_m::resolve(vec![Input {
            name: "main-manifest".into(),
            path: main_manifest_path,
        }])
        .await;

        testing_m::assert_events_contain_no_errors(&resolve_events);
        testing_m::assert_events_contain_done_last(&resolve_events);

        let actual = plan(Direction::Get, manifests);
        let expected = Plan {
            roots: vec![
                Root::Directory(RootDirectory {
                    manifest_indexes: [([0, 0], []).into()].into(),
                    dst: path_dir.path().join("sub-dir1"),
                    directories: Vec::new(),
                    leaves: vec![NestedLeaf {
                        manifest_index: ([0, 0], [0]).into(),
                        src: shed_dir.path().join("sub-dir1-leaf"),
                        dst: "sub-dir1-leaf".into(),
                    }],
                }),
                Root::Directory(RootDirectory {
                    manifest_indexes: [([0, 1], []).into()].into(),
                    dst: path_dir.path().join("sub-dir2"),
                    directories: Vec::new(),
                    leaves: vec![NestedLeaf {
                        manifest_index: ([0, 1], [0]).into(),
                        src: shed_dir.path().join("sub-dir2-leaf"),
                        dst: "sub-dir2-leaf".into(),
                    }],
                }),
            ],
            errors: Vec::new(),
        };

        assert_eq!(actual, expected);
    }

    #[tokio::test]
    async fn omits_overlapping_leaves_and_plans_unrelated_leaves_for_get() {
        let path_dir = TempDir::new().unwrap();
        let shed_dir = TempDir::new().unwrap();

        let path_dir_display = path_dir.path().display();
        let manifest_path = testing_m::write_yaml_manifest(
            shed_dir.path(),
            "manifest",
            formatdoc! {"
              items:
                - path: {path_dir_display}/valid-leaf
                  shed: valid-leaf
                - path: {path_dir_display}/leaf
                  shed: leaf1
                - path: {path_dir_display}/leaf
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
        let expected = Plan {
            roots: vec![Root::Leaf(RootLeaf {
                manifest_index: ([0], [0]).into(),
                src: shed_dir.path().join("valid-leaf"),
                dst: path_dir.path().join("valid-leaf"),
            })],
            errors: vec![Error::OverlappingLeaves {
                manifest_indexes: [([0], [1]).into(), ([0], [2]).into()].into(),
            }],
        };

        assert_eq!(actual, expected);
    }

    #[tokio::test]
    async fn omits_ancestor_and_descendant_leaves_and_plans_unrelated_leaves_for_get() {
        let path_dir = TempDir::new().unwrap();
        let shed_dir = TempDir::new().unwrap();

        let path_dir_display = path_dir.path().display();
        let manifest_path = testing_m::write_yaml_manifest(
            shed_dir.path(),
            "manifest",
            formatdoc! {"
              items:
                - path: {path_dir_display}/valid-leaf
                  shed: valid-leaf
                - path: {path_dir_display}/overlap
                  shed: ancestor-leaf
                - path: {path_dir_display}/overlap/descendant
                  shed: descendant-leaf
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
        let expected = Plan {
            roots: vec![Root::Leaf(RootLeaf {
                manifest_index: ([0], [0]).into(),
                src: shed_dir.path().join("valid-leaf"),
                dst: path_dir.path().join("valid-leaf"),
            })],
            errors: vec![Error::OverlappingLeaves {
                manifest_indexes: [([0], [1]).into(), ([0], [2]).into()].into(),
            }],
        };

        assert_eq!(actual, expected);
    }
}
