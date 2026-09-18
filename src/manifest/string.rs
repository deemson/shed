use super::serde::ManifestFile;

impl TryFrom<&str> for ManifestFile {
    type Error = yaml_serde::Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        yaml_serde::from_str(value)
    }
}

impl TryFrom<String> for ManifestFile {
    type Error = yaml_serde::Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        ManifestFile::try_from(value.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::super::serde::*;

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
        let actual = ManifestFile::try_from(s).unwrap();
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
        let res = ManifestFile::try_from("bad");
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
        let res = ManifestFile::try_from(s);
        let error = res.expect_err("must error");

        assert_eq!(
            error.to_string(),
            "unknown field `bad`, expected `include` or `items`"
        );
    }

    #[test]
    fn accepts_empty_manifest() {
        #[rustfmt::skip]
        let s = [
            "{}",
        ].join("\n");

        let actual = ManifestFile::try_from(s).unwrap();
        let expected = ManifestFile {
            include: vec![],
            items: vec![],
        };

        assert_eq!(actual, expected);
    }

    #[test]
    fn rejects_bare_string_root_items() {
        #[rustfmt::skip]
        let s = [
            "items:",
            "  - bare-string",
        ].join("\n");
        let res = ManifestFile::try_from(s);
        let error = res.expect_err("must error");

        assert_eq!(
            error.to_string(),
            r#"items[0]: invalid type: string "bare-string", expected an object at line 2 column 5"#
        );
    }
}
