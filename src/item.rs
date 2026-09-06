use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::error::Error;

#[derive(Deserialize)]
struct ItemEntry {
    system: String,
    shed: String,
}

pub struct ResolvedItem {
    pub name: String,
    pub entries: Vec<ResolvedEntry>,
}

pub struct ResolvedEntry {
    pub system: PathBuf,
    pub shed: PathBuf,
}

pub fn load_all(item_refs: &[String], items_dir: &Path) -> Result<Vec<ResolvedItem>, Error> {
    let mut items = Vec::new();

    for item_ref in item_refs {
        let item = load_item(item_ref, items_dir)?;
        items.push(item);
    }

    Ok(items)
}

fn load_item(item_ref: &str, items_dir: &Path) -> Result<ResolvedItem, Error> {
    let yaml_path = items_dir.join(format!("{}.yaml", item_ref));

    if !yaml_path.exists() {
        return Err(Error::ItemNotFound(yaml_path));
    }

    let content = fs::read_to_string(&yaml_path)
        .map_err(|e| Error::Io(format!("reading {:?}", yaml_path), e))?;

    let entries: Vec<ItemEntry> =
        yaml_serde::from_str(&content).map_err(|e| Error::Yaml(yaml_path.clone(), e))?;

    // Derive shed base directory from item path
    // e.g., items/editors/neovim.yaml -> items/editors/neovim/
    let shed_base = items_dir.join(item_ref);

    let resolved_entries = entries
        .into_iter()
        .map(|entry| ResolvedEntry {
            system: expand_home(&entry.system),
            shed: shed_base.join(&entry.shed),
        })
        .collect();

    Ok(ResolvedItem {
        name: item_ref.to_string(),
        entries: resolved_entries,
    })
}

fn expand_home(path: &str) -> PathBuf {
    if path.starts_with("$HOME") {
        if let Ok(home) = env::var("HOME") {
            return PathBuf::from(path.replacen("$HOME", &home, 1));
        }
    }
    PathBuf::from(path)
}
