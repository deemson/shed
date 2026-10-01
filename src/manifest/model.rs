use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema, Default, PartialEq)]
#[serde(deny_unknown_fields, expecting = "an object")]
pub struct ManifestFile {
    pub path: Option<String>,
    pub shed: Option<String>,
    #[serde(default)]
    pub include: Vec<String>,
    #[serde(default)]
    pub items: Vec<ItemKind>,
}

#[derive(Debug, Deserialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields, expecting = "an object")]
pub struct Item {
    pub path: String,
    pub shed: Option<String>,
    #[serde(default)]
    #[schemars(with = "Vec<ItemKind>")]
    pub items: Option<Vec<ItemKind>>,
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

#[derive(Debug, Deserialize, JsonSchema, PartialEq)]
#[serde(untagged)]
pub enum ItemKind {
    Path(String),
    Item(Item),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gets_child_at_indexes() {
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
