use super::model::*;
use crate::{manifest, traversal};

pub fn build(manifests: &[manifest::Manifest], plan: traversal::Plan) -> Report {
    let mut report: Report = manifests.into();
    report
}
