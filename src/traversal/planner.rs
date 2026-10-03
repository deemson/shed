use super::{
    error::Error,
    model::{Direction, Directory, Leaf, Plan, Root},
    radix,
};
use crate::manifest::Manifest;

pub struct Planner {
    direction: Direction,
}

impl Planner {
    pub fn new(direction: Direction) -> Self {
        Self { direction }
    }

    pub fn plan(&self, manifests: impl IntoIterator<Item = Manifest>) -> Plan {
        let radix_plan = radix::Planner::new(self.direction.clone()).plan(manifests);
        let _ = radix_plan;
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
    async fn plans_two_root_leaves_for_get() {
        let temp_dir = TempDir::new().unwrap();
        let path_dir = temp_dir.path().join("path");
        let shed_dir = temp_dir.path().join("shed");

        let path_dir_display = path_dir.display();
        let manifest_path = testing_m::write_yaml_manifest(
            &shed_dir,
            "manifest",
            formatdoc! {"
              items:
                - path: {path_dir_display}/leaf1
                  shed: leaf1
                - path: {path_dir_display}/leaf2
                  shed: leaf2
            "},
        );
        let manifest_directory = manifest_path
            .parent()
            .expect("a manifest path has a parent")
            .to_path_buf();

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
                Root::Leaf {
                    index: ([0], [0]).into(),
                    src: manifest_directory.join("leaf1"),
                    dst: path_dir.join("leaf1"),
                },
                Root::Leaf {
                    index: ([0], [1]).into(),
                    src: manifest_directory.join("leaf2"),
                    dst: path_dir.join("leaf2"),
                },
            ],
            errors: Vec::new(),
        };

        assert_eq!(actual, expected);
    }

    #[tokio::test]
    async fn plans_nested_explicit_leaves_for_get() {
        let temp_dir = TempDir::new().unwrap();
        let path_dir = temp_dir.path().join("path");
        let shed_dir = temp_dir.path().join("shed");

        let path_dir_display = path_dir.display();
        let manifest_path = testing_m::write_yaml_manifest(
            &shed_dir,
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
        let manifest_directory = manifest_path
            .parent()
            .expect("a manifest path has a parent")
            .to_path_buf();

        let (manifests, resolve_events) = testing_m::resolve(vec![Input {
            name: "manifest".into(),
            path: manifest_path,
        }])
        .await;

        testing_m::assert_events_contain_no_errors(&resolve_events);
        testing_m::assert_events_contain_done_last(&resolve_events);

        let actual = plan(Direction::Get, manifests);
        let expected = Plan {
            roots: vec![Root::Directory {
                dst: path_dir,
                directories: vec![Directory {
                    dst: "dir1".into(),
                    directories: vec![Directory {
                        dst: "dir2".into(),
                        directories: Vec::new(),
                        leaves: vec![
                            Leaf {
                                index: ([0], [0, 0, 0, 0]).into(),
                                src: manifest_directory.join("dir1/dir2/leaf1"),
                                dst: "leaf1".into(),
                            },
                            Leaf {
                                index: ([0], [0, 0, 0, 1]).into(),
                                src: manifest_directory.join("dir1/dir2/leaf2"),
                                dst: "leaf2".into(),
                            },
                        ],
                    }],
                    leaves: Vec::new(),
                }],
                leaves: Vec::new(),
            }],
            errors: Vec::new(),
        };

        assert_eq!(actual, expected);
    }

    #[tokio::test]
    async fn plans_bare_leaves_under_a_manifest_level_path_for_get() {
        let temp_dir = TempDir::new().unwrap();
        let path_dir = temp_dir.path().join("path");
        let shed_dir = temp_dir.path().join("shed");

        let path_dir_display = path_dir.display();
        let manifest_path = testing_m::write_yaml_manifest(
            &shed_dir,
            "manifest",
            formatdoc! {"
              path: {path_dir_display}
              items:
                - leaf1
                - leaf2
            "},
        );
        let manifest_directory = manifest_path
            .parent()
            .expect("a manifest path has a parent")
            .to_path_buf();

        let (manifests, resolve_events) = testing_m::resolve(vec![Input {
            name: "manifest".into(),
            path: manifest_path,
        }])
        .await;

        testing_m::assert_events_contain_no_errors(&resolve_events);
        testing_m::assert_events_contain_done_last(&resolve_events);

        let actual = plan(Direction::Get, manifests);
        let expected = Plan {
            roots: vec![Root::Directory {
                dst: path_dir,
                directories: Vec::new(),
                leaves: vec![
                    Leaf {
                        index: ([0], [0]).into(),
                        src: manifest_directory.join("leaf1"),
                        dst: "leaf1".into(),
                    },
                    Leaf {
                        index: ([0], [1]).into(),
                        src: manifest_directory.join("leaf2"),
                        dst: "leaf2".into(),
                    },
                ],
            }],
            errors: Vec::new(),
        };

        assert_eq!(actual, expected);
    }

    #[tokio::test]
    async fn plans_overlapping_manifest_paths_with_traversal_dependencies_for_get() {
        let temp_dir = TempDir::new().unwrap();
        let path_dir = temp_dir.path().join("path");
        let shed_dir = temp_dir.path().join("shed");

        let path_dir_display = path_dir.display();
        let main_manifest_path = testing_m::write_yaml_manifest(
            &shed_dir,
            "main-manifest",
            formatdoc! {"
              include:
                - manifest1
                - manifest2
            "},
        );
        let _ = testing_m::write_yaml_manifest(
            &shed_dir,
            "manifest1",
            formatdoc! {"
              path: {path_dir_display}/sub-dir1/sub-dir2
              items:
                - sub-dir-leaf
            "},
        );
        let _ = testing_m::write_yaml_manifest(
            &shed_dir,
            "manifest2",
            formatdoc! {"
              path: {path_dir_display}
              items:
                - top-leaf
            "},
        );
        let manifest_directory = main_manifest_path
            .parent()
            .expect("a manifest path has a parent")
            .to_path_buf();

        let (manifests, resolve_events) = testing_m::resolve(vec![Input {
            name: "manifest".into(),
            path: main_manifest_path,
        }])
        .await;

        testing_m::assert_events_contain_no_errors(&resolve_events);
        testing_m::assert_events_contain_done_last(&resolve_events);

        let actual = plan(Direction::Get, manifests);
        let expected = Plan {
            roots: vec![Root::Directory {
                dst: path_dir,
                directories: vec![Directory {
                    dst: "sub-dir1".into(),
                    directories: vec![Directory {
                        dst: "sub-dir2".into(),
                        directories: Vec::new(),
                        leaves: vec![Leaf {
                            index: ([0, 0], [0]).into(),
                            src: manifest_directory.join("sub-dir-leaf"),
                            dst: "sub-dir-leaf".into(),
                        }],
                    }],
                    leaves: Vec::new(),
                }],
                leaves: vec![Leaf {
                    index: ([0, 1], [0]).into(),
                    src: manifest_directory.join("top-leaf"),
                    dst: "top-leaf".into(),
                }],
            }],
            errors: Vec::new(),
        };

        assert_eq!(actual, expected);
    }

    #[tokio::test]
    async fn does_not_merge_manifest_paths_that_only_share_a_common_ancestor_for_get() {
        let temp_dir = TempDir::new().unwrap();
        let path_dir = temp_dir.path().join("path");
        let shed_dir = temp_dir.path().join("shed");

        let path_dir_display = path_dir.display();
        let main_manifest_path = testing_m::write_yaml_manifest(
            &shed_dir,
            "main-manifest",
            formatdoc! {"
              include:
                - manifest1
                - manifest2
            "},
        );
        let _ = testing_m::write_yaml_manifest(
            &shed_dir,
            "manifest1",
            formatdoc! {"
              path: {path_dir_display}/sub-dir1
              items:
                - sub-dir1-leaf
            "},
        );
        let _ = testing_m::write_yaml_manifest(
            &shed_dir,
            "manifest2",
            formatdoc! {"
              path: {path_dir_display}/sub-dir2
              items:
                - sub-dir2-leaf
            "},
        );
        let manifest_directory = main_manifest_path
            .parent()
            .expect("a manifest path has a parent")
            .to_path_buf();

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
                Root::Directory {
                    dst: path_dir.join("sub-dir1"),
                    directories: Vec::new(),
                    leaves: vec![Leaf {
                        index: ([0, 0], [0]).into(),
                        src: manifest_directory.join("sub-dir1-leaf"),
                        dst: "sub-dir1-leaf".into(),
                    }],
                },
                Root::Directory {
                    dst: path_dir.join("sub-dir2"),
                    directories: Vec::new(),
                    leaves: vec![Leaf {
                        index: ([0, 1], [0]).into(),
                        src: manifest_directory.join("sub-dir2-leaf"),
                        dst: "sub-dir2-leaf".into(),
                    }],
                },
            ],
            errors: Vec::new(),
        };

        assert_eq!(actual, expected);
    }

    #[tokio::test]
    async fn omits_overlapping_leaves_and_plans_unrelated_leaves_for_get() {
        let temp_dir = TempDir::new().unwrap();
        let path_dir = temp_dir.path().join("path");
        let shed_dir = temp_dir.path().join("shed");

        let path_dir_display = path_dir.display();
        let manifest_path = testing_m::write_yaml_manifest(
            &shed_dir,
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
        let manifest_directory = manifest_path
            .parent()
            .expect("a manifest path has a parent")
            .to_path_buf();

        let (manifests, resolve_events) = testing_m::resolve(vec![Input {
            name: "manifest".into(),
            path: manifest_path,
        }])
        .await;

        testing_m::assert_events_contain_no_errors(&resolve_events);
        testing_m::assert_events_contain_done_last(&resolve_events);

        let actual = plan(Direction::Get, manifests);
        let expected = Plan {
            roots: vec![Root::Leaf {
                index: ([0], [0]).into(),
                src: manifest_directory.join("valid-leaf"),
                dst: path_dir.join("valid-leaf"),
            }],
            errors: vec![Error::OverlappingLeaves {
                indexes: [([0], [1]).into(), ([0], [2]).into()].into(),
            }],
        };

        assert_eq!(actual, expected);
    }

    #[tokio::test]
    async fn omits_ancestor_and_descendant_leaves_and_plans_unrelated_leaves_for_get() {
        let temp_dir = TempDir::new().unwrap();
        let path_dir = temp_dir.path().join("path");
        let shed_dir = temp_dir.path().join("shed");

        let path_dir_display = path_dir.display();
        let manifest_path = testing_m::write_yaml_manifest(
            &shed_dir,
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
        let manifest_directory = manifest_path
            .parent()
            .expect("a manifest path has a parent")
            .to_path_buf();

        let (manifests, resolve_events) = testing_m::resolve(vec![Input {
            name: "manifest".into(),
            path: manifest_path,
        }])
        .await;

        testing_m::assert_events_contain_no_errors(&resolve_events);
        testing_m::assert_events_contain_done_last(&resolve_events);

        let actual = plan(Direction::Get, manifests);
        let expected = Plan {
            roots: vec![Root::Leaf {
                index: ([0], [0]).into(),
                src: manifest_directory.join("valid-leaf"),
                dst: path_dir.join("valid-leaf"),
            }],
            errors: vec![Error::OverlappingLeaves {
                indexes: [([0], [1]).into(), ([0], [2]).into()].into(),
            }],
        };

        assert_eq!(actual, expected);
    }

    #[tokio::test]
    async fn omits_non_root_items_with_absolute_path_and_shed_and_plans_unrelated_items_for_get() {
        let temp_dir = TempDir::new().unwrap();
        let path_dir = temp_dir.path().join("path");
        let shed_dir = temp_dir.path().join("shed");

        let path_dir_display = path_dir.display();
        let shed_dir_display = shed_dir.display();
        let manifest_path = testing_m::write_yaml_manifest(
            &shed_dir,
            "manifest",
            formatdoc! {"
              items:
                - path: {path_dir_display}
                  items:
                    - valid-leaf
                    - path: {path_dir_display}/invalid-leaf
                      shed: {shed_dir_display}/invalid-leaf
            "},
        );
        let manifest_directory = manifest_path
            .parent()
            .expect("a manifest path has a parent")
            .to_path_buf();

        let (manifests, resolve_events) = testing_m::resolve(vec![Input {
            name: "manifest".into(),
            path: manifest_path,
        }])
        .await;

        testing_m::assert_events_contain_no_errors(&resolve_events);
        testing_m::assert_events_contain_done_last(&resolve_events);

        let actual = plan(Direction::Get, manifests);
        let expected = Plan {
            roots: vec![Root::Directory {
                dst: path_dir,
                directories: Vec::new(),
                leaves: vec![Leaf {
                    index: ([0], [0, 0]).into(),
                    src: manifest_directory.join("valid-leaf"),
                    dst: "valid-leaf".into(),
                }],
            }],
            errors: vec![Error::NonRootPathShedBothAbsolute {
                index: ([0], [0, 1]).into(),
            }],
        };

        assert_eq!(actual, expected);
    }
}
