use crate::manifest::Manifest;

use super::super::model::{Direction, Plan};

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
