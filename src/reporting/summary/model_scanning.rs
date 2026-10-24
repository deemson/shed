use super::model::*;
use crate::scanning;

impl Report {
    pub fn scanning(
        &mut self,
        nodes: &[scanning::Node],
        events: impl IntoIterator<Item = scanning::Event>,
    ) {
        assert_eq!(self.status, ReportStatus::Traversed);
        self.status = ReportStatus::Scanning;
        self.scanning_nodes(nodes);
        for event in events {
            self.scanning_event(event);
        }
    }
    fn scanning_nodes(&mut self, nodes: &[scanning::Node]) {
        for node in nodes {
            self.scanning_node(node);
        }
    }
    fn scanning_node(&mut self, node: &scanning::Node) {
        match node {
            scanning::Node::EnsureDirectory { nodes, .. } => self.scanning_nodes(nodes),
            scanning::Node::ReplaceFile { .. } => (),
            scanning::Node::ReplaceDirectory { index, entries, .. } => {
                let item = self.get_item_mut(index.as_ref());
                item.entries = Some(entries.iter().map(Entry::from).collect());
            }
        }
    }
    fn scanning_event(&mut self, event: scanning::Event) {
        match event {
            scanning::Event::NodeScanningStarted { index } => {
                let item = self.get_item_mut(index.as_ref());
                item.scanning_started();
            }
            scanning::Event::NodeScanningFinished { index, error } => {
                let item = self.get_item_mut(index.as_ref());
                item.scanning_finished(error);
            }
            scanning::Event::EntryScanningStarted { index } => {
                let entry = self.get_entry_mut(index.as_ref());
                entry.scanning_started();
            }
            scanning::Event::EntryScanningFinished { index, error } => {
                let entry = self.get_entry_mut(index.as_ref());
                entry.scanning_finished(error);
            }
            scanning::Event::Done => self.status = ReportStatus::Scanned,
        }
    }
}

impl Item {
    fn scanning_started(&mut self) {
        assert_eq!(self.status, ItemStatus::Traversed);
        self.status = ItemStatus::Scanning;
    }
    fn scanning_finished(&mut self, error: Option<scanning::Error>) {
        assert_eq!(self.status, ItemStatus::Scanning);
        self.error = error.map(|error| error.to_string());
        self.status = ItemStatus::Scanned;
    }
}

impl Entry {
    fn scanning_started(&mut self) {
        assert_eq!(self.status, EntryStatus::Initial);
        self.status = EntryStatus::Scanning;
    }
    fn scanning_finished(&mut self, error: Option<scanning::Error>) {
        assert_eq!(self.status, EntryStatus::Scanning);
        self.error = error.map(|error| error.to_string());
        self.status = EntryStatus::Scanned;
    }
}
