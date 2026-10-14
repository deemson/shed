use super::model::*;
use crate::manifest;

impl Report {
    pub fn resolution(&mut self, events: impl IntoIterator<Item = manifest::Event>) {
        for event in events {
            self.resolution_event(event);
        }
    }

    fn resolution_event(&mut self, event: manifest::Event) {
        match event {
            manifest::Event::ResolutionStarted { index } => {
                let manifest = self.get_manifest_mut(&index);
                manifest.resolution_started();
            }
            manifest::Event::ResolutionFinished { index, error } => {
                let manifest = self.get_manifest_mut(&index);
                manifest.resolution_finished(error);
            }
            manifest::Event::Done => (),
        }
    }
}

impl Manifest {
    fn resolution_started(&mut self) {
        match self.status {
            ManifestStatus::Initial => self.status = ManifestStatus::ResolutionStarted,
            ManifestStatus::ResolutionStarted => panic!("resolution already started"),
            ManifestStatus::ResolutionFinished => panic!("resolution already finished"),
        }
    }
    fn resolution_finished(&mut self, error: Option<manifest::Error>) {
        match self.status {
            ManifestStatus::Initial => panic!("resolution did not start"),
            ManifestStatus::ResolutionStarted => self.status = ManifestStatus::ResolutionFinished,
            ManifestStatus::ResolutionFinished => panic!("resolution already finished"),
        }
        self.error = error.map(|error| error.to_string())
    }
}
