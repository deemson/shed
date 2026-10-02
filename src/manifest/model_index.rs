use super::model::Manifest;
use super::yaml::{Item, ItemKind};

impl Manifest {
    pub fn get(&self, indexes: &[usize]) -> &Manifest {
        indexes
            .iter()
            .fold(self, |manifest, &index| &manifest.manifests[index])
    }
}

impl Item {
    pub fn get(&self, indexes: &[usize]) -> &ItemKind {
        let item = &self.items.as_ref().expect("item has no children")[indexes[0]];

        indexes[1..].iter().fold(item, |item, &index| {
            let ItemKind::Item(item) = item else {
                panic!("path child has no children");
            };
            &item.items.as_ref().expect("item has no children")[index]
        })
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

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

    #[test]
    fn gets_item_at_indexes() {
        let root = Item {
            path: "root".into(),
            shed: None,
            items: Some(vec![
                ItemKind::Path("first".into()),
                ItemKind::Item(Item {
                    path: "parent".into(),
                    shed: None,
                    items: Some(vec![
                        ItemKind::Path("second".into()),
                        ItemKind::Path("third".into()),
                    ]),
                }),
            ]),
        };

        assert_eq!(root.get(&[0]), &ItemKind::Path("first".into()));
        assert_eq!(root.get(&[1, 1]), &ItemKind::Path("third".into()));
    }

}
