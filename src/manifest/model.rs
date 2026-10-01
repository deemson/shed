use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema, Default, PartialEq)]
#[serde(deny_unknown_fields, expecting = "an object")]
pub struct ManifestFile {
    #[serde(default)]
    pub include: Vec<String>,
    #[serde(default)]
    pub items: Vec<RootItem>,
}

#[derive(Debug, Deserialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields, expecting = "an object")]
pub struct RootItem {
    pub path: String,
    pub shed: Option<String>,
    #[serde(default)]
    #[schemars(with = "Vec<ChildItem>")]
    pub items: Option<Vec<ChildItem>>,
}

impl RootItem {
    pub fn get(&self, indexes: &[usize]) -> &ChildItem {
        let item = &self.items.as_ref().expect("root item has no children")[indexes[0]];

        indexes[1..].iter().fold(item, |item, &index| {
            let ChildItem::Item(item) = item else {
                panic!("path child has no children");
            };
            &item.items.as_ref().expect("item has no children")[index]
        })
    }
}

#[derive(Debug, Deserialize, JsonSchema, PartialEq)]
#[serde(untagged)]
pub enum ChildItem {
    Path(String),
    Item(Item),
}

#[derive(Debug, Deserialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Item {
    pub path: String,
    #[serde(default)]
    #[schemars(with = "String")]
    pub shed: Option<String>,
    #[serde(default)]
    #[schemars(with = "Vec<ChildItem>")]
    pub items: Option<Vec<ChildItem>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gets_child_at_indexes() {
        let root = RootItem {
            path: "root".into(),
            shed: None,
            items: Some(vec![
                ChildItem::Path("first".into()),
                ChildItem::Item(Item {
                    path: "parent".into(),
                    shed: None,
                    items: Some(vec![
                        ChildItem::Path("second".into()),
                        ChildItem::Path("third".into()),
                    ]),
                }),
            ]),
        };

        assert_eq!(root.get(&[0]), &ChildItem::Path("first".into()));
        assert_eq!(root.get(&[1, 1]), &ChildItem::Path("third".into()));
    }
}
