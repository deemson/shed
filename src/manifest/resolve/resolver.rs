use std::path::PathBuf;

use futures::future::join_all;
use tokio::sync::mpsc;

use super::event::Event;
use super::manifest::Manifest;
use crate::manifest::ManifestFile;

pub struct Input {
    pub name: String,
    pub path: PathBuf,
    pub manifest_file: ManifestFile,
}

#[derive(Clone)]
pub struct Active {
    pub name: String,
    pub path: PathBuf,
}

pub struct Resolver {
    event_sender: mpsc::Sender<Event>,
}

impl Resolver {
    pub async fn resolve(&self, inputs: impl IntoIterator<Item = Input>) -> Vec<Manifest> {
        let manifests = self.resolve_manifest_files(Vec::new(), inputs).await;
        self.send_event(Event::Done).await;
        manifests
    }

    async fn resolve_manifest_files(
        &self,
        active: Vec<Active>,
        inputs: impl IntoIterator<Item = Input>,
    ) -> Vec<Manifest> {
        join_all(
            inputs
                .into_iter()
                .map(|input| self.resolve_manifest_file(active.clone(), input)),
        )
        .await
    }

    async fn resolve_manifest_file(&self, active: Vec<Active>, input: Input) -> Manifest {
        self.send_event(Event::Started).await;

        let content = match tokio::fs::read_to_string(&input.path).await {
            Ok(content) => content,
            Err(source) => {
                let _ = source;
                return self.error_resolution(input).await;
            }
        };

        let manifest_file = match ManifestFile::try_from(content) {
            Ok(manifest_file) => manifest_file,
            Err(source) => {
                let _ = source;
                return self.error_resolution(input).await;
            }
        };

        let mut active = active;
        active.push(Active {
            name: input.name.clone(),
            path: input.path.clone(),
        });


        self.send_event(Event::Resolved).await;
        Manifest {
            name: input.name,
            path: input.path,
            manifests: Vec::new(),
            items: manifest_file.items,
        }
    }

    async fn error_resolution(&self, input: Input) -> Manifest {
        self.send_event(Event::Error).await;
        Manifest {
            name: input.name,
            path: input.path,
            manifests: Vec::new(),
            items: Vec::new(),
        }
    }

    async fn send_event(&self, event: Event) {
        let _ = self.event_sender.send(event).await;
    }
}
