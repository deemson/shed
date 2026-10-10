use super::super::{
    error::{Error, PathField},
    model::Direction,
};
use super::model::{Entry, Plan};
use crate::{
    manifest::{FullIndex, Index, Item, ItemKind, Manifest},
    util,
};
use path_clean::PathClean;
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

        let full_index = FullIndex {
            manifest: index.clone(),
            item: Vec::new(),
        };

        let path = match manifest
            .path
            .as_deref()
            .map(|path| Self::expand_path(&full_index, PathField::Path, path))
            .transpose()
        {
            Ok(path) => path.unwrap_or_default(),
            Err(error) => {
                plan.errors.push(error);
                return;
            }
        }.clean();

        let shed = match manifest
            .shed
            .as_deref()
            .map(|shed| Self::expand_path(&full_index, PathField::Shed, shed))
            .transpose()
        {
            Ok(Some(shed)) => directory.join(shed),
            Ok(None) => directory.to_path_buf(),
            Err(error) => {
                plan.errors.push(error);
                return;
            }
        }.clean();

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
        let item_path = match Self::expand_path(&index, PathField::Path, &item.path) {
            Ok(path) => path,
            Err(error) => {
                plan.errors.push(error);
                return;
            }
        };
        let item_shed = match item
            .shed
            .as_deref()
            .map(|shed| Self::expand_path(&index, PathField::Shed, shed))
            .transpose()
        {
            Ok(shed) => shed,
            Err(error) => {
                plan.errors.push(error);
                return;
            }
        };

        let path_is_absolute = item_path.is_absolute();
        let shed_is_absolute = item_shed.as_deref().is_some_and(Path::is_absolute);

        if index.item.len() > 1 && path_is_absolute && shed_is_absolute {
            plan.errors
                .push(Error::NonRootPathShedBothAbsolute { index });
            return;
        }

        let path = path_parent.join(item_path).clean();
        let shed = match item_shed {
            Some(shed) => shed_parent.join(shed),
            None if Path::new(&item.path).is_absolute() => shed_parent.to_path_buf(),
            None => shed_parent.join(&item.path),
        }
        .clean();

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

            if let Some(root) = dst_root
                && !dst.starts_with(root)
            {
                plan.errors.push(Error::DestinationOutsideRoot {
                    index,
                    root: root.to_path_buf(),
                    dst,
                });
                return;
            }

            let mut entries = plan.radix.get(&dst).cloned().unwrap_or_default();
            entries.push(Entry {
                dst_root: dst_root.map(Path::to_path_buf),
                index,
                src,
            });
            plan.radix.insert(&dst, entries);
        }
    }

    fn expand_path(
        index: &FullIndex,
        field: PathField,
        path: impl AsRef<Path>,
    ) -> Result<PathBuf, Error> {
        util::path::expand(path).map_err(|source| Error::Expansion {
            index: index.clone(),
            field,
            source,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

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
                    index: ([0], [0]).into(),
                    src: "/shed/a".into(),
                }],
            ),
            (
                "/config/b".into(),
                vec![Entry {
                    dst_root: Some("/config".into()),
                    index: ([0], [1]).into(),
                    src: "/shed/b".into(),
                }],
            ),
        ];

        assert_eq!(errors, vec![]);
        assert_eq!(actual, expected);
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
                    index: ([0], [0]).into(),
                    src: "/shed/a".into(),
                }],
            ),
            (
                "/config/b".into(),
                vec![Entry {
                    dst_root: None,
                    index: ([0], [1]).into(),
                    src: "/shed/b".into(),
                }],
            ),
        ];

        assert_eq!(errors, vec![]);
        assert_eq!(actual, expected);
    }

    #[test]
    fn expands_manifest_root_and_normalizes_joined_item_paths() {
        let manifest = Manifest {
            name: "manifest".into(),
            location: "/dotfiles/shed/manifest.yaml".into(),
            path: Some("$HOME/.config/app".into()),
            shed: Some("../app".into()),
            manifests: Vec::new(),
            items: vec![
                ItemKind::Item(Item {
                    path: "config".into(),
                    shed: None,
                    items: None,
                }),
                ItemKind::Item(Item {
                    path: "dir/../schema".into(),
                    shed: None,
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
        let home = PathBuf::from(std::env::var("HOME").unwrap());
        let dst_root = home.join(".config/app");
        let expected = vec![
            (
                dst_root.join("config"),
                vec![Entry {
                    dst_root: Some(dst_root.clone()),
                    index: ([0], [0]).into(),
                    src: "/dotfiles/app/config".into(),
                }],
            ),
            (
                dst_root.join("schema"),
                vec![Entry {
                    dst_root: Some(dst_root),
                    index: ([0], [1]).into(),
                    src: "/dotfiles/app/schema".into(),
                }],
            ),
        ];

        assert_eq!(errors, vec![]);
        assert_eq!(actual, expected);
    }
}
