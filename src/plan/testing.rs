use super::model::{Direction, Root};
use super::planner::Planner;
use crate::manifest::Manifest;

pub(crate) async fn plan(
    direction: Direction,
    manifests: impl IntoIterator<Item = Manifest>,
) -> Vec<Root> {
    let planner = Planner::new(direction);
    planner.plan(manifests).await
}
