use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::Deserialize;

use crate::error::Error;

#[derive(Deserialize, JsonSchema, Default)]
pub struct CollectionFile {
    /// If true, this collection cannot be run directly (only inherited)
    #[serde(default, rename = "abstract")]
    pub r#abstract: bool,
    /// List of collection names to inherit (relative to this file's directory)
    #[serde(default)]
    pub inherit: Vec<String>,
    /// List of item references (paths relative to items directory, without .yaml)
    #[serde(default)]
    pub items: Vec<String>,
}

pub struct ResolvedCollection {
    pub abstract_root: bool,
    pub item_refs: Vec<String>,
}

pub fn resolve(collection_path: &Path, config_dir: &Path) -> Result<ResolvedCollection, Error> {
    let mut visited = HashSet::new();
    let mut chain = Vec::new();
    let mut all_items = Vec::new();
    let mut seen_items = HashSet::new();

    let root_name = collection_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("root")
        .to_string();

    let root = load_collection(collection_path)?;
    let abstract_root = root.r#abstract;

    resolve_recursive(
        &root_name,
        collection_path,
        config_dir,
        &mut visited,
        &mut chain,
        &mut all_items,
        &mut seen_items,
    )?;

    Ok(ResolvedCollection {
        abstract_root,
        item_refs: all_items,
    })
}

fn resolve_recursive(
    name: &str,
    path: &Path,
    base_dir: &Path,
    visited: &mut HashSet<PathBuf>,
    chain: &mut Vec<String>,
    all_items: &mut Vec<String>,
    seen_items: &mut HashSet<String>,
) -> Result<(), Error> {
    let canonical = path.canonicalize().map_err(|e| {
        Error::Io(format!("collection {:?}", path), e)
    })?;

    // Check for cycle first (before checking visited)
    if chain.contains(&name.to_string()) {
        chain.push(name.to_string());
        return Err(Error::InheritanceCycle(chain.join(" -> ")));
    }

    // Skip if already fully processed (not a cycle, just diamond inheritance)
    if visited.contains(&canonical) {
        return Ok(());
    }

    chain.push(name.to_string());

    let collection = load_collection(path)?;

    // Process inherited collections first
    for inherit_name in &collection.inherit {
        let inherit_path = base_dir.join(format!("{}.yaml", inherit_name));
        resolve_recursive(
            inherit_name,
            &inherit_path,
            base_dir,
            visited,
            chain,
            all_items,
            seen_items,
        )?;
    }

    // Add items from this collection (deduplicate)
    for item in &collection.items {
        if seen_items.insert(item.clone()) {
            all_items.push(item.clone());
        }
    }

    // Mark as visited only after fully processing (so cycles are detected)
    visited.insert(canonical);
    chain.pop();
    Ok(())
}

fn load_collection(path: &Path) -> Result<CollectionFile, Error> {
    let content = fs::read_to_string(path)
        .map_err(|e| Error::Io(format!("reading {:?}", path), e))?;
    yaml_serde::from_str(&content).map_err(|e| Error::Yaml(path.to_path_buf(), e))
}

pub fn schema() -> schemars::Schema {
    schemars::generate::SchemaSettings::default()
        .into_generator()
        .into_root_schema_for::<CollectionFile>()
}
