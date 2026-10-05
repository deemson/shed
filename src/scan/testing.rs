use super::{event::Event, model::Root, scanner::Scanner};
use crate::traversal;
use tokio::sync::mpsc;

pub(crate) async fn scan(
    traversal_roots: impl IntoIterator<Item = traversal::Root>,
) -> (Vec<Root>, Vec<Event>) {
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
