use super::model::*;
use crate::traversal;

impl Report {
    pub fn traversal(&mut self, direction: traversal::Direction, plan: &traversal::Plan) {
        self.direction = Some(direction);
        self.traversal_nodes(&plan.roots);
        let mut overlap_group = 1;
        for error in &plan.errors {
            let error_string = error.to_string();
            match error {
                traversal::Error::Expansion { index, .. }
                | traversal::Error::NonRootPathShedBothAbsolute { index }
                | traversal::Error::DestinationOutsideRoot { index, .. } => {
                    let item = self.get_item_mut(index.as_ref());
                    item.error = Some(error_string);
                }
                traversal::Error::OverlappingLeaves { indexes } => {
                    for index in indexes {
                        let item = self.get_item_mut(index.as_ref());
                        item.error = Some(format!("overlap #{overlap_group}"));
                    }
                    overlap_group += 1;
                }
            }
        }
    }
    fn traversal_nodes(&mut self, nodes: &[traversal::Node]) {
        for node in nodes {
            self.traversal_node(node);
        }
    }
    fn traversal_node(&mut self, node: &traversal::Node) {
        match node {
            traversal::Node::Directory { nodes, .. } => self.traversal_nodes(nodes),
            traversal::Node::Leaf { index, .. } => {
                let item = self.get_item_mut(index.as_ref());
                item.status = ItemStatus::Traversed
            }
        }
    }
}
