use super::model::Manifest;

impl TryFrom<&str> for Manifest {
    type Error = yaml_serde::Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        yaml_serde::from_str(value)
    }
}

impl TryFrom<String> for Manifest {
    type Error = yaml_serde::Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Manifest::try_from(value.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::super::{Item, ItemKind};
    use super::*;
    use indoc::indoc;

    #[test]
    fn accepts_full_manifest() {
        let actual = Manifest::try_from(indoc! {"
          path: manifest-path
          shed: manifest-shed
          include:
            - manifest1
            - manifest2
          items:
            - item1-string
            - path: item2-path
              shed: item2-shed
              items:
                - item2-string-item
                - path: item2-object-item-path
                  shed: item2-object-item-shed
            - path: item3-path
        "})
        .unwrap();
        let expected = Manifest {
            path: Some("manifest-path".into()),
            shed: Some("manifest-shed".into()),
            include: ["manifest1", "manifest2"].map(String::from).into(),
            items: vec![
                ItemKind::Path("item1-string".into()),
                ItemKind::Item(Item {
                    path: "item2-path".into(),
                    shed: Some("item2-shed".into()),
                    items: Some(vec![
                        ItemKind::Path("item2-string-item".into()),
                        ItemKind::Item(Item {
                            path: "item2-object-item-path".into(),
                            shed: Some("item2-object-item-shed".into()),
                            items: None,
                        }),
                    ]),
                }),
                ItemKind::Item(Item {
                    path: "item3-path".into(),
                    shed: None,
                    items: None,
                }),
            ],
        };

        assert_eq!(actual, expected);
    }

    #[test]
    fn rejects_non_object_manifest() {
        let res = Manifest::try_from("bad");
        let error = res.expect_err("must error");

        assert_eq!(
            error.to_string(),
            r#"invalid type: string "bad", expected an object"#
        );
    }

    #[test]
    fn rejects_unknown_fields() {
        let res = Manifest::try_from("bad: value");
        let error = res.expect_err("must error");

        assert_eq!(
            error.to_string(),
            "unknown field `bad`, expected one of `path`, `shed`, `include`, `items`"
        );
    }

    #[test]
    fn accepts_empty_manifest() {
        let actual = Manifest::try_from("{}").unwrap();
        let expected = Manifest {
            path: None,
            shed: None,
            include: vec![],
            items: vec![],
        };

        assert_eq!(actual, expected);
    }

    #[test]
    fn accepts_bare_string_root_items() {
        let actual = Manifest::try_from(indoc! {"
          items:
            - bare-string
        "})
        .unwrap();

        assert_eq!(actual.items, vec![ItemKind::Path("bare-string".into())]);
    }
}
