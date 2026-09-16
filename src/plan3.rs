use std::path::{Path, PathBuf};

use crate::manifest::{ChildItem, ManifestFile, RootItem};

#[derive(Debug, PartialEq)]
pub struct Dir {
    pub dst: PathBuf,
    pub items: Vec<Item>,
}

#[derive(Debug, PartialEq)]
pub struct Leaf {
    pub src: PathBuf,
    pub dst: PathBuf,
}

#[derive(Debug, PartialEq)]
pub enum Item {
    Dir(Dir),
    Leaf(Leaf),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Direction {
    Put,
    Get,
}

/// A parsed manifest and the directory containing it.
pub struct Manifest {
    location: PathBuf,
    file: ManifestFile,
}

impl Manifest {
    pub fn new(location: impl Into<PathBuf>, file: ManifestFile) -> Self {
        Self {
            location: location.into(),
            file,
        }
    }
}

pub struct Planner {
    direction: Direction,
}

impl Planner {
    pub fn new(direction: Direction) -> Self {
        Self { direction }
    }

    /// Resolves all endpoints, then groups leaves with the same destination parent.
    pub fn plan(&self, manifests: impl IntoIterator<Item = Manifest>) -> Vec<Item> {
        let mut leaves = Vec::new();

        for manifest in manifests {
            for item in manifest.file.items {
                self.resolve_root(&manifest.location, item, &mut leaves);
            }
        }

        group_leaves(leaves)
    }

    fn resolve_root(&self, location: &Path, item: RootItem, leaves: &mut Vec<Leaf>) {
        let system = PathBuf::from(item.path);
        let shed = location.join(item.shed);
        self.resolve_item(system, shed, item.items, leaves);
    }

    fn resolve_child(
        &self,
        system_parent: &Path,
        shed_parent: &Path,
        item: ChildItem,
        leaves: &mut Vec<Leaf>,
    ) {
        let (path, shed, children) = match item {
            ChildItem::Path(path) => (path.clone(), path, None),
            ChildItem::Item(item) => {
                let shed = item.shed.unwrap_or_else(|| item.path.clone());
                (item.path, shed, item.items)
            }
        };

        self.resolve_item(
            system_parent.join(path),
            shed_parent.join(shed),
            children,
            leaves,
        );
    }

    fn resolve_item(
        &self,
        system: PathBuf,
        shed: PathBuf,
        children: Option<Vec<ChildItem>>,
        leaves: &mut Vec<Leaf>,
    ) {
        match children {
            Some(children) => {
                for child in children {
                    self.resolve_child(&system, &shed, child, leaves);
                }
            }
            None => {
                let (src, dst) = match self.direction {
                    Direction::Put => (system, shed),
                    Direction::Get => (shed, system),
                };
                leaves.push(Leaf { src, dst });
            }
        }
    }
}

fn group_leaves(leaves: Vec<Leaf>) -> Vec<Item> {
    let mut groups: Vec<(Option<PathBuf>, Vec<Leaf>)> = Vec::new();

    for leaf in leaves {
        let parent = leaf
            .dst
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .map(Path::to_path_buf);

        if let Some((_, group)) = groups
            .iter_mut()
            .find(|(group_parent, _)| parent.is_some() && group_parent == &parent)
        {
            group.push(leaf);
        } else {
            groups.push((parent, vec![leaf]));
        }
    }

    groups
        .into_iter()
        .map(|(parent, mut leaves)| {
            if leaves.len() == 1 {
                Item::Leaf(leaves.pop().expect("the group contains one leaf"))
            } else {
                Item::Dir(Dir {
                    dst: parent.expect("a multi-leaf group has a destination parent"),
                    items: leaves.into_iter().map(Item::Leaf).collect(),
                })
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_destinations_across_manifests() {
        #[rustfmt::skip]
        let ms1 = [
            "items:",
            "  - path: /dir/dst-item1",
            "    shed: src-item1",
            "  - path: /dir/dst-item2",
            "    shed: src-item2",
        ].join("\n");
        let manifest1 = Manifest::new("/manifest1", ManifestFile::try_from(ms1).unwrap());

        #[rustfmt::skip]
        let ms2 = [
            "items:",
            "  - path: /dir/dst-item3",
            "    shed: src-item3",
            "  - path: /dir/dst-item4",
            "    shed: src-item4",
        ].join("\n");
        let manifest2 = Manifest::new("/manifest2", ManifestFile::try_from(ms2).unwrap());

        let actual = Planner::new(Direction::Get).plan([manifest1, manifest2]);
        let expected = vec![Item::Dir(Dir {
            dst: PathBuf::from("/dir"),
            items: vec![
                Item::Leaf(Leaf {
                    src: PathBuf::from("/manifest1/src-item1"),
                    dst: PathBuf::from("/dir/dst-item1"),
                }),
                Item::Leaf(Leaf {
                    src: PathBuf::from("/manifest1/src-item2"),
                    dst: PathBuf::from("/dir/dst-item2"),
                }),
                Item::Leaf(Leaf {
                    src: PathBuf::from("/manifest2/src-item3"),
                    dst: PathBuf::from("/dir/dst-item3"),
                }),
                Item::Leaf(Leaf {
                    src: PathBuf::from("/manifest2/src-item4"),
                    dst: PathBuf::from("/dir/dst-item4"),
                }),
            ],
        })];

        assert_eq!(actual, expected);
    }

    #[test]
    fn keeps_a_single_destination_as_a_leaf() {
        let file =
            ManifestFile::try_from("items:\n  - path: /dst/file\n    shed: src/file").unwrap();

        let actual = Planner::new(Direction::Get).plan([Manifest::new("/manifest", file)]);
        let expected = vec![Item::Leaf(Leaf {
            src: PathBuf::from("/manifest/src/file"),
            dst: PathBuf::from("/dst/file"),
        })];

        assert_eq!(actual, expected);
    }

    #[test]
    fn creates_separate_groups_for_unrelated_destinations() {
        #[rustfmt::skip]
        let source = [
            "items:",
            "  - path: /first/a",
            "    shed: a",
            "  - path: /first/b",
            "    shed: b",
            "  - path: /second/c",
            "    shed: c",
            "  - path: /second/d",
            "    shed: d",
        ].join("\n");
        let file = ManifestFile::try_from(source).unwrap();

        let actual = Planner::new(Direction::Get).plan([Manifest::new("/manifest", file)]);

        assert_eq!(
            actual,
            vec![
                Item::Dir(Dir {
                    dst: PathBuf::from("/first"),
                    items: vec![
                        Item::Leaf(Leaf {
                            src: PathBuf::from("/manifest/a"),
                            dst: PathBuf::from("/first/a"),
                        }),
                        Item::Leaf(Leaf {
                            src: PathBuf::from("/manifest/b"),
                            dst: PathBuf::from("/first/b"),
                        }),
                    ],
                }),
                Item::Dir(Dir {
                    dst: PathBuf::from("/second"),
                    items: vec![
                        Item::Leaf(Leaf {
                            src: PathBuf::from("/manifest/c"),
                            dst: PathBuf::from("/second/c"),
                        }),
                        Item::Leaf(Leaf {
                            src: PathBuf::from("/manifest/d"),
                            dst: PathBuf::from("/second/d"),
                        }),
                    ],
                }),
            ]
        );
    }
}
