use crate::manifest::RootItem;
use std::path::PathBuf;

#[derive(Debug, PartialEq)]
pub struct Manifest {
    pub name: String,
    pub path: PathBuf,
    pub manifests: Vec<Manifest>,
    pub items: Vec<RootItem>,
}

impl Manifest {
    pub fn get(&self, indexes: &[usize]) -> Option<&Manifest> {
        indexes
            .iter()
            .try_fold(self, |manifest, &index| manifest.manifests.get(index))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(name: &str, manifests: Vec<Manifest>) -> Manifest {
        Manifest {
            name: name.into(),
            path: PathBuf::from(name),
            manifests,
            items: Vec::new(),
        }
    }

    #[test]
    fn gets_manifest_at_indexes() {
        let root = manifest(
            "root",
            vec![manifest(
                "child",
                vec![
                    manifest("first", Vec::new()),
                    manifest("second", Vec::new()),
                ],
            )],
        );

        assert_eq!(root.get(&[]), Some(&root));
        assert_eq!(
            root.get(&[0]).map(|manifest| manifest.name.as_str()),
            Some("child")
        );
        assert_eq!(
            root.get(&[0, 1]).map(|manifest| manifest.name.as_str()),
            Some("second")
        );
        assert_eq!(root.get(&[1]), None);
        assert_eq!(root.get(&[0, 2]), None);
    }
}
