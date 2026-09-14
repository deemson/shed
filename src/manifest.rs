use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema, Default, PartialEq)]
#[serde(deny_unknown_fields, expecting = "an object")]
pub struct ManifestFile {
    /// Manifests to process before this manifest's local items.
    #[serde(default)]
    pub include: Vec<String>,
    /// Top-level system-to-shed path trees.
    #[serde(default)]
    pub items: Vec<RootItem>,
}

#[derive(Debug, Deserialize, JsonSchema, PartialEq)]
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

#[derive(Debug, Deserialize, JsonSchema, PartialEq)]
#[serde(untagged)]
pub enum ChildItem {
    Path(String),
    Item(Item),
}

#[derive(Debug, Deserialize, JsonSchema, PartialEq)]
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
    fn accepts_full_manifest() {
        #[rustfmt::skip]
        let s = [
            "include:",
            "  - manifest1",
            "  - manifest2",
            "items:",
            "  - path: item1-path",
            "    shed: item1-shed",
            "  - path: item2-path",
            "    shed: item2-shed",
            "    items:",
            "      - item2-string-item",
            "      - path: item2-object-item-path",
            "        shed: item2-object-item-shed",
        ].join("\n");
        let actual = yaml_serde::from_str::<ManifestFile>(&s).unwrap();
        let expected = ManifestFile {
            include: ["manifest1", "manifest2"].map(String::from).into(),
            items: vec![
                RootItem {
                    path: "item1-path".into(),
                    shed: "item1-shed".into(),
                    items: None,
                },
                RootItem {
                    path: "item2-path".into(),
                    shed: "item2-shed".into(),
                    items: Some(vec![
                        ChildItem::Path("item2-string-item".into()),
                        ChildItem::Item(Item {
                            path: "item2-object-item-path".into(),
                            shed: Some("item2-object-item-shed".into()),
                            items: None,
                        }),
                    ]),
                },
            ],
        };

        assert_eq!(actual, expected);
    }

    #[test]
    fn rejects_non_object_manifest() {
        let res = yaml_serde::from_str::<ManifestFile>("bad");
        let error = res.expect_err("must error");

        assert_eq!(
            error.to_string(),
            r#"invalid type: string "bad", expected an object"#
        );
    }

    #[test]
    fn rejects_unknown_fields() {
        #[rustfmt::skip]
        let s = [
            "bad: value",
        ].join("\n");
        let res = yaml_serde::from_str::<ManifestFile>(&s);
        let error = res.expect_err("must error");

        assert_eq!(
            error.to_string(),
            "unknown field `bad`, expected `include` or `items`"
        );
    }
}
