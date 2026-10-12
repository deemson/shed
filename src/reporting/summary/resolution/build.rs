use super::model::*;
use crate::manifest;

pub fn build(
    manifests: &[manifest::Manifest],
    events: impl IntoIterator<Item = manifest::Event>,
) -> Report {
    let mut report: Report = manifests.into();
    for event in events {
        if report.is_done {
            panic!("report is already done")
        }
        match event {
            manifest::Event::ResolutionStarted { index } => {
                let manifest = report.get_mut(&index);
                match manifest.state {
                    ManifestState::Initial => manifest.state = ManifestState::Started,
                    ManifestState::Started => panic!("manifest already started"),
                    ManifestState::Finished => panic!("manifest already finished"),
                };
            }
            manifest::Event::ResolutionFinished { index, error } => {
                let has_error = error.is_some();
                let manifest = report.get_mut(&index);
                match manifest.state {
                    ManifestState::Initial => panic!("manifest have not been started"),
                    ManifestState::Started => {
                        manifest.state = ManifestState::Finished;
                        manifest.error = error;
                    }
                    ManifestState::Finished => panic!("manifest already finished"),
                };
                report.has_errors |= has_error;
            }
            manifest::Event::Done => {
                report.is_done = true;
            }
        }
    }
    report
}
