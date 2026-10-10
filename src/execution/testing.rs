use super::{event::Event, executor::Executor};
use crate::scanning;
use tokio::sync::mpsc;

pub(super) async fn execute(dry: bool, nodes: &[scanning::Node]) -> Vec<Event> {
    let (sender, mut receiver) = mpsc::channel(1);
    let executor = Executor::new(dry, sender);

    let produce = async {
        executor.execute(nodes).await;
        drop(executor);
    };

    let collect = async {
        let mut events = Vec::new();
        while let Some(event) = receiver.recv().await {
            events.push(event);
        }
        events
    };

    let (_, events) = tokio::join!(produce, collect);

    events
}

#[track_caller]
pub(super) fn assert_events_contain_no_ensure_errors(events: &[Event]) {
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, Event::EnsureFinished { error: Some(_), .. }))
    );
}

#[track_caller]
pub(super) fn assert_events_contain_no_replace_errors(events: &[Event]) {
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, Event::ReplaceFinished { error: Some(_), .. }))
    );
}

#[track_caller]
pub(super) fn assert_events_contain_done_last(events: &[Event]) {
    assert!(matches!(events.last(), Some(Event::Done)));
}
