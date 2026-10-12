use super::model::*;
use crate::{manifest, traversal};

pub fn build(manifests: &[manifest::Manifest], plan: traversal::Plan) -> Report {
    let mut report: Report = manifests.into();
    for error in plan.errors {
        match error {
            traversal::Error::OverlappingLeaves { ref indexes } => {
                for index in indexes {
                    let item = report.get_item_mut(index.as_ref());
                    item.error = Some(error);
                }
            }
            traversal::Error::Expansion { ref index, .. }
            | traversal::Error::NonRootPathShedBothAbsolute { ref index }
            | traversal::Error::DestinationOutsideRoot { ref index, .. } => {
                let item = report.get_item_mut(index.as_ref());
                item.error = Some(error);
            }
        }
    }
    report
}
