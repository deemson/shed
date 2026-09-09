use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::path::{Component, Path, PathBuf};

use schemars::JsonSchema;
use serde::{Deserialize, Deserializer};

use crate::error::Error;

#[derive(Debug, Deserialize, JsonSchema, Default)]
#[serde(deny_unknown_fields)]
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
    #[serde(default, deserialize_with = "present")]
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
    #[serde(default, deserialize_with = "present")]
    #[schemars(with = "String")]
    pub shed: Option<String>,
    /// Relative child items; a node with children is a path prefix, not an endpoint.
    #[serde(default, deserialize_with = "present")]
    #[schemars(with = "Vec<ChildItem>")]
    pub items: Option<Vec<ChildItem>>,
}

fn present<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}

#[derive(Clone, Debug)]
pub struct ResolvedEntry {
    pub system: PathBuf,
    pub shed: PathBuf,
}

#[derive(Clone, Debug)]
pub struct ResolvedItem {
    #[cfg(test)]
    pub name: String,
    pub shed_base: PathBuf,
    pub entries: Vec<ResolvedEntry>,
}

#[derive(Clone, Debug, Default)]
pub struct ReportLayout {
    pub sections: Vec<ReportSection>,
    pub nodes: Vec<ReportNode>,
    pub leaf_owners: Vec<usize>,
}

#[derive(Clone, Debug)]
pub struct ReportSection {
    pub heading: Option<String>,
    pub roots: Vec<usize>,
}

#[derive(Clone, Debug)]
pub struct ReportNode {
    pub label: String,
    pub origin: String,
    pub depth: usize,
    pub children: Vec<usize>,
    pub leaf: Option<usize>,
    pub duplicate_of: Option<usize>,
}

impl ReportLayout {
    #[cfg(test)]
    pub fn flat(items: &[ResolvedItem]) -> Self {
        let mut layout = Self::default();
        let mut roots = Vec::new();
        for (leaf, item) in items.iter().enumerate() {
            let node = layout.nodes.len();
            layout.nodes.push(ReportNode {
                label: item.name.clone(),
                origin: item.name.clone(),
                depth: 0,
                children: Vec::new(),
                leaf: Some(leaf),
                duplicate_of: None,
            });
            layout.leaf_owners.push(node);
            roots.push(node);
        }
        layout.sections.push(ReportSection {
            heading: None,
            roots,
        });
        layout
    }

    pub fn leaf_label(&self, leaf: usize) -> Option<(&str, usize)> {
        let node = *self.leaf_owners.get(leaf)?;
        let node = self.nodes.get(node)?;
        Some((&node.label, node.depth))
    }
}

#[derive(Debug)]
pub struct ResolvedConfig {
    pub items: Vec<ResolvedItem>,
    pub report: ReportLayout,
}

pub fn resolve(manifest_paths: &[PathBuf]) -> Result<ResolvedConfig, Error> {
    if manifest_paths.is_empty() {
        return Err(Error::InvalidConfig(
            "at least one manifest is required".to_string(),
        ));
    }

    let mut resolver = Resolver::default();
    for path in manifest_paths {
        let canonical = canonical_manifest(path)?;
        let root_dir = canonical.parent().ok_or_else(|| {
            Error::InvalidManifest(canonical.clone(), "manifest has no parent directory".into())
        })?;
        resolver.resolve_manifest(&canonical, root_dir)?;
    }

    Ok(ResolvedConfig {
        items: resolver.items,
        report: resolver.report,
    })
}

#[derive(Default)]
struct Resolver {
    visited: HashSet<PathBuf>,
    active: Vec<PathBuf>,
    items: Vec<ResolvedItem>,
    report: ReportLayout,
    endpoints: Vec<(PathBuf, PathBuf, usize, usize)>,
    exact: HashMap<(PathBuf, PathBuf), (usize, usize)>,
    home: Option<Result<PathBuf, String>>,
}

impl Resolver {
    fn resolve_manifest(&mut self, path: &Path, root_dir: &Path) -> Result<(), Error> {
        let canonical = canonical_manifest(path)?;
        if let Some(start) = self.active.iter().position(|active| active == &canonical) {
            let mut cycle: Vec<String> = self.active[start..]
                .iter()
                .map(|path| path.display().to_string())
                .collect();
            cycle.push(canonical.display().to_string());
            return Err(Error::IncludeCycle(cycle.join(" -> ")));
        }
        if self.visited.contains(&canonical) {
            return Ok(());
        }

        let manifest = load_manifest(&canonical)?;
        let directory = canonical.parent().ok_or_else(|| {
            Error::InvalidManifest(canonical.clone(), "manifest has no parent directory".into())
        })?;

        self.active.push(canonical.clone());
        for include in &manifest.include {
            if include.is_empty() {
                return Err(Error::InvalidManifest(
                    canonical.clone(),
                    "include paths cannot be empty".into(),
                ));
            }
            let mut include_path = PathBuf::from(include);
            if include_path.extension().is_none() {
                include_path.set_extension("yaml");
            }
            if include_path.is_relative() {
                include_path = directory.join(include_path);
            }
            self.resolve_manifest(&include_path, root_dir)?;
        }

        if !manifest.items.is_empty() {
            let mut roots = Vec::with_capacity(manifest.items.len());
            for item in manifest.items {
                roots.push(self.resolve_root_item(item, directory, &canonical)?);
            }
            self.report.sections.push(ReportSection {
                heading: Some(manifest_label(&canonical, root_dir)),
                roots,
            });
        }

        self.active.pop();
        self.visited.insert(canonical);
        Ok(())
    }

    fn resolve_root_item(
        &mut self,
        item: RootItem,
        manifest_dir: &Path,
        manifest_path: &Path,
    ) -> Result<usize, Error> {
        let authored_path = item.path.clone();
        let authored_shed = item.shed.clone();
        let system = self.expand_home(&item.path, manifest_path)?;
        if !system.is_absolute() {
            return Err(Error::InvalidManifest(
                manifest_path.to_path_buf(),
                format!("top-level item path must be absolute: {:?}", item.path),
            ));
        }
        let system = normalize_absolute(&system).map_err(|message| {
            Error::InvalidManifest(
                manifest_path.to_path_buf(),
                format!("{}: {message}", item.path),
            )
        })?;

        let shed = self.expand_home(&item.shed, manifest_path)?;
        let shed = if shed.is_absolute() {
            shed
        } else {
            manifest_dir.join(shed)
        };
        let shed = normalize_absolute(&shed).map_err(|message| {
            Error::InvalidManifest(
                manifest_path.to_path_buf(),
                format!("{}: {message}", item.shed),
            )
        })?;

        let label = item_label(&authored_path, Some(&authored_shed));
        self.resolve_node(label, system, shed, item.items, 0, manifest_path)
    }

    fn resolve_child(
        &mut self,
        item: ChildItem,
        parent_system: &Path,
        parent_shed: &Path,
        depth: usize,
        manifest_path: &Path,
    ) -> Result<usize, Error> {
        let (authored_path, authored_shed, children) = match item {
            ChildItem::Path(path) => (path.clone(), None, None),
            ChildItem::Item(item) => (item.path, item.shed, item.items),
        };
        let path = normalize_child(&authored_path).map_err(|message| {
            Error::InvalidManifest(
                manifest_path.to_path_buf(),
                format!("child path {:?}: {message}", authored_path),
            )
        })?;
        let shed_text = authored_shed.as_deref().unwrap_or(&authored_path);
        let shed = normalize_child(shed_text).map_err(|message| {
            Error::InvalidManifest(
                manifest_path.to_path_buf(),
                format!("child shed {:?}: {message}", shed_text),
            )
        })?;
        let label = item_label(&authored_path, authored_shed.as_deref());
        self.resolve_node(
            label,
            parent_system.join(path),
            parent_shed.join(shed),
            children,
            depth,
            manifest_path,
        )
    }

    fn resolve_node(
        &mut self,
        label: String,
        system: PathBuf,
        shed: PathBuf,
        children: Option<Vec<ChildItem>>,
        depth: usize,
        manifest_path: &Path,
    ) -> Result<usize, Error> {
        let node = self.report.nodes.len();
        let origin = format!("{}:{}", manifest_path.display(), system.display());
        self.report.nodes.push(ReportNode {
            label,
            origin,
            depth,
            children: Vec::new(),
            leaf: None,
            duplicate_of: None,
        });

        match children {
            Some(children) => {
                if children.is_empty() {
                    return Err(Error::InvalidManifest(
                        manifest_path.to_path_buf(),
                        format!(
                            "item {:?} has an empty items list",
                            self.report.nodes[node].label
                        ),
                    ));
                }
                let mut resolved = Vec::with_capacity(children.len());
                for child in children {
                    resolved.push(self.resolve_child(
                        child,
                        &system,
                        &shed,
                        depth + 1,
                        manifest_path,
                    )?);
                }
                self.report.nodes[node].children = resolved;
            }
            None => self.resolve_leaf(node, system, shed, manifest_path)?,
        }
        Ok(node)
    }

    fn resolve_leaf(
        &mut self,
        node: usize,
        system: PathBuf,
        shed: PathBuf,
        manifest_path: &Path,
    ) -> Result<(), Error> {
        if system == shed {
            return Err(Error::InvalidManifest(
                manifest_path.to_path_buf(),
                format!(
                    "item {:?} maps a path onto itself",
                    self.report.nodes[node].label
                ),
            ));
        }

        if let Some(&(leaf, owner)) = self.exact.get(&(system.clone(), shed.clone())) {
            self.report.nodes[node].leaf = Some(leaf);
            self.report.nodes[node].duplicate_of = Some(owner);
            return Ok(());
        }

        for (other_system, other_shed, _, owner) in &self.endpoints {
            if overlaps(&system, other_system) || overlaps(&shed, other_shed) {
                return Err(Error::InvalidManifest(
                    manifest_path.to_path_buf(),
                    format!(
                        "item {:?} overlaps item {:?}: {:?} <-> {:?}",
                        self.report.nodes[node].label,
                        self.report.nodes[*owner].label,
                        system,
                        shed
                    ),
                ));
            }
        }

        let leaf = self.items.len();
        self.items.push(ResolvedItem {
            #[cfg(test)]
            name: self.report.nodes[node].label.clone(),
            shed_base: shed.clone(),
            entries: vec![ResolvedEntry {
                system: system.clone(),
                shed: shed.clone(),
            }],
        });
        self.report.nodes[node].leaf = Some(leaf);
        self.report.leaf_owners.push(node);
        self.endpoints
            .push((system.clone(), shed.clone(), leaf, node));
        self.exact.insert((system, shed), (leaf, node));
        Ok(())
    }

    fn expand_home(&mut self, value: &str, manifest: &Path) -> Result<PathBuf, Error> {
        if value == "$HOME" || value.starts_with("$HOME/") {
            if self.home.is_none() {
                self.home = Some(validated_home());
            }
            let home = self
                .home
                .as_ref()
                .expect("home was initialized")
                .as_ref()
                .map_err(|message| {
                    Error::InvalidManifest(manifest.to_path_buf(), message.clone())
                })?;
            if value == "$HOME" {
                return Ok(home.clone());
            }
            return Ok(home.join(&value[6..]));
        }
        if value.starts_with("$HOME") {
            return Err(Error::InvalidManifest(
                manifest.to_path_buf(),
                format!("invalid $HOME expansion: {value:?}"),
            ));
        }
        if value.is_empty() {
            return Err(Error::InvalidManifest(
                manifest.to_path_buf(),
                "endpoint paths cannot be empty".into(),
            ));
        }
        Ok(PathBuf::from(value))
    }
}

fn load_manifest(path: &Path) -> Result<ManifestFile, Error> {
    let content = fs::read_to_string(path)
        .map_err(|error| Error::Io(format!("reading manifest {:?}", path), error))?;
    yaml_serde::from_str(&content).map_err(|error| Error::Yaml(path.to_path_buf(), error))
}

fn canonical_manifest(path: &Path) -> Result<PathBuf, Error> {
    path.canonicalize()
        .map_err(|error| Error::Io(format!("manifest {:?}", path), error))
}

fn validated_home() -> Result<PathBuf, String> {
    let home = env::var_os("HOME").ok_or_else(|| "HOME is not set".to_string())?;
    let home = PathBuf::from(home);
    if home.as_os_str().is_empty() || !home.is_absolute() {
        return Err("HOME must be a nonempty absolute path".to_string());
    }
    normalize_absolute(&home)
}

fn normalize_absolute(path: &Path) -> Result<PathBuf, String> {
    if !path.is_absolute() {
        return Err("path is not absolute".into());
    }
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    return Err("path climbs above the filesystem root".into());
                }
            }
            _ => normalized.push(component.as_os_str()),
        }
    }
    Ok(normalized)
}

fn normalize_child(value: &str) -> Result<PathBuf, String> {
    let path = Path::new(value);
    if value.is_empty() || path.is_absolute() {
        return Err("path must be a nonempty relative path".into());
    }
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => normalized.push(part),
            Component::CurDir => {}
            Component::ParentDir => return Err("parent components are not allowed".into()),
            Component::RootDir | Component::Prefix(_) => {
                return Err("absolute paths are not allowed".into());
            }
        }
    }
    if normalized.as_os_str().is_empty() {
        return Err("path must contain a normal component".into());
    }
    Ok(normalized)
}

fn overlaps(left: &Path, right: &Path) -> bool {
    left.starts_with(right) || right.starts_with(left)
}

fn item_label(path: &str, shed: Option<&str>) -> String {
    match shed {
        Some(shed) if shed != path => format!("{path} → {shed}"),
        _ => path.to_string(),
    }
}

fn manifest_label(path: &Path, root_dir: &Path) -> String {
    path.strip_prefix(root_dir)
        .unwrap_or(path)
        .display()
        .to_string()
}

pub fn schema() -> schemars::Schema {
    schemars::generate::SchemaSettings::default()
        .into_generator()
        .into_root_schema_for::<ManifestFile>()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write(path: &Path, content: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, content).unwrap();
    }

    #[test]
    fn resolves_includes_and_nested_items() {
        let temp = TempDir::new().unwrap();
        let home = temp.path().join("home");
        let manifests = temp.path().join("manifests");
        write(
            &manifests.join("leaf.yaml"),
            &format!(
                "items:\n  - path: {}/.config/app\n    shed: ../stored/app\n    items:\n      - config\n      - path: live.txt\n        shed: saved.txt\n",
                home.display()
            ),
        );
        write(&manifests.join("root.yaml"), "include:\n  - leaf\n");

        let resolved = resolve(&[manifests.join("root.yaml")]).unwrap();

        assert_eq!(resolved.items.len(), 2);
        assert_eq!(resolved.report.sections.len(), 1);
        assert_eq!(
            resolved.report.sections[0].heading.as_deref(),
            Some("leaf.yaml")
        );
        assert_eq!(
            resolved.items[0].entries[0].system,
            home.join(".config/app/config")
        );
        let canonical_temp = temp.path().canonicalize().unwrap();
        assert_eq!(
            resolved.items[0].entries[0].shed,
            canonical_temp.join("stored/app/config")
        );
        assert_eq!(
            resolved.items[1].entries[0].shed,
            canonical_temp.join("stored/app/saved.txt")
        );
    }

    #[test]
    fn rejects_top_level_strings_unknown_fields_and_nulls() {
        let string = yaml_serde::from_str::<ManifestFile>("items:\n  - child\n");
        let unknown = yaml_serde::from_str::<ManifestFile>("apps: []\n");
        let null_children = yaml_serde::from_str::<ManifestFile>(
            "items:\n  - path: /system\n    shed: stored\n    items: null\n",
        );
        let null_shed = yaml_serde::from_str::<ManifestFile>(
            "items:\n  - path: /system\n    shed: stored\n    items:\n      - path: child\n        shed: null\n",
        );
        assert!(string.is_err());
        assert!(unknown.is_err());
        assert!(null_children.is_err());
        assert!(null_shed.is_err());
    }

    #[test]
    fn rejects_empty_branches_and_child_parent_components() {
        let temp = TempDir::new().unwrap();
        let empty = temp.path().join("empty.yaml");
        write(
            &empty,
            &format!(
                "items:\n  - path: {}/system\n    shed: stored\n    items: []\n",
                temp.path().display()
            ),
        );
        assert!(resolve(&[empty]).is_err());

        let parent = temp.path().join("parent.yaml");
        write(
            &parent,
            &format!(
                "items:\n  - path: {}/system\n    shed: stored\n    items:\n      - ../escape\n",
                temp.path().display()
            ),
        );
        assert!(resolve(&[parent]).is_err());
    }

    #[test]
    fn rejects_overlaps_and_collapses_exact_duplicates() {
        let temp = TempDir::new().unwrap();
        let duplicate = temp.path().join("duplicate.yaml");
        write(
            &duplicate,
            &format!(
                "items:\n  - path: {0}/a\n    shed: one\n  - path: {0}/a\n    shed: one\n",
                temp.path().display()
            ),
        );
        let resolved = resolve(&[duplicate]).unwrap();
        assert_eq!(resolved.items.len(), 1);
        assert!(resolved.report.nodes[1].duplicate_of.is_some());

        let overlap = temp.path().join("overlap.yaml");
        write(
            &overlap,
            &format!(
                "items:\n  - path: {0}/a\n    shed: one\n  - path: {0}/a/child\n    shed: two\n",
                temp.path().display()
            ),
        );
        assert!(resolve(&[overlap]).is_err());
    }

    #[test]
    fn composes_multiple_roots_and_deduplicates_shared_includes() {
        let temp = TempDir::new().unwrap();
        write(
            &temp.path().join("shared.yaml"),
            &format!(
                "items:\n  - path: {0}/shared\n    shed: stored/shared\n",
                temp.path().display()
            ),
        );
        write(
            &temp.path().join("one.yaml"),
            &format!(
                "include: [shared]\nitems:\n  - path: {0}/one\n    shed: stored/one\n",
                temp.path().display()
            ),
        );
        write(
            &temp.path().join("two.yaml"),
            &format!(
                "include: [shared]\nitems:\n  - path: {0}/two\n    shed: stored/two\n",
                temp.path().display()
            ),
        );

        let resolved =
            resolve(&[temp.path().join("one.yaml"), temp.path().join("two.yaml")]).unwrap();

        assert_eq!(resolved.items.len(), 3);
        let headings: Vec<_> = resolved
            .report
            .sections
            .iter()
            .filter_map(|section| section.heading.as_deref())
            .collect();
        assert_eq!(headings, ["shared.yaml", "one.yaml", "two.yaml"]);
    }

    #[test]
    fn accepts_empty_manifests_and_rejects_invalid_root_endpoints() {
        let temp = TempDir::new().unwrap();
        let empty = temp.path().join("empty.yaml");
        write(&empty, "");
        let resolved = resolve(&[empty]).unwrap();
        assert!(resolved.items.is_empty());
        assert!(resolved.report.sections.is_empty());

        let relative = temp.path().join("relative.yaml");
        write(&relative, "items:\n  - path: relative\n    shed: stored\n");
        assert!(resolve(&[relative]).is_err());

        let same = temp.path().join("same.yaml");
        write(
            &same,
            &format!(
                "items:\n  - path: {0}/same\n    shed: {0}/same\n",
                temp.path().display()
            ),
        );
        assert!(resolve(&[same]).is_err());
    }

    #[test]
    fn generated_schema_is_strict() {
        let json = serde_json::to_value(schema()).unwrap();
        assert_eq!(json["additionalProperties"], false);
        assert_eq!(json["$defs"]["RootItem"]["additionalProperties"], false);
        assert_eq!(json["$defs"]["Item"]["additionalProperties"], false);
    }

    #[test]
    fn detects_include_cycles() {
        let temp = TempDir::new().unwrap();
        write(&temp.path().join("a.yaml"), "include: [b]\n");
        write(&temp.path().join("b.yaml"), "include: [a]\n");
        let error = resolve(&[temp.path().join("a.yaml")]).unwrap_err();
        assert!(error.to_string().contains("include cycle"));
    }
}
