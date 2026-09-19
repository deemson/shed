use tokio::sync::mpsc;

use super::event::Event;
use super::model::Manifest;
use super::resolver::{Input, Resolver};
use std::{
    fs,
    path::{Path, PathBuf},
};

fn write(path: &Path, source: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, source).unwrap();
}

pub(crate) fn write_yaml_manifest(directory: &Path, name: &str, lines: &[&str]) -> PathBuf {
    let path = directory.join(name).with_extension("yaml");
    write(&path, &lines.join("\n"));
    fs::canonicalize(path).unwrap()
}

pub(crate) fn assert_events_contain_started(events: &[Event], expected_index: &[usize]) {
    assert!(events.iter().any(|event| {
        matches!(event, Event::Started { index } if index.as_slice() == expected_index)
    }));
}

pub(crate) fn assert_events_contain_resolved(events: &[Event], expected_index: &[usize]) {
    assert!(events.iter().any(|event| {
        matches!(event, Event::Resolved { index } if index.as_slice() == expected_index)
    }));
}

pub(crate) fn assert_events_contain_no_errors(events: &[Event]) {
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, Event::Error { .. }))
    );
}

pub(crate) fn assert_events_contain_done_last(events: &[Event]) {
    assert!(matches!(events.last(), Some(Event::Done)));
}

pub(crate) async fn resolve(inputs: impl IntoIterator<Item = Input>) -> (Vec<Manifest>, Vec<Event>) {
    let (sender, mut receiver) = mpsc::channel(1);
    let resolver = Resolver::new(sender);

    let produce = async {
        let manifests = resolver.resolve(inputs).await;
        drop(resolver);
        manifests
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
