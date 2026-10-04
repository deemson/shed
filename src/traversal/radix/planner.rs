use super::super::{
    error::Error,
    model::{Direction, Leaf},
};
use super::model::{Entry, Plan};
use crate::manifest::{FullIndex, Index, Item, ItemKind, Manifest};
use std::path::{Path, PathBuf};

pub(in crate::traversal) struct Planner {
    direction: Direction,
}

impl Planner {
    pub fn new(direction: Direction) -> Self {
        Self { direction }
    }

    pub fn plan(&self, manifests: &[Manifest]) -> Plan {
        let mut plan = Plan::default();
        self.plan_manifests(&mut plan, Vec::new(), manifests);
        plan
    }

    fn plan_manifests(&self, plan: &mut Plan, index_parent: Index, manifests: &[Manifest]) {
        for (position, manifest) in manifests.iter().enumerate() {
            let mut index = index_parent.clone();
            index.push(position);

            self.plan_manifest(plan, index, manifest);
        }
    }

    fn plan_manifest(&self, plan: &mut Plan, index: Index, manifest: &Manifest) {
        let directory = manifest
            .location
            .parent()
            .expect("a resolved manifest location has a parent");

        let path = manifest
            .path
            .as_deref()
            .map(PathBuf::from)
            .unwrap_or_default();

        let shed = manifest
            .shed
            .as_deref()
            .map(|shed| directory.join(shed))
            .unwrap_or_else(|| directory.to_path_buf());

        self.plan_manifests(plan, index.clone(), &manifest.manifests);

        let dst_root = if manifest.path.is_some() || manifest.shed.is_some() {
            let (_, dst) = self.direction.resolve(path.clone(), shed.clone());
            Some(dst)
        } else {
            None
        };

        self.plan_item_kinds(
            plan,
            FullIndex {
                manifest: index,
                item: Vec::new(),
            },
            &path,
            &shed,
            dst_root.as_deref(),
            &manifest.items,
        );
    }

    fn plan_item_kinds(
        &self,
        plan: &mut Plan,
        index_parent: FullIndex,
        path_parent: &Path,
        shed_parent: &Path,
        dst_root: Option<&Path>,
        item_kinds: &[ItemKind],
    ) {
        for (position, item_kind) in item_kinds.iter().enumerate() {
            let mut item_index = index_parent.item.clone();
            item_index.push(position);
            let index = FullIndex {
                manifest: index_parent.manifest.clone(),
                item: item_index,
            };

            match item_kind {
                ItemKind::Item(item) => {
                    self.plan_item(plan, index, path_parent, shed_parent, dst_root, item);
                }
                ItemKind::Path(path) => {
                    let bare_item = Item {
                        path: path.clone(),
                        shed: None,
                        items: None,
                    };

                    self.plan_item(plan, index, path_parent, shed_parent, dst_root, &bare_item);
                }
            }
        }
    }

    fn plan_item(
        &self,
        plan: &mut Plan,
        index: FullIndex,
        path_parent: &Path,
        shed_parent: &Path,
        dst_root: Option<&Path>,
        item: &Item,
    ) {
        let path_is_absolute = Path::new(&item.path).is_absolute();
        let shed_is_absolute = item
            .shed
            .as_deref()
            .is_some_and(|shed| Path::new(shed).is_absolute());

        if index.item.len() > 1 && path_is_absolute && shed_is_absolute {
            plan.errors
                .push(Error::NonRootPathShedBothAbsolute { index });
            return;
        }

        let path = path_parent.join(&item.path);
        let shed = match &item.shed {
            Some(shed) => shed_parent.join(shed),
            None if Path::new(&item.path).is_absolute() => shed_parent.to_path_buf(),
            None => shed_parent.join(&item.path),
        };

        if let Some(children) = &item.items {
            let child_dst_root = match dst_root {
                Some(dst_root) => dst_root.to_path_buf(),
                None => {
                    let (_, dst) = self.direction.resolve(path.clone(), shed.clone());
                    dst
                }
            };

            self.plan_item_kinds(plan, index, &path, &shed, Some(&child_dst_root), children);
        } else {
            let (src, dst) = self.direction.resolve(path, shed);

            let dst_name = dst
                .file_name()
                .expect("a leaf destination has a file name")
                .to_owned();

            let leaf = Leaf {
                index,
                src,
                dst: dst_name,
            };

            let mut entries = plan.radix.get(&dst).cloned().unwrap_or_default();
            entries.push(Entry {
                dst_root: dst_root.map(Path::to_path_buf),
                leaf,
            });
            plan.radix.insert(&dst, entries);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_a_declared_manifest_destination_root() {
        let manifest = Manifest {
            name: "manifest".into(),
            location: "/shed/manifest.yaml".into(),
            path: Some("/config".into()),
            shed: None,
            manifests: Vec::new(),
            items: vec![ItemKind::Path("a".into()), ItemKind::Path("b".into())],
        };

        let Plan { radix, errors } = Planner::new(Direction::Get).plan(&[manifest]);
        let actual = radix
            .range::<Path, _>(..)
            .map(|(components, entries)| {
                (components.into_iter().collect::<PathBuf>(), entries.clone())
            })
            .collect::<Vec<_>>();
        let expected = vec![
            (
                "/config/a".into(),
                vec![Entry {
                    dst_root: Some("/config".into()),
                    leaf: Leaf {
                        index: ([0], [0]).into(),
                        src: "/shed/a".into(),
                        dst: "a".into(),
                    },
                }],
            ),
            (
                "/config/b".into(),
                vec![Entry {
                    dst_root: Some("/config".into()),
                    leaf: Leaf {
                        index: ([0], [1]).into(),
                        src: "/shed/b".into(),
                        dst: "b".into(),
                    },
                }],
            ),
        ];

        assert_eq!(actual, expected);
        assert_eq!(errors, vec![]);
    }

    #[test]
    fn does_not_infer_a_destination_root_from_a_common_ancestor() {
        let manifest = Manifest {
            name: "manifest".into(),
            location: "/shed/manifest.yaml".into(),
            path: None,
            shed: None,
            manifests: Vec::new(),
            items: vec![
                ItemKind::Item(Item {
                    path: "/config/a".into(),
                    shed: Some("a".into()),
                    items: None,
                }),
                ItemKind::Item(Item {
                    path: "/config/b".into(),
                    shed: Some("b".into()),
                    items: None,
                }),
            ],
        };

        let Plan { radix, errors } = Planner::new(Direction::Get).plan(&[manifest]);
        let actual = radix
            .range::<Path, _>(..)
            .map(|(components, entries)| {
                (components.into_iter().collect::<PathBuf>(), entries.clone())
            })
            .collect::<Vec<_>>();
        let expected = vec![
            (
                "/config/a".into(),
                vec![Entry {
                    dst_root: None,
                    leaf: Leaf {
                        index: ([0], [0]).into(),
                        src: "/shed/a".into(),
                        dst: "a".into(),
                    },
                }],
            ),
            (
                "/config/b".into(),
                vec![Entry {
                    dst_root: None,
                    leaf: Leaf {
                        index: ([0], [1]).into(),
                        src: "/shed/b".into(),
                        dst: "b".into(),
                    },
                }],
            ),
        ];

        assert_eq!(actual, expected);
        assert_eq!(errors, vec![]);
    }
}
