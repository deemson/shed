use std::{
    io,
    path::{Path, PathBuf},
};

use futures::future::join_all;
use tokio::sync::mpsc;

use super::event::Event;
use super::manifest::Manifest;
use crate::manifest::ManifestFile;

#[derive(Clone)]
pub struct Input {
    pub name: String,
    pub path: PathBuf,
}

fn include_input_candidate_path(directory: &Path, name: &str, extension: &str) -> PathBuf {
    let path = PathBuf::from(format!("{name}.{extension}"));
    if path.is_absolute() {
        path
    } else {
        directory.join(path)
    }
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
        active: Vec<Input>,
        inputs: impl IntoIterator<Item = Input>,
    ) -> Vec<Manifest> {
        join_all(
            inputs
                .into_iter()
                .map(|input| self.resolve_manifest_file(active.clone(), input)),
        )
        .await
    }

    async fn resolve_manifest_file(&self, active: Vec<Input>, input: Input) -> Manifest {
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
        active.push(input.clone());

        let manifests = join_all(manifest_file.include.into_iter().map(|name| {
            let active = active.clone();
            let path = &input.path;
            async move {
                self.resolve_manifest_file_include(active, path, &name)
                    .await
            }
        }))
        .await;

        self.send_event(Event::Resolved).await;
        Manifest {
            name: input.name,
            path: input.path,
            manifests,
            items: manifest_file.items,
        }
    }

    async fn resolve_manifest_file_include(
        &self,
        active: Vec<Input>,
        path: &Path,
        name: &str,
    ) -> Manifest {
        let directory = path
            .parent()
            .expect("a canonical manifest path has a parent")
            .to_path_buf();

        let yaml = include_input_candidate_path(&directory, name, "yaml");
        let yml = include_input_candidate_path(&directory, name, "yml");

        let path = match tokio::fs::canonicalize(&yaml).await {
            Ok(path) => path,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                match tokio::fs::canonicalize(&yml).await {
                    Ok(path) => path,
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {
                        return self
                            .error_resolution(Input {
                                name: name.into(),
                                path: yml,
                            })
                            .await;
                    }
                    Err(source) => {
                        let _ = source;
                        return self
                            .error_resolution(Input {
                                name: name.into(),
                                path: yml,
                            })
                            .await;
                    }
                }
            }
            Err(source) => {
                let _ = source;
                return self
                    .error_resolution(Input {
                        name: name.into(),
                        path: yaml,
                    })
                    .await;
            }
        };

        self.resolve_manifest_file(
            active,
            Input {
                name: name.into(),
                path,
            },
        )
        .await
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

#[cfg(test)]
mod tests {
    use crate::manifest::RootItem;

    use super::*;

    use std::{fs, path::Path};
    use tempfile::TempDir;

    fn write(path: &Path, source: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, source).unwrap();
    }

    async fn resolve(inputs: impl IntoIterator<Item = Input>) -> (Vec<Manifest>, Vec<Event>) {
        let (sender, mut receiver) = mpsc::channel(1);
        let resolver = Resolver {
            event_sender: sender,
        };
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

    #[tokio::test]
    async fn resolves_two_simple_manifests() {
        let temp_dir = TempDir::new().unwrap();

        let parent_path = temp_dir.path().join("parent").with_extension("yaml");
        #[rustfmt::skip]
        let parent_content = [
            "include:",
            "  - child",
            "items:",
            "  - parent-path"
        ].join("\n");
        write(&parent_path, &parent_content);

        let child_path = temp_dir.path().join("child").with_extension("yaml");
        #[rustfmt::skip]
        let child_content = [
            "items:",
            "  - child-path"
        ].join("\n");
        write(&child_path, &child_content);

        let (actual, events) = resolve(vec![Input {
            name: String::from("parent"),
            path: parent_path.clone(),
        }])
        .await;

        let expected = vec![Manifest {
            name: String::from("parent"),
            path: parent_path,
            manifests: vec![Manifest {
                name: String::from("child"),
                path: child_path,
                manifests: Vec::new(),
                items: vec![RootItem {
                    path: String::from("child-path"),
                    shed: String::from("child-path"),
                    items: None,
                }],
            }],
            items: vec![RootItem {
                path: String::from("parent-path"),
                shed: String::from("parent-path"),
                items: None,
            }],
        }];

        dbg!(&events);
        let _ = events;
        assert_eq!(actual, expected)
    }
}
