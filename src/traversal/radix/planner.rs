use super::super::model::Direction;
use super::model::Plan;
use crate::manifest::Manifest;

pub(in crate::traversal) struct Planner {
    direction: Direction,
}

impl Planner {
    pub fn new(direction: Direction) -> Self {
        Self { direction }
    }

    pub fn plan(&self, manifests: &[Manifest]) -> Plan {
        todo!()
    }
}
