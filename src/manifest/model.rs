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
    pub shed: String,
    #[serde(default)]
    #[schemars(with = "Vec<ChildItem>")]
    pub items: Option<Vec<ChildItem>>,
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
