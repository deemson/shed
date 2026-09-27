use super::event::Event;
use super::model::{Direction, Root};
use super::planner::Planner;
use crate::manifest::Manifest;
use tokio::sync::mpsc;

pub(crate) async fn plan(
    direction: Direction,
    manifests: impl IntoIterator<Item = Manifest>,
) -> (Vec<Root>, Vec<Event>) {
    let (sender, mut receiver) = mpsc::channel(1);
    let planner = Planner::new(direction, sender);

    let produce = async {
        let roots = planner.plan(manifests).await;
        drop(planner);
        roots
    };

    let collect = async {
        let mut events = Vec::new();
        while let Some(event) = receiver.recv().await {
            events.push(event);
        }
        events
    };

    tokio::join!(produce, collect)
}
