use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema, Default)]
#[serde(deny_unknown_fields, expecting = "an object")]
pub struct ManifestFile {
    /// Manifests to process before this manifest's local items.
    #[serde(default)]
    pub include: Vec<String>,
    /// Top-level system-to-shed path trees.
    #[serde(default)]
    pub items: Vec<RootItem>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RootItem {
    /// Absolute system path, optionally beginning with `$HOME`.
    pub path: String,
    /// Absolute shed path or a path relative to this manifest.
    pub shed: String,
    /// Relative child items; a node with children is a path prefix, not an endpoint.
    #[serde(default)]
    #[schemars(with = "Vec<ChildItem>")]
    pub items: Option<Vec<ChildItem>>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum ChildItem {
    Path(String),
    Item(Item),
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Item {
    /// Relative system-path suffix.
    pub path: String,
    /// Relative shed-path suffix; defaults to `path` when omitted.
    #[serde(default)]
    #[schemars(with = "String")]
    pub shed: Option<String>,
    /// Relative child items; a node with children is a path prefix, not an endpoint.
    #[serde(default)]
    #[schemars(with = "Vec<ChildItem>")]
    pub items: Option<Vec<ChildItem>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_object_manifest() {
        let res = yaml_serde::from_str::<ManifestFile>("bad");
        let error = res.expect_err("a manifest must be an object");

        assert_eq!(
            error.to_string(),
            r#"invalid type: string "bad", expected an object"#
        );
    }
}
