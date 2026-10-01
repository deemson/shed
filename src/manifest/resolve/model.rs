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
    pub fn get(&self, indexes: &[usize]) -> &Manifest {
        indexes
            .iter()
            .fold(self, |manifest, &index| &manifest.manifests[index])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gets_manifest_at_indexes() {
        let root = Manifest {
            name: "root".into(),
            path: PathBuf::from("root"),
            manifests: vec![Manifest {
                name: "child".into(),
                path: PathBuf::from("child"),
                manifests: vec![
                    Manifest {
                        name: "first".into(),
                        path: PathBuf::from("first"),
                        manifests: Vec::new(),
                        items: Vec::new(),
                    },
                    Manifest {
                        name: "second".into(),
                        path: PathBuf::from("second"),
                        manifests: Vec::new(),
                        items: Vec::new(),
                    },
                ],
                items: Vec::new(),
            }],
            items: Vec::new(),
        };

        assert_eq!(root.get(&[]), &root);
        assert_eq!(root.get(&[0]).name, "child");
        assert_eq!(root.get(&[0, 1]).name, "second");
    }
}
