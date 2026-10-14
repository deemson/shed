use super::model::*;
use crate::traversal;

impl Report {
    pub fn traversal(&mut self, plan: traversal::Plan) {
        let mut overlap_group = 0;
        for error in plan.errors {
            let error_string = error.to_string();
            match error {
                traversal::Error::Expansion {
                    index,
                    field,
                    source,
                } => {
                    let item = self.get_item_mut(index.as_ref());
                    item.error = Some(error_string);
                }
                traversal::Error::NonRootPathShedBothAbsolute { index } => todo!(),
                traversal::Error::OverlappingLeaves { indexes } => {
                    for index in indexes {
                        let item = self.get_item_mut(index.as_ref());
                        item.error = Some(format!("overlap group {overlap_group}"));
                    }
                    overlap_group += 1;
                }
                traversal::Error::DestinationOutsideRoot { index, root, dst } => todo!(),
            }
        }
    }
}
