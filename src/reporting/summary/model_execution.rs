use super::model::*;
use crate::execution;

impl Report {
    pub fn execution(&mut self, events: impl IntoIterator<Item = execution::Event>) {
        assert_eq!(self.status, ReportStatus::Scanned);
        self.status = ReportStatus::Executing;
        for event in events {
            self.execution_event(event);
        }
    }
    fn execution_event(&mut self, event: execution::Event) {
        match event {
            execution::Event::EnsureStarted { .. } => (),
            execution::Event::EnsureFinished { error, .. } => {
                let Some(error) = error else { return };
                let execution::Error::EnsureDirectoryIo { affected, .. } = &error else {
                    unreachable!("ensure can only fail with EnsureDirectoryIo, got {error:?}");
                };
                let message = error.to_string();
                for index in affected {
                    let item = self.get_item_mut(index.as_ref());
                    item.error = Some(message.clone());
                }
            }
            execution::Event::ReplaceStarted { index, .. } => {
                let item = self.get_item_mut(index.as_ref());
                item.execution_started();
            }
            execution::Event::ReplaceFinished { index, error, .. } => {
                let item = self.get_item_mut(index.as_ref());
                item.execution_finished(error);
            }
            execution::Event::EntryStarted { index, .. } => {
                let entry = self.get_entry_mut(index.as_ref());
                entry.execution_started();
            }
            execution::Event::EntryFinished { index, error, .. } => {
                let entry = self.get_entry_mut(index.as_ref());
                entry.execution_finished(error);
            }
            execution::Event::Done => self.status = ReportStatus::Executed,
        }
    }
}

impl Item {
    fn execution_started(&mut self) {
        assert_eq!(self.status, ItemStatus::Scanned);
        self.status = ItemStatus::Executing;
    }
    fn execution_finished(&mut self, error: Option<execution::Error>) {
        assert_eq!(self.status, ItemStatus::Executing);
        self.error = error.map(|error| error.to_string());
        self.status = ItemStatus::Executed;
    }
}

impl Entry {
    fn execution_started(&mut self) {
        assert_eq!(self.status, EntryStatus::Scanned);
        self.status = EntryStatus::Executing;
    }
    fn execution_finished(&mut self, error: Option<execution::Error>) {
        assert_eq!(self.status, EntryStatus::Executing);
        self.error = error.map(|error| error.to_string());
        self.status = EntryStatus::Executed;
    }
}
