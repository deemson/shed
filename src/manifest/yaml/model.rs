use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema, Default, PartialEq)]
#[serde(deny_unknown_fields, expecting = "an object")]
pub struct Manifest {
    pub path: Option<String>,
    pub shed: Option<String>,
    #[serde(default)]
    pub include: Vec<String>,
    #[serde(default)]
    pub items: Vec<ItemKind>,
}

#[derive(Debug, Deserialize, JsonSchema, PartialEq)]
#[serde(untagged)]
pub enum ItemKind {
    Path(String),
    Item(Item),
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
