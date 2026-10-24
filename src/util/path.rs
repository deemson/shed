use std::{
    env::VarError,
    path::{Path, PathBuf},
};

pub(crate) fn expand(
    path: impl AsRef<Path>,
) -> Result<PathBuf, shellexpand::path::LookupError<VarError>> {
    shellexpand::path::full(path.as_ref()).map(|expanded| expanded.into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_paths_without_expansions() {
        for path in [
            "",
            "relative/path",
            "../dotfiles/./config",
            "/absolute/path",
        ] {
            assert_eq!(expand(path).unwrap(), PathBuf::from(path));
        }
    }

    #[test]
    fn accepts_string_and_path_types() {
        let text: &str = "relative/config";
        let string = String::from(text);
        let path: &Path = Path::new(text);
        let path_buf = PathBuf::from(text);
        let expected = PathBuf::from(text);

        assert_eq!(expand(text).unwrap(), expected);
        assert_eq!(expand(&string).unwrap(), expected);
        assert_eq!(expand(string).unwrap(), expected);
        assert_eq!(expand(path).unwrap(), expected);
        assert_eq!(expand(&path_buf).unwrap(), expected);
        assert_eq!(expand(path_buf).unwrap(), expected);
    }

    #[test]
    fn expands_tilde() {
        let home = shellexpand::path::tilde("~").into_owned();
        assert_ne!(home, PathBuf::from("~"), "test requires a home directory");

        assert_eq!(expand("~").unwrap(), home);
        assert_eq!(expand("~/config").unwrap(), home.join("config"));
    }

    #[test]
    fn expands_tilde_and_variables_together() {
        let home = shellexpand::path::tilde("~").into_owned();
        assert_ne!(home, PathBuf::from("~"), "test requires a home directory");
        let value = std::env::var("PATH").unwrap();
        let expected = PathBuf::from(format!("{}/prefix-{value}", home.display()));

        assert_eq!(expand("~/prefix-${PATH}").unwrap(), expected);
    }

    #[test]
    fn preserves_non_leading_tildes() {
        assert_eq!(
            expand("relative/~/config").unwrap(),
            PathBuf::from("relative/~/config")
        );
    }

    #[test]
    fn expands_unbraced_variables() {
        let value = std::env::var("PATH").unwrap();
        assert_eq!(expand("$PATH").unwrap(), PathBuf::from(value));
    }

    #[test]
    fn expands_braced_variables_with_adjacent_text() {
        let value = std::env::var("PATH").unwrap();
        assert_eq!(
            expand("prefix-${PATH}-suffix").unwrap(),
            PathBuf::from(format!("prefix-{value}-suffix")),
        );
    }

    #[test]
    fn expands_multiple_references() {
        let value = std::env::var("PATH").unwrap();
        assert_eq!(
            expand("$PATH/${PATH}/config").unwrap(),
            PathBuf::from(format!("{value}/{value}/config")),
        );
    }

    #[test]
    fn reports_unset_variables() {
        const NAME: &str = "SHED_TEST_UNSET_ENV";
        assert!(matches!(std::env::var(NAME), Err(VarError::NotPresent)));

        let error = expand(format!("${{{NAME}}}/config")).unwrap_err();
        assert_eq!(error.var_name, NAME);
        assert!(matches!(error.cause, VarError::NotPresent));
    }
}
