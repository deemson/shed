use super::model::*;
use crate::scanning;

impl Report {
    pub fn scanning(
        &mut self,
        nodes: impl IntoIterator<Item = scanning::Node>,
        events: impl IntoIterator<Item = scanning::Event>,
    ) {
        self.scanning_nodes(nodes);
        for event in events {
            self.scanning_event(event);
        }
    }
    fn scanning_nodes(&mut self, nodes: impl IntoIterator<Item = scanning::Node>) {
        for node in nodes {
            self.scanning_node(node);
        }
    }
    fn scanning_node(&mut self, node: scanning::Node) {
        match node {
            scanning::Node::EnsureDirectory { nodes, .. } => self.scanning_nodes(nodes),
            scanning::Node::ReplaceFile { .. } => (),
            scanning::Node::ReplaceDirectory { index, entries, .. } => {
                let item = self.get_item_mut(index.as_ref());
                item.entries = entries.into_iter().map(Entry::from).collect();
            }
        }
    }
    fn scanning_event(&mut self, event: scanning::Event) {
        match event {
            scanning::Event::NodeScanningStarted { index } => (),
            scanning::Event::NodeScanningFinished { index, error } => (),
            scanning::Event::EntryScanningStarted { index } => (),
            scanning::Event::EntryScanningFinished { index, error } => (),
            scanning::Event::Done => (),
        }
    }
}
