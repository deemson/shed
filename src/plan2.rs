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

    /// Converts manifests, already loaded in include order, into planned items.
    pub fn plan(&self, manifests: impl IntoIterator<Item = Manifest>) -> Vec<Item> {
        manifests
            .into_iter()
            .flat_map(|manifest| {
                let location = manifest.location;
                manifest
                    .file
                    .items
                    .into_iter()
                    .map(move |item| self.plan_root(&location, item))
            })
            .collect()
    }

    fn plan_root(&self, location: &Path, item: RootItem) -> Item {
        let system = PathBuf::from(item.path);
        let shed = location.join(item.shed);
        self.plan_item(system, shed, item.items)
    }

    fn plan_child(&self, system_parent: &Path, shed_parent: &Path, item: ChildItem) -> Item {
        let (path, shed, children) = match item {
            ChildItem::Path(path) => (path.clone(), path, None),
            ChildItem::Item(item) => {
                let shed = item.shed.unwrap_or_else(|| item.path.clone());
                (item.path, shed, item.items)
            }
        };

        self.plan_item(system_parent.join(path), shed_parent.join(shed), children)
    }

    fn plan_item(&self, system: PathBuf, shed: PathBuf, children: Option<Vec<ChildItem>>) -> Item {
        match children {
            Some(children) => {
                let dst = self.destination(&system, &shed).to_path_buf();
                let items = children
                    .into_iter()
                    .map(|item| self.plan_child(&system, &shed, item))
                    .collect();
                Item::Dir(Dir { dst, items })
            }
            None => {
                let (src, dst) = self.endpoints(system, shed);
                Item::Leaf(Leaf { src, dst })
            }
        }
    }

    fn endpoints(&self, system: PathBuf, shed: PathBuf) -> (PathBuf, PathBuf) {
        match self.direction {
            Direction::Put => (system, shed),
            Direction::Get => (shed, system),
        }
    }

    fn destination<'a>(&self, system: &'a Path, shed: &'a Path) -> &'a Path {
        match self.direction {
            Direction::Put => shed,
            Direction::Get => system,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn manifest() -> Manifest {
        #[rustfmt::skip]
        let source = [
            "items:",
            "  - path: /system/editor",
            "    shed: config/editor",
            "    items:",
            "      - init.lua",
            "      - path: local-settings.json",
            "        shed: settings.json",
            "  - path: /system/shellrc",
            "    shed: shell/shellrc",
        ].join("\n");

        Manifest::new("/repo/manifests", ManifestFile::try_from(source).unwrap())
    }

    #[test]
    fn plans_put_from_system_to_shed() {
        let actual = Planner::new(Direction::Put).plan([manifest()]);
        let expected = vec![
            Item::Dir(Dir {
                dst: PathBuf::from("/repo/manifests/config/editor"),
                items: vec![
                    Item::Leaf(Leaf {
                        src: PathBuf::from("/system/editor/init.lua"),
                        dst: PathBuf::from("/repo/manifests/config/editor/init.lua"),
                    }),
                    Item::Leaf(Leaf {
                        src: PathBuf::from("/system/editor/local-settings.json"),
                        dst: PathBuf::from("/repo/manifests/config/editor/settings.json"),
                    }),
                ],
            }),
            Item::Leaf(Leaf {
                src: PathBuf::from("/system/shellrc"),
                dst: PathBuf::from("/repo/manifests/shell/shellrc"),
            }),
        ];

        assert_eq!(actual, expected);
    }

    #[test]
    fn plans_get_from_shed_to_system() {
        let actual = Planner::new(Direction::Get).plan([manifest()]);
        let expected = vec![
            Item::Dir(Dir {
                dst: PathBuf::from("/system/editor"),
                items: vec![
                    Item::Leaf(Leaf {
                        src: PathBuf::from("/repo/manifests/config/editor/init.lua"),
                        dst: PathBuf::from("/system/editor/init.lua"),
                    }),
                    Item::Leaf(Leaf {
                        src: PathBuf::from("/repo/manifests/config/editor/settings.json"),
                        dst: PathBuf::from("/system/editor/local-settings.json"),
                    }),
                ],
            }),
            Item::Leaf(Leaf {
                src: PathBuf::from("/repo/manifests/shell/shellrc"),
                dst: PathBuf::from("/system/shellrc"),
            }),
        ];

        assert_eq!(actual, expected);
    }
}
