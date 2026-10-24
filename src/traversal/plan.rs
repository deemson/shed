use super::{
    error::Error,
    model::{Direction, Node, Plan},
    radix,
};
use crate::manifest::Manifest;
use std::{
    collections::HashSet,
    path::{Component, Path, PathBuf},
};

pub fn plan(direction: Direction, manifests: &[Manifest]) -> Plan {
    let radix_plan = radix::Planner::new(direction).plan(manifests);
    let (mut entries, errors) = collect_non_overlapping_entries(radix_plan);
    normalize_dst_roots(&mut entries);

    Plan {
        roots: build_roots(entries),
        errors,
    }
}

fn collect_non_overlapping_entries(
    plan: radix::Plan,
) -> (Vec<(PathBuf, radix::Entry)>, Vec<Error>) {
    let entries = plan
        .radix
        .range::<Path, _>(..)
        .map(|(components, entries)| (components.into_iter().collect(), entries.clone()))
        .collect::<Vec<(PathBuf, Vec<radix::Entry>)>>();
    let mut omitted = vec![false; entries.len()];
    let mut valid_entries = Vec::new();
    let mut errors = plan.errors;

    for (position, (dst, entries_at_dst)) in entries.iter().enumerate() {
        if omitted[position] {
            continue;
        }

        let descendants = entries
            .iter()
            .enumerate()
            .skip(position + 1)
            .filter_map(|(position, (descendant_dst, _))| {
                descendant_dst.starts_with(dst).then_some(position)
            })
            .collect::<Vec<_>>();

        if entries_at_dst.len() > 1 || !descendants.is_empty() {
            let mut indexes = HashSet::new();
            for entry in entries_at_dst {
                indexes.insert(entry.index.clone());
            }
            omitted[position] = true;

            for descendant in descendants {
                omitted[descendant] = true;
                for entry in &entries[descendant].1 {
                    indexes.insert(entry.index.clone());
                }
            }

            errors.push(Error::OverlappingLeaves { indexes });
        } else {
            valid_entries.push((dst.clone(), entries_at_dst[0].clone()));
        }
    }

    (valid_entries, errors)
}

fn normalize_dst_roots(entries: &mut [(PathBuf, radix::Entry)]) {
    let declared_roots = entries
        .iter()
        .filter_map(|(_, entry)| entry.dst_root.clone())
        .collect::<Vec<_>>();

    for (_, entry) in entries {
        let Some(root) = &entry.dst_root else {
            continue;
        };

        let normalized_root = declared_roots
            .iter()
            .filter(|candidate| root.starts_with(candidate))
            .min_by_key(|candidate| candidate.components().count())
            .expect("a declared destination root is among the declared roots")
            .clone();
        entry.dst_root = Some(normalized_root);
    }
}

fn build_roots(entries: Vec<(PathBuf, radix::Entry)>) -> Vec<Node> {
    let mut roots = Vec::new();

    for (dst, entry) in entries {
        let radix::Entry {
            index,
            src,
            dst_root,
        } = entry;
        let leaf = Node::Leaf { index, src, dst };

        let Some(dst_root) = dst_root else {
            roots.push(leaf);
            continue;
        };

        let nodes = match roots.iter_mut().find_map(|root| match root {
            Node::Directory { dst, nodes } if *dst == dst_root => Some(nodes),
            Node::Directory { .. } | Node::Leaf { .. } => None,
        }) {
            Some(nodes) => nodes,
            None => {
                roots.push(Node::Directory {
                    dst: dst_root.clone(),
                    nodes: Vec::new(),
                });
                match roots.last_mut() {
                    Some(Node::Directory { nodes, .. }) => nodes,
                    Some(Node::Leaf { .. }) | None => unreachable!("a directory was just added"),
                }
            }
        };

        insert_leaf(nodes, &dst_root, leaf);
    }

    roots
}

fn insert_leaf(nodes: &mut Vec<Node>, root: &Path, leaf: Node) {
    let Node::Leaf { dst, .. } = &leaf else {
        unreachable!("only leaves are inserted into directory nodes");
    };

    let relative_dst = dst.strip_prefix(root).expect(
        "a leaf destination with a declared root must be under its normalized destination root",
    );
    let relative_parent = relative_dst.parent().unwrap_or_else(|| Path::new(""));
    let mut parent = root.to_path_buf();
    let mut children = nodes;

    for component in relative_parent.components() {
        let Component::Normal(component) = component else {
            panic!("a leaf destination relative to its root contains only normal components");
        };
        parent.push(component);

        let position = children.iter().position(|node| match node {
            Node::Directory { dst, .. } => *dst == parent,
            Node::Leaf { .. } => false,
        });
        let directory = match position {
            Some(position) => &mut children[position],
            None => {
                let position = children
                    .iter()
                    .position(|node| matches!(node, Node::Leaf { .. }))
                    .unwrap_or(children.len());
                children.insert(
                    position,
                    Node::Directory {
                        dst: parent.clone(),
                        nodes: Vec::new(),
                    },
                );
                &mut children[position]
            }
        };
        children = match directory {
            Node::Directory { nodes, .. } => nodes,
            Node::Leaf { .. } => unreachable!("only directories are traversed"),
        };
    }

    children.push(leaf);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{Input, testing as testing_m};
    use indoc::formatdoc;
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    #[tokio::test]
    async fn groups_siblings_with_full_destinations_and_directories_before_leaves() {
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
                - a
                - z/b
                - z/c
            "},
        );
        let manifest_directory = manifest_path.parent().unwrap().to_path_buf();

        let (manifests, resolve_events) = testing_m::resolve(vec![Input {
            name: "manifest".into(),
            path: manifest_path,
        }])
        .await;

        testing_m::assert_events_contain_no_errors(&resolve_events);
        testing_m::assert_events_contain_done_last(&resolve_events);

        let actual = plan(Direction::Get, &manifests);
        let expected = Plan {
            roots: vec![Node::Directory {
                dst: path_dir.clone(),
                nodes: vec![
                    Node::Directory {
                        dst: path_dir.join("z"),
                        nodes: vec![
                            Node::Leaf {
                                index: ([0], [1]).into(),
                                src: manifest_directory.join("z/b"),
                                dst: path_dir.join("z/b"),
                            },
                            Node::Leaf {
                                index: ([0], [2]).into(),
                                src: manifest_directory.join("z/c"),
                                dst: path_dir.join("z/c"),
                            },
                        ],
                    },
                    Node::Leaf {
                        index: ([0], [0]).into(),
                        src: manifest_directory.join("a"),
                        dst: path_dir.join("a"),
                    },
                ],
            }],
            errors: Vec::new(),
        };
        assert_eq!(actual, expected);
    }

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
        let manifest_directory = manifest_path.parent().unwrap().to_path_buf();

        let (manifests, resolve_events) = testing_m::resolve(vec![Input {
            name: "manifest".into(),
            path: manifest_path,
        }])
        .await;

        testing_m::assert_events_contain_no_errors(&resolve_events);
        testing_m::assert_events_contain_done_last(&resolve_events);

        let actual = plan(Direction::Get, &manifests);
        let expected = Plan {
            roots: vec![
                Node::Leaf {
                    index: ([0], [0]).into(),
                    src: manifest_directory.join("leaf1"),
                    dst: path_dir.join("leaf1"),
                },
                Node::Leaf {
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
    async fn plans_bare_leaves_under_a_manifest_level_path_for_put() {
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
        let manifest_directory = manifest_path.parent().unwrap().to_path_buf();

        let (manifests, resolve_events) = testing_m::resolve(vec![Input {
            name: "manifest".into(),
            path: manifest_path,
        }])
        .await;

        testing_m::assert_events_contain_no_errors(&resolve_events);
        testing_m::assert_events_contain_done_last(&resolve_events);

        let actual = plan(Direction::Put, &manifests);
        let expected = Plan {
            roots: vec![Node::Directory {
                dst: manifest_directory.clone(),
                nodes: vec![
                    Node::Leaf {
                        index: ([0], [0]).into(),
                        src: path_dir.join("leaf1"),
                        dst: manifest_directory.join("leaf1"),
                    },
                    Node::Leaf {
                        index: ([0], [1]).into(),
                        src: path_dir.join("leaf2"),
                        dst: manifest_directory.join("leaf2"),
                    },
                ],
            }],
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
                  shed: .
                  items:
                    - path: dir1
                      items:
                        - path: dir2
                          items:
                            - leaf1
                            - leaf2
            "},
        );
        let manifest_directory = manifest_path.parent().unwrap().to_path_buf();

        let (manifests, resolve_events) = testing_m::resolve(vec![Input {
            name: "manifest".into(),
            path: manifest_path,
        }])
        .await;

        testing_m::assert_events_contain_no_errors(&resolve_events);
        testing_m::assert_events_contain_done_last(&resolve_events);

        let actual = plan(Direction::Get, &manifests);
        let expected = Plan {
            roots: vec![Node::Directory {
                dst: path_dir.clone(),
                nodes: vec![Node::Directory {
                    dst: path_dir.join("dir1"),
                    nodes: vec![Node::Directory {
                        dst: path_dir.join("dir1/dir2"),
                        nodes: vec![
                            Node::Leaf {
                                index: ([0], [0, 0, 0, 0]).into(),
                                src: manifest_directory.join("dir1/dir2/leaf1"),
                                dst: path_dir.join("dir1/dir2/leaf1"),
                            },
                            Node::Leaf {
                                index: ([0], [0, 0, 0, 1]).into(),
                                src: manifest_directory.join("dir1/dir2/leaf2"),
                                dst: path_dir.join("dir1/dir2/leaf2"),
                            },
                        ],
                    }],
                }],
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
        let manifest_directory = manifest_path.parent().unwrap().to_path_buf();

        let (manifests, resolve_events) = testing_m::resolve(vec![Input {
            name: "manifest".into(),
            path: manifest_path,
        }])
        .await;

        testing_m::assert_events_contain_no_errors(&resolve_events);
        testing_m::assert_events_contain_done_last(&resolve_events);

        let actual = plan(Direction::Get, &manifests);
        let expected = Plan {
            roots: vec![Node::Directory {
                dst: path_dir.clone(),
                nodes: vec![
                    Node::Leaf {
                        index: ([0], [0]).into(),
                        src: manifest_directory.join("leaf1"),
                        dst: path_dir.join("leaf1"),
                    },
                    Node::Leaf {
                        index: ([0], [1]).into(),
                        src: manifest_directory.join("leaf2"),
                        dst: path_dir.join("leaf2"),
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
        let manifest_directory = main_manifest_path.parent().unwrap().to_path_buf();

        let (manifests, resolve_events) = testing_m::resolve(vec![Input {
            name: "manifest".into(),
            path: main_manifest_path,
        }])
        .await;

        testing_m::assert_events_contain_no_errors(&resolve_events);
        testing_m::assert_events_contain_done_last(&resolve_events);

        let actual = plan(Direction::Get, &manifests);
        let expected = Plan {
            roots: vec![Node::Directory {
                dst: path_dir.clone(),
                nodes: vec![
                    Node::Directory {
                        dst: path_dir.join("sub-dir1"),
                        nodes: vec![Node::Directory {
                            dst: path_dir.join("sub-dir1/sub-dir2"),
                            nodes: vec![Node::Leaf {
                                index: ([0, 0], [0]).into(),
                                src: manifest_directory.join("sub-dir-leaf"),
                                dst: path_dir.join("sub-dir1/sub-dir2/sub-dir-leaf"),
                            }],
                        }],
                    },
                    Node::Leaf {
                        index: ([0, 1], [0]).into(),
                        src: manifest_directory.join("top-leaf"),
                        dst: path_dir.join("top-leaf"),
                    },
                ],
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
        let manifest_directory = main_manifest_path.parent().unwrap().to_path_buf();

        let (manifests, resolve_events) = testing_m::resolve(vec![Input {
            name: "main-manifest".into(),
            path: main_manifest_path,
        }])
        .await;

        testing_m::assert_events_contain_no_errors(&resolve_events);
        testing_m::assert_events_contain_done_last(&resolve_events);

        let actual = plan(Direction::Get, &manifests);
        let expected = Plan {
            roots: vec![
                Node::Directory {
                    dst: path_dir.join("sub-dir1"),
                    nodes: vec![Node::Leaf {
                        index: ([0, 0], [0]).into(),
                        src: manifest_directory.join("sub-dir1-leaf"),
                        dst: path_dir.join("sub-dir1/sub-dir1-leaf"),
                    }],
                },
                Node::Directory {
                    dst: path_dir.join("sub-dir2"),
                    nodes: vec![Node::Leaf {
                        index: ([0, 1], [0]).into(),
                        src: manifest_directory.join("sub-dir2-leaf"),
                        dst: path_dir.join("sub-dir2/sub-dir2-leaf"),
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
        let manifest_directory = manifest_path.parent().unwrap().to_path_buf();

        let (manifests, resolve_events) = testing_m::resolve(vec![Input {
            name: "manifest".into(),
            path: manifest_path,
        }])
        .await;

        testing_m::assert_events_contain_no_errors(&resolve_events);
        testing_m::assert_events_contain_done_last(&resolve_events);

        let actual = plan(Direction::Get, &manifests);
        let expected = Plan {
            roots: vec![Node::Leaf {
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
        let manifest_directory = manifest_path.parent().unwrap().to_path_buf();

        let (manifests, resolve_events) = testing_m::resolve(vec![Input {
            name: "manifest".into(),
            path: manifest_path,
        }])
        .await;

        testing_m::assert_events_contain_no_errors(&resolve_events);
        testing_m::assert_events_contain_done_last(&resolve_events);

        let actual = plan(Direction::Get, &manifests);
        let expected = Plan {
            roots: vec![Node::Leaf {
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
                  shed: .
                  items:
                    - valid-leaf
                    - path: {path_dir_display}/invalid-leaf
                      shed: {shed_dir_display}/invalid-leaf
            "},
        );
        let manifest_directory = manifest_path.parent().unwrap().to_path_buf();

        let (manifests, resolve_events) = testing_m::resolve(vec![Input {
            name: "manifest".into(),
            path: manifest_path,
        }])
        .await;

        testing_m::assert_events_contain_no_errors(&resolve_events);
        testing_m::assert_events_contain_done_last(&resolve_events);

        let actual = plan(Direction::Get, &manifests);
        let expected = Plan {
            roots: vec![Node::Directory {
                dst: path_dir.clone(),
                nodes: vec![Node::Leaf {
                    index: ([0], [0, 0]).into(),
                    src: manifest_directory.join("valid-leaf"),
                    dst: path_dir.join("valid-leaf"),
                }],
            }],
            errors: vec![Error::NonRootPathShedBothAbsolute {
                index: ([0], [0, 1]).into(),
            }],
        };
        assert_eq!(actual, expected);
    }

    #[tokio::test]
    async fn plans_normalized_manifest_shed_and_item_paths_for_put() {
        let temp_dir = TempDir::new().unwrap();
        let shed_dir = temp_dir.path().join("shed");

        let manifest_path = testing_m::write_yaml_manifest(
            &shed_dir,
            "manifest",
            formatdoc! {"
              path: $HOME/.config/app
              shed: ../app
              items:
                - config
                - dir/../schema
            "},
        );
        let manifest_directory = manifest_path.parent().unwrap().to_path_buf();
        let app_dir = manifest_directory.parent().unwrap().join("app");

        let (manifests, resolve_events) = testing_m::resolve(vec![Input {
            name: "manifest".into(),
            path: manifest_path,
        }])
        .await;

        testing_m::assert_events_contain_no_errors(&resolve_events);
        testing_m::assert_events_contain_done_last(&resolve_events);

        let home = PathBuf::from(std::env::var("HOME").unwrap());
        let actual = plan(Direction::Put, &manifests);
        let expected = Plan {
            roots: vec![Node::Directory {
                dst: app_dir.clone(),
                nodes: vec![
                    Node::Leaf {
                        index: ([0], [0]).into(),
                        src: home.join(".config/app/config"),
                        dst: app_dir.join("config"),
                    },
                    Node::Leaf {
                        index: ([0], [1]).into(),
                        src: home.join(".config/app/schema"),
                        dst: app_dir.join("schema"),
                    },
                ],
            }],
            errors: Vec::new(),
        };
        assert_eq!(actual, expected);
    }

    #[tokio::test]
    async fn omits_leaves_escaping_the_manifest_destination_root_and_plans_valid_siblings_for_get() {
        let temp_dir = TempDir::new().unwrap();
        let shed_dir = temp_dir.path().join("shed");

        let manifest_path = testing_m::write_yaml_manifest(
            &shed_dir,
            "manifest",
            formatdoc! {"
              path: $HOME/.config/app
              shed: ../app
              items:
                - ../escaping-leaf
                - normal leaf
            "},
        );
        let manifest_directory = manifest_path.parent().unwrap().to_path_buf();
        let app_dir = manifest_directory.parent().unwrap().join("app");

        let (manifests, resolve_events) = testing_m::resolve(vec![Input {
            name: "manifest".into(),
            path: manifest_path,
        }])
        .await;

        testing_m::assert_events_contain_no_errors(&resolve_events);
        testing_m::assert_events_contain_done_last(&resolve_events);

        let home = PathBuf::from(std::env::var("HOME").unwrap());
        let dst_root = home.join(".config/app");

        let actual = plan(Direction::Get, &manifests);
        let expected = Plan {
            roots: vec![Node::Directory {
                dst: dst_root.clone(),
                nodes: vec![Node::Leaf {
                    index: ([0], [1]).into(),
                    src: app_dir.join("normal leaf"),
                    dst: dst_root.join("normal leaf"),
                }],
            }],
            errors: vec![Error::DestinationOutsideRoot {
                index: ([0], [0]).into(),
                root: dst_root,
                dst: home.join(".config/escaping-leaf"),
            }],
        };
        assert_eq!(actual, expected);
    }

    #[tokio::test]
    async fn omits_items_with_absolute_path_and_no_shed_and_plans_unrelated_items_for_put() {
        let temp_dir = TempDir::new().unwrap();
        let path_dir = temp_dir.path().join("path");
        let shed_dir = temp_dir.path().join("shed");

        let path_dir_display = path_dir.display();
        let manifest_path = testing_m::write_yaml_manifest(
            &shed_dir,
            "manifest",
            formatdoc! {"
              items:
                - {path_dir_display}/invalid-leaf
                - path: {path_dir_display}/invalid-parent
                  items:
                    - child
                - path: {path_dir_display}/valid-leaf
                  shed: valid-leaf
            "},
        );
        let manifest_directory = manifest_path.parent().unwrap().to_path_buf();

        let (manifests, resolve_events) = testing_m::resolve(vec![Input {
            name: "manifest".into(),
            path: manifest_path,
        }])
        .await;

        testing_m::assert_events_contain_no_errors(&resolve_events);
        testing_m::assert_events_contain_done_last(&resolve_events);

        let actual = plan(Direction::Put, &manifests);
        let expected = Plan {
            roots: vec![Node::Leaf {
                index: ([0], [2]).into(),
                src: path_dir.join("valid-leaf"),
                dst: manifest_directory.join("valid-leaf"),
            }],
            errors: vec![
                Error::AbsolutePathWithoutShed {
                    index: ([0], [0]).into(),
                },
                Error::AbsolutePathWithoutShed {
                    index: ([0], [1]).into(),
                },
            ],
        };
        assert_eq!(actual, expected);
    }
}
