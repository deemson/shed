use std::{env::VarError, path::PathBuf};

pub(crate) fn expand_env(path: &str) -> Result<PathBuf, shellexpand::LookupError<VarError>> {
    shellexpand::env(path).map(|expanded| PathBuf::from(expanded.as_ref()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_paths_without_variables() {
        for path in [
            "",
            "relative/path",
            "../dotfiles/./config",
            "/absolute/path",
            "~/config",
        ] {
            assert_eq!(expand_env(path).unwrap(), PathBuf::from(path));
        }
    }

    #[test]
    fn expands_unbraced_variables() {
        let value = std::env::var("PATH").unwrap();
        assert_eq!(expand_env("$PATH").unwrap(), PathBuf::from(value));
    }

    #[test]
    fn expands_braced_variables_with_adjacent_text() {
        let value = std::env::var("PATH").unwrap();
        assert_eq!(
            expand_env("prefix-${PATH}-suffix").unwrap(),
            PathBuf::from(format!("prefix-{value}-suffix")),
        );
    }

    #[test]
    fn expands_multiple_references() {
        let value = std::env::var("PATH").unwrap();
        assert_eq!(
            expand_env("$PATH/${PATH}/config").unwrap(),
            PathBuf::from(format!("{value}/{value}/config")),
        );
    }

    #[test]
    fn reports_unset_variables() {
        const NAME: &str = "SHED_TEST_UNSET_ENV";
        assert!(matches!(std::env::var(NAME), Err(VarError::NotPresent)));

        let error = expand_env(&format!("${{{NAME}}}/config")).unwrap_err();
        assert_eq!(error.var_name, NAME);
        assert!(matches!(error.cause, VarError::NotPresent));
    }
}
