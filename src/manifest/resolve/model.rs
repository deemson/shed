use crate::manifest::ItemKind;
use std::path::PathBuf;

#[derive(Debug, PartialEq)]
pub struct Manifest {
    pub name: String,
    pub location: PathBuf,
    pub path: Option<String>,
    pub shed: Option<String>,
    pub manifests: Vec<Manifest>,
    pub items: Vec<ItemKind>,
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
            location: PathBuf::from("root"),
            path: None,
            shed: None,
            manifests: vec![Manifest {
                name: "child".into(),
                location: PathBuf::from("child"),
                path: None,
                shed: None,
                manifests: vec![
                    Manifest {
                        name: "first".into(),
                        location: PathBuf::from("first"),
                        path: None,
                        shed: None,
                        manifests: Vec::new(),
                        items: Vec::new(),
                    },
                    Manifest {
                        name: "second".into(),
                        location: PathBuf::from("second"),
                        path: None,
                        shed: None,
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
