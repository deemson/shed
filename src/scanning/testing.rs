use super::{event::Event, model::Node, scanner::Scanner};
use crate::traversal;
use tokio::sync::mpsc;

pub(crate) async fn scan(
    traversal_roots: impl IntoIterator<Item = traversal::Node>,
) -> (Vec<Node>, Vec<Event>) {
    let (sender, mut receiver) = mpsc::channel(1);
    let scanner = Scanner::new(sender);

    let produce = async {
        let roots = scanner.scan(traversal_roots).await;
        drop(scanner);
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

#[track_caller]
pub(crate) fn assert_events_contain_no_errors(events: &[Event]) {
    assert!(!events.iter().any(|event| matches!(
        event,
        Event::NodeScanningFinished { error: Some(_), .. }
            | Event::EntryScanningFinished { error: Some(_), .. }
    )));
}

#[track_caller]
pub(crate) fn assert_events_contain_done_last(events: &[Event]) {
    assert!(matches!(events.last(), Some(Event::Done)));
}
