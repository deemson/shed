use crate::traversal;
use std::{ffi::OsString, path::PathBuf};

#[derive(Debug)]
pub struct Report {
    pub direction: Option<traversal::Direction>,
    pub status: ReportStatus,
    pub manifests: Vec<Manifest>,
}

#[derive(Debug, PartialEq, PartialOrd, Eq, Ord)]
pub enum ReportStatus {
    Initial,
    Resolving,
    Resolved,
    Traversed,
    Scanning,
    Scanned,
    Executing,
    Executed,
}

#[derive(Debug)]
pub struct Manifest {
    pub name: String,
    pub location: PathBuf,
    pub path: Option<String>,
    pub shed: Option<String>,
    pub status: ManifestStatus,
    pub error: Option<String>,
    pub manifests: Vec<Manifest>,
    pub items: Vec<Item>,
}

#[derive(Debug, PartialEq, PartialOrd, Eq, Ord)]
pub enum ManifestStatus {
    Initial,
    Resolving,
    Resolved,
}

#[derive(Debug)]
pub struct Item {
    pub path: String,
    pub shed: Option<String>,
    pub status: ItemStatus,
    pub error: Option<String>,
    pub items: Vec<Item>,
    pub entries: Option<Vec<Entry>>,
}

#[derive(Debug, PartialEq, PartialOrd, Eq, Ord)]
pub enum ItemStatus {
    Initial,
    Traversed,
    Scanning,
    Scanned,
    Executing,
    Executed,
}

#[derive(Debug)]
pub struct Entry {
    pub name: OsString,
    pub status: EntryStatus,
    pub error: Option<String>,
    pub entries: Option<Vec<Entry>>,
}

#[derive(Debug, PartialEq, PartialOrd, Eq, Ord)]
pub enum EntryStatus {
    Initial,
    Scanning,
    Scanned,
    Executing,
    Executed,
}
