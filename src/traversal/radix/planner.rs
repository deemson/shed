use crate::manifest::Manifest;

use super::super::model::Direction;
use super::model::Plan;

pub(in crate::traversal) struct Planner {
    direction: Direction,
}

impl Planner {
    pub fn new(direction: Direction) -> Self {
        Self { direction }
    }

    pub fn plan(
        &self,
        manifests: impl IntoIterator<Item = Manifest>,
    ) -> Plan {
        todo!()
    }
}
