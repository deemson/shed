use super::{
    error::Error,
    model::{Direction, Directory, Leaf, Plan, Root},
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
                indexes.insert(entry.leaf.index.clone());
            }
            omitted[position] = true;

            for descendant in descendants {
                omitted[descendant] = true;
                for entry in &entries[descendant].1 {
                    indexes.insert(entry.leaf.index.clone());
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

fn build_roots(entries: Vec<(PathBuf, radix::Entry)>) -> Vec<Root> {
    let mut roots = Vec::new();

    for (dst, entry) in entries {
        let radix::Entry { dst_root, leaf } = entry;

        let Some(dst_root) = dst_root else {
            roots.push(Root::Leaf {
                index: leaf.index,
                src: leaf.src,
                dst,
            });
            continue;
        };

        let relative_dst = dst.strip_prefix(&dst_root).expect(
            "a leaf destination with a declared root must be under its normalized destination root",
        );
        let relative_parent = relative_dst.parent().unwrap_or_else(|| Path::new(""));

        let Some((directories, leaves)) = roots.iter_mut().find_map(|root| match root {
            Root::Directory {
                dst,
                directories,
                leaves,
            } if *dst == dst_root => Some((directories, leaves)),
            _ => None,
        }) else {
            let mut directories = Vec::new();
            let mut leaves = Vec::new();
            insert_leaf(&mut directories, &mut leaves, relative_parent, leaf);
            roots.push(Root::Directory {
                dst: dst_root,
                directories,
                leaves,
            });
            continue;
        };

        insert_leaf(directories, leaves, relative_parent, leaf);
    }

    roots
}

fn insert_leaf(
    directories: &mut Vec<Directory>,
    leaves: &mut Vec<Leaf>,
    relative_parent: &Path,
    leaf: Leaf,
) {
    let mut components = relative_parent.components();
    let Some(component) = components.next() else {
        leaves.push(leaf);
        return;
    };
    let Component::Normal(dst) = component else {
        panic!("a leaf destination relative to its root contains only normal components");
    };

    let position = directories
        .iter()
        .position(|directory| directory.dst == dst);
    let directory = match position {
        Some(position) => &mut directories[position],
        None => {
            directories.push(Directory {
                dst: dst.to_owned(),
                directories: Vec::new(),
                leaves: Vec::new(),
            });
            directories
                .last_mut()
                .expect("a directory was just added to the directory list")
        }
    };

    insert_leaf(
        &mut directory.directories,
        &mut directory.leaves,
        components.as_path(),
        leaf,
    );
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

        let actual = plan(Direction::Get, &manifests);
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

        let actual = plan(Direction::Put, &manifests);
        let expected = Plan {
            roots: vec![Root::Directory {
                dst: manifest_directory,
                directories: Vec::new(),
                leaves: vec![
                    Leaf {
                        index: ([0], [0]).into(),
                        src: path_dir.join("leaf1"),
                        dst: "leaf1".into(),
                    },
                    Leaf {
                        index: ([0], [1]).into(),
                        src: path_dir.join("leaf2"),
                        dst: "leaf2".into(),
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

        let actual = plan(Direction::Get, &manifests);
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

        let actual = plan(Direction::Get, &manifests);
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

        let actual = plan(Direction::Get, &manifests);
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

        let actual = plan(Direction::Get, &manifests);
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

        let actual = plan(Direction::Get, &manifests);
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

        let actual = plan(Direction::Get, &manifests);
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

        let actual = plan(Direction::Get, &manifests);
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
