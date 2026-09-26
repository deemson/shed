use std::{
    io,
    path::{Path, PathBuf},
};

use futures::future::join_all;
use tokio::sync::mpsc;

use super::error::Error;
use super::event::Event;
use super::model::Manifest;
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
    pub fn new(event_sender: mpsc::Sender<Event>) -> Self {
        Self { event_sender }
    }

    pub async fn resolve(&self, inputs: impl IntoIterator<Item = Input>) -> Vec<Manifest> {
        let manifests = self
            .resolve_manifest_files(Vec::new(), Vec::new(), inputs)
            .await;
        self.send_event(Event::Done).await;
        manifests
    }

    async fn resolve_manifest_files(
        &self,
        input_stack: Vec<Input>,
        index: Vec<usize>,
        inputs: impl IntoIterator<Item = Input>,
    ) -> Vec<Manifest> {
        join_all(inputs.into_iter().enumerate().map(|(position, input)| {
            let mut child_index = index.clone();
            child_index.push(position);
            self.resolve_manifest_file(input_stack.clone(), child_index, input)
        }))
        .await
    }

    async fn resolve_manifest_file(
        &self,
        input_stack: Vec<Input>,
        index: Vec<usize>,
        input: Input,
    ) -> Manifest {
        self.send_event(Event::Started {
            index: index.clone(),
        })
        .await;

        if let Some(position) = input_stack
            .iter()
            .position(|active_input| active_input.path == input.path)
        {
            let start_index = index[..=position].to_vec();
            return self
                .error_resolution(index, input, Error::Cycle { start_index })
                .await;
        }

        let content = match tokio::fs::read_to_string(&input.path).await {
            Ok(content) => content,
            Err(source) => {
                return self
                    .error_resolution(index, input, Error::Io { source })
                    .await;
            }
        };

        let manifest_file = match ManifestFile::try_from(content) {
            Ok(manifest_file) => manifest_file,
            Err(source) => {
                return self
                    .error_resolution(index, input, Error::Yaml { source })
                    .await;
            }
        };

        let mut input_stack = input_stack;
        input_stack.push(input.clone());

        let manifests = join_all(manifest_file.include.into_iter().enumerate().map(
            |(position, name)| {
                let active = input_stack.clone();
                let path = &input.path;
                let mut child_index = index.clone();
                child_index.push(position);
                async move {
                    self.resolve_manifest_file_include(active, child_index, path, &name)
                        .await
                }
            },
        ))
        .await;

        self.send_event(Event::Resolved { index }).await;
        Manifest {
            name: input.name,
            path: input.path,
            manifests,
            items: manifest_file.items,
        }
    }

    async fn resolve_manifest_file_include(
        &self,
        input_stack: Vec<Input>,
        index: Vec<usize>,
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
                            .error_resolution(
                                index,
                                Input {
                                    name: name.into(),
                                    path: yml,
                                },
                                Error::NotFound,
                            )
                            .await;
                    }
                    Err(source) => {
                        return self
                            .error_resolution(
                                index,
                                Input {
                                    name: name.into(),
                                    path: yml,
                                },
                                Error::Io { source },
                            )
                            .await;
                    }
                }
            }
            Err(source) => {
                return self
                    .error_resolution(
                        index,
                        Input {
                            name: name.into(),
                            path: yaml,
                        },
                        Error::Io { source },
                    )
                    .await;
            }
        };

        self.resolve_manifest_file(
            input_stack,
            index,
            Input {
                name: name.into(),
                path,
            },
        )
        .await
    }

    async fn error_resolution(&self, index: Vec<usize>, input: Input, error: Error) -> Manifest {
        self.send_event(Event::Error { index, error }).await;
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
    use crate::manifest::{RootItem, resolve::testing::*};

    use super::*;

    use indoc::indoc;
    use tempfile::TempDir;

    #[tokio::test]
    async fn resolves_two_simple_manifests() {
        let temp_dir = TempDir::new().unwrap();

        let parent_path = write_yaml_manifest(
            temp_dir.path(),
            "parent",
            indoc! {"
              include:
                - child
              items:
                - path: parent-path
                  shed: parent-shed
            "},
        );

        let child_path = write_yaml_manifest(
            temp_dir.path(),
            "child",
            indoc! {"
              items:
                - path: child-path
                  shed: child-shed
            "},
        );

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
                    shed: Some(String::from("child-shed")),
                    items: None,
                }],
            }],
            items: vec![RootItem {
                path: String::from("parent-path"),
                shed: Some(String::from("parent-shed")),
                items: None,
            }],
        }];

        assert_events_contain_started(&events, &[0]);
        assert_events_contain_started(&events, &[0, 0]);
        assert_events_contain_resolved(&events, &[0]);
        assert_events_contain_resolved(&events, &[0, 0]);
        assert_events_contain_no_errors(&events);
        assert_events_contain_done_last(&events);

        assert_eq!(actual, expected)
    }

    #[tokio::test]
    async fn reports_the_start_and_end_of_a_cycle() {
        let temp_dir = TempDir::new().unwrap();

        let m1_path = write_yaml_manifest(
            temp_dir.path(),
            "m1",
            indoc! {"
              include:
                - m2
            "},
        );

        let m2_path = write_yaml_manifest(
            temp_dir.path(),
            "m2",
            indoc! {"
              include:
                - m3
            "},
        );

        let m3_path = write_yaml_manifest(
            temp_dir.path(),
            "m3",
            indoc! {"
              include:
                - m1
            "},
        );

        let (actual, events) = resolve(vec![Input {
            name: String::from("m1"),
            path: m1_path.clone(),
        }])
        .await;

        let expected = vec![Manifest {
            name: String::from("m1"),
            path: m1_path.clone(),
            manifests: vec![Manifest {
                name: String::from("m2"),
                path: m2_path,
                manifests: vec![Manifest {
                    name: String::from("m3"),
                    path: m3_path,
                    manifests: vec![Manifest {
                        name: String::from("m1"),
                        path: m1_path,
                        manifests: Vec::new(),
                        items: Vec::new(),
                    }],
                    items: Vec::new(),
                }],
                items: Vec::new(),
            }],
            items: Vec::new(),
        }];

        assert!(events.iter().any(|event| matches!(
            event,
            Event::Error {
                index,
                error: Error::Cycle { start_index },
            } if index.as_slice() == [0, 0, 0, 0] && start_index.as_slice() == [0]
        )));
        assert_events_contain_done_last(&events);

        assert_eq!(actual, expected)
    }
}
