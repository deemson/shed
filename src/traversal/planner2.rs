use super::model::{Direction, NestedLeaf, Plan, Root, RootDirectory, RootLeaf};
use crate::manifest::{FullIndex as ManifestIndex, ItemKind, Manifest};
use std::path::PathBuf;

pub struct Planner {
    direction: Direction,
}

impl Planner {
    pub fn new(direction: Direction) -> Self {
        Self { direction }
    }

    pub fn plan(&self, manifests: impl IntoIterator<Item = Manifest>) -> Plan {
        let mut plan = Plan {
            roots: Vec::new(),
            errors: Vec::new(),
        };

        for (manifest_position, manifest) in manifests.into_iter().enumerate() {
            self.plan_manifest(&mut plan, manifest, vec![manifest_position]);
        }

        plan
    }

    fn plan_manifest(&self, plan: &mut Plan, manifest: Manifest, manifest_index: Vec<usize>) {
        let manifest_directory = manifest
            .location
            .parent()
            .expect("a resolved manifest location has a parent")
            .to_path_buf();
        let manifest_path = manifest.path.map(PathBuf::from);
        if manifest.shed.is_some() {
            todo!("plan manifest-level shed paths");
        }

        for (child_position, child_manifest) in manifest.manifests.into_iter().enumerate() {
            let mut child_index = manifest_index.clone();
            child_index.push(child_position);
            self.plan_manifest(plan, child_manifest, child_index);
        }

        if let Some(path_root) = manifest_path {
            let (_, root_dst) =
                self.resolve_direction(path_root.clone(), manifest_directory.clone());
            let mut leaves = Vec::new();

            for (item_position, item) in manifest.items.into_iter().enumerate() {
                let ItemKind::Path(item_path) = item else {
                    todo!("plan explicit items under a manifest-level path");
                };

                let path = path_root.join(&item_path);
                let shed = manifest_directory.join(&item_path);
                let (src, dst) = self.resolve_direction(path, shed);
                let relative_dst = dst
                    .strip_prefix(&root_dst)
                    .expect("a child destination is beneath its root destination")
                    .as_os_str()
                    .to_owned();

                leaves.push(NestedLeaf {
                    manifest_index: (manifest_index.clone(), [item_position]).into(),
                    src,
                    dst: relative_dst,
                });
            }

            let root_manifest_index: ManifestIndex = (manifest_index, []).into();
            plan.roots.push(Root::Directory(RootDirectory {
                manifest_indexes: [root_manifest_index].into(),
                dst: root_dst,
                directories: Vec::new(),
                leaves,
            }));
            return;
        }

        for (item_position, item) in manifest.items.into_iter().enumerate() {
            let ItemKind::Item(item) = item else {
                todo!("plan bare path items");
            };
            if item.items.is_some() {
                todo!("plan directory items");
            }
            let Some(shed) = item.shed else {
                todo!("plan root leaves without an explicit shed path");
            };

            let path = PathBuf::from(item.path);
            let shed = manifest_directory.join(shed);
            let (src, dst) = self.resolve_direction(path, shed);

            plan.roots.push(Root::Leaf(RootLeaf {
                manifest_index: (manifest_index.clone(), [item_position]).into(),
                src,
                dst,
            }));
        }
    }

    fn resolve_direction(&self, path: PathBuf, shed: PathBuf) -> (PathBuf, PathBuf) {
        match &self.direction {
            Direction::Put => (path, shed),
            Direction::Get => (shed, path),
        }
    }
}

#[cfg(test)]
mod tests {
    use iradix::unsync::Radix;
    use std::{ffi::OsString, path::Path};

    #[test]
    fn radix_indexes_paths_and_queries_their_relationships() {
        // Path keys are split on path components, not bytes. This is the shape the
        // planner can use to detect exact, ancestor, and descendant destinations.
        let mut paths: Radix<OsString, &str> = Radix::new();
        paths.insert(Path::new("home/alice"), "home");
        paths.insert(Path::new("home/alice/.config"), "config");
        paths.insert(Path::new("home/alice/.config/fish"), "fish");
        paths.insert(Path::new("home/alice/.config/nvim"), "nvim");
        paths.insert(Path::new("home/alice-old"), "sibling");

        // Exact lookup.
        assert_eq!(paths.get(Path::new("home/alice/.config")), Some(&"config"));

        // Longest-prefix lookup finds the nearest planned ancestor.
        assert_eq!(
            paths.get_ancestor(Path::new("home/alice/.config/nvim/init.lua")),
            Some(&"nvim")
        );
        assert_eq!(
            paths.strict_ancestor(Path::new("home/alice/.config/nvim")),
            Some(&"config")
        );

        // Descendant queries are strict and returned in path order.
        assert_eq!(
            paths
                .descendants(Path::new("home/alice/.config"))
                .copied()
                .collect::<Vec<_>>(),
            vec!["fish", "nvim"]
        );

        // Component-aware matching does not treat `alice` as a prefix of
        // `alice-old`, unlike a byte-prefix data structure.
        assert_eq!(paths.strict_ancestor(Path::new("home/alice-old")), None);

        // Cloning creates an isolated, copy-on-write snapshot. Prefix deletion
        // mutates only the live trie and removes the key plus all descendants.
        let snapshot = paths.clone();
        assert_eq!(paths.delete_prefix(Path::new("home/alice/.config")), 3);
        assert_eq!(paths.get(Path::new("home/alice/.config")), None);
        assert_eq!(
            snapshot.get(Path::new("home/alice/.config/nvim")),
            Some(&"nvim")
        );
    }
}
