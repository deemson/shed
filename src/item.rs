use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::Deserialize;

use crate::error::Error;

#[derive(Deserialize, JsonSchema)]
pub struct ItemEntry {
    /// Absolute system path (supports $HOME expansion)
    pub system: String,
    /// Relative path within the item's storage directory
    pub shed: String,
}

pub struct ResolvedItem {
    pub name: String,
    pub shed_base: PathBuf,
    pub entries: Vec<ResolvedEntry>,
}

pub struct ResolvedEntry {
    pub system: PathBuf,
    pub shed: PathBuf,
}

pub fn load_all(
    item_refs: &[String],
    config_dir: &Path,
    sync_dir: &Path,
) -> Result<Vec<ResolvedItem>, Error> {
    let mut items = Vec::new();

    for item_ref in item_refs {
        let item = load_item(item_ref, config_dir, sync_dir)?;
        items.push(item);
    }

    Ok(items)
}

fn load_item(item_ref: &str, config_dir: &Path, sync_dir: &Path) -> Result<ResolvedItem, Error> {
    // Item YAML is resolved from config directory
    let yaml_path = config_dir.join(format!("{}.yaml", item_ref));

    if !yaml_path.exists() {
        return Err(Error::ItemNotFound(yaml_path));
    }

    let content = fs::read_to_string(&yaml_path)
        .map_err(|e| Error::Io(format!("reading {:?}", yaml_path), e))?;

    let entries: Vec<ItemEntry> =
        yaml_serde::from_str(&content).map_err(|e| Error::Yaml(yaml_path.clone(), e))?;

    // Shed paths are resolved from sync directory (cwd)
    // e.g., editors/neovim -> <sync_dir>/editors/neovim/
    let shed_base = sync_dir.join(item_ref);

    let resolved_entries = entries
        .into_iter()
        .map(|entry| ResolvedEntry {
            system: expand_home(&entry.system),
            shed: shed_base.join(&entry.shed),
        })
        .collect();

    Ok(ResolvedItem {
        name: item_ref.to_string(),
        shed_base,
        entries: resolved_entries,
    })
}

fn expand_home(path: &str) -> PathBuf {
    if path.starts_with("$HOME")
        && let Ok(home) = env::var("HOME")
    {
        return PathBuf::from(path.replacen("$HOME", &home, 1));
    }
    PathBuf::from(path)
}

pub fn schema() -> schemars::Schema {
    schemars::generate::SchemaSettings::default()
        .into_generator()
        .into_root_schema_for::<Vec<ItemEntry>>()
}
