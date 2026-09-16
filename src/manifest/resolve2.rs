use std::io;
use std::path::{Path, PathBuf};

use futures::FutureExt;
use futures::future::{BoxFuture, join_all};
use tokio::sync::mpsc;

use super::serde::{ManifestFile, RootItem};

#[derive(Debug, PartialEq)]
pub struct Manifest {
    pub name: String,
    pub path: PathBuf,
    pub manifests: Vec<Manifest>,
    pub items: Vec<RootItem>,
}

#[derive(Debug)]
pub enum Event {
    Started { name: String },
    Resolved { name: String, path: PathBuf },
    Error { name: String, error: Error },
    Finished { resolved: usize, errors: usize },
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("manifest was not found; tried {attempted:?}")]
    NotFound { attempted: [PathBuf; 2] },

    #[error("failed to access manifest {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("failed to parse manifest {path}: {source}")]
    Yaml {
        path: PathBuf,
        #[source]
        source: yaml_serde::Error,
    },

    #[error("include cycle detected: {chain:?}")]
    Cycle { chain: Vec<PathBuf> },
}

#[derive(Debug, PartialEq)]
pub struct Outcome {
    pub manifest: Option<Manifest>,
    pub resolved: usize,
    pub errors: usize,
}

struct NodeOutcome {
    manifest: Option<Manifest>,
    resolved: usize,
    errors: usize,
}

pub async fn resolve(name: impl Into<String>, events: mpsc::Sender<Event>) -> Outcome {
    let outcome = resolve_manifest(name.into(), None, Vec::new(), events.clone()).await;
    let outcome = Outcome {
        manifest: outcome.manifest,
        resolved: outcome.resolved,
        errors: outcome.errors,
    };

    send_event(
        &events,
        Event::Finished {
            resolved: outcome.resolved,
            errors: outcome.errors,
        },
    )
    .await;

    outcome
}

fn resolve_manifest(
    name: String,
    directory: Option<PathBuf>,
    active: Vec<PathBuf>,
    events: mpsc::Sender<Event>,
) -> BoxFuture<'static, NodeOutcome> {
    async move {
        send_event(&events, Event::Started { name: name.clone() }).await;

        let path = match find_manifest(&name, directory.as_deref()).await {
            Ok(path) => path,
            Err(error) => return failed(name, error, &events).await,
        };

        if let Some(start) = active.iter().position(|active| active == &path) {
            let mut chain = active[start..].to_vec();
            chain.push(path);
            return failed(name, Error::Cycle { chain }, &events).await;
        }

        let source = match tokio::fs::read_to_string(&path).await {
            Ok(source) => source,
            Err(source) => {
                return failed(
                    name,
                    Error::Io {
                        path: path.clone(),
                        source,
                    },
                    &events,
                )
                .await;
            }
        };
        let file = match ManifestFile::try_from(source) {
            Ok(file) => file,
            Err(source) => {
                return failed(
                    name,
                    Error::Yaml {
                        path: path.clone(),
                        source,
                    },
                    &events,
                )
                .await;
            }
        };

        let ManifestFile { include, items } = file;
        let directory = path
            .parent()
            .expect("a canonical manifest path has a parent")
            .to_path_buf();
        let mut descendants = active;
        descendants.push(path.clone());

        let children = join_all(include.into_iter().map(|child| {
            resolve_manifest(
                child,
                Some(directory.clone()),
                descendants.clone(),
                events.clone(),
            )
        }))
        .await;

        let resolved = children.iter().map(|child| child.resolved).sum::<usize>();
        let errors = children.iter().map(|child| child.errors).sum::<usize>();

        if errors > 0 {
            return NodeOutcome {
                manifest: None,
                resolved,
                errors,
            };
        }

        let manifests = children
            .into_iter()
            .map(|child| {
                child
                    .manifest
                    .expect("a child without errors has a manifest")
            })
            .collect();
        let manifest = Manifest {
            name: name.clone(),
            path: path.clone(),
            manifests,
            items,
        };

        send_event(&events, Event::Resolved { name, path }).await;

        NodeOutcome {
            manifest: Some(manifest),
            resolved: resolved + 1,
            errors: 0,
        }
    }
    .boxed()
}

async fn find_manifest(name: &str, directory: Option<&Path>) -> Result<PathBuf, Error> {
    let yaml = candidate(name, "yaml", directory);
    let yml = candidate(name, "yml", directory);

    match tokio::fs::canonicalize(&yaml).await {
        Ok(path) => Ok(path),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            match tokio::fs::canonicalize(&yml).await {
                Ok(path) => Ok(path),
                Err(error) if error.kind() == io::ErrorKind::NotFound => Err(Error::NotFound {
                    attempted: [yaml, yml],
                }),
                Err(source) => Err(Error::Io { path: yml, source }),
            }
        }
        Err(source) => Err(Error::Io { path: yaml, source }),
    }
}

fn candidate(name: &str, extension: &str, directory: Option<&Path>) -> PathBuf {
    let path = PathBuf::from(format!("{name}.{extension}"));
    if path.is_absolute() {
        path
    } else if let Some(directory) = directory {
        directory.join(path)
    } else {
        path
    }
}

async fn failed(name: String, error: Error, events: &mpsc::Sender<Event>) -> NodeOutcome {
    send_event(events, Event::Error { name, error }).await;
    NodeOutcome {
        manifest: None,
        resolved: 0,
        errors: 1,
    }
}

async fn send_event(events: &mpsc::Sender<Event>, event: Event) {
    let _ = events.send(event).await;
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;

    async fn run(name: impl Into<String>) -> (Outcome, Vec<Event>) {
        let (sender, mut receiver) = mpsc::channel(1);
        let resolve = resolve(name, sender);
        let collect = async move {
            let mut events = Vec::new();
            while let Some(event) = receiver.recv().await {
                events.push(event);
            }
            events
        };

        tokio::join!(resolve, collect)
    }

    fn write(path: &Path, source: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, source).unwrap();
    }

    fn logical(path: impl AsRef<Path>) -> String {
        path.as_ref().to_str().unwrap().to_owned()
    }

    #[tokio::test]
    async fn resolves_root_and_includes_in_declaration_order() {
        let temp = TempDir::new().unwrap();
        let root = temp.path().join("root");
        write(
            &root.with_extension("yaml"),
            "include:\n  - nested/first\n  - second\nitems:\n  - path: /root\n    shed: root\n",
        );
        write(
            &temp.path().join("nested/first.yml"),
            "include:\n  - leaf\n",
        );
        write(&temp.path().join("nested/leaf.yaml"), "{}\n");
        write(&temp.path().join("second.yaml"), "{}\n");

        let root_name = logical(&root);
        let (outcome, events) = run(root_name.clone()).await;
        let manifest = outcome.manifest.as_ref().unwrap();

        assert_eq!(manifest.name, root_name);
        assert_eq!(
            manifest.path,
            root.with_extension("yaml").canonicalize().unwrap()
        );
        assert_eq!(
            manifest
                .manifests
                .iter()
                .map(|manifest| manifest.name.as_str())
                .collect::<Vec<_>>(),
            ["nested/first", "second"]
        );
        assert_eq!(manifest.manifests[0].manifests[0].name, "leaf");
        assert_eq!(manifest.items.len(), 1);
        assert_eq!(outcome.resolved, 4);
        assert_eq!(outcome.errors, 0);
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(event, Event::Resolved { .. }))
                .count(),
            4
        );
        assert!(matches!(
            events.last(),
            Some(Event::Finished {
                resolved: 4,
                errors: 0
            })
        ));
    }

    #[tokio::test]
    async fn prefers_yaml_and_falls_back_to_yml_only_when_yaml_is_missing() {
        let temp = TempDir::new().unwrap();
        let preferred = temp.path().join("preferred");
        write(
            &preferred.with_extension("yaml"),
            "items:\n  - path: /yaml\n    shed: yaml\n",
        );
        write(
            &preferred.with_extension("yml"),
            "items:\n  - path: /yml\n    shed: yml\n",
        );

        let (outcome, _) = run(logical(&preferred)).await;
        assert_eq!(outcome.manifest.unwrap().items[0].path, "/yaml");

        let fallback = temp.path().join("fallback");
        write(&fallback.with_extension("yml"), "{}\n");
        let (outcome, _) = run(logical(&fallback)).await;
        assert_eq!(
            outcome.manifest.unwrap().path,
            fallback.with_extension("yml").canonicalize().unwrap()
        );

        let malformed = temp.path().join("malformed");
        write(&malformed.with_extension("yaml"), "not an object\n");
        write(&malformed.with_extension("yml"), "{}\n");
        let (outcome, events) = run(logical(&malformed)).await;
        assert!(outcome.manifest.is_none());
        assert!(events.iter().any(|event| matches!(
            event,
            Event::Error {
                error: Error::Yaml { .. },
                ..
            }
        )));
    }

    #[tokio::test]
    async fn appends_suffixes_even_when_the_logical_name_has_an_extension() {
        let temp = TempDir::new().unwrap();
        let root = temp.path().join("root.yaml");
        write(&PathBuf::from(format!("{}.yml", root.display())), "{}\n");

        let name = logical(&root);
        let (outcome, _) = run(name.clone()).await;

        assert_eq!(outcome.manifest.unwrap().name, name);
    }

    #[tokio::test]
    async fn resolves_absolute_includes_and_keeps_repeated_occurrences() {
        let temp = TempDir::new().unwrap();
        let root = temp.path().join("root");
        let shared = temp.path().join("elsewhere/shared");
        write(&shared.with_extension("yaml"), "{}\n");
        write(
            &root.with_extension("yaml"),
            &format!("include:\n  - {0}\n  - {0}\n", shared.to_str().unwrap()),
        );

        let (outcome, _) = run(logical(&root)).await;
        let manifest = outcome.manifest.unwrap();

        assert_eq!(manifest.manifests.len(), 2);
        assert_eq!(manifest.manifests[0].name, logical(&shared));
        assert_eq!(manifest.manifests[1].name, logical(&shared));
        assert_eq!(manifest.manifests[0].path, manifest.manifests[1].path);
    }

    #[tokio::test]
    async fn detects_cycles_using_canonical_paths() {
        let temp = TempDir::new().unwrap();
        let a = temp.path().join("a");
        write(&a.with_extension("yaml"), "include:\n  - b\n");
        write(&temp.path().join("b.yaml"), "include:\n  - a\n");

        let (outcome, events) = run(logical(&a)).await;

        assert_eq!(outcome.resolved, 0);
        assert_eq!(outcome.errors, 1);
        assert!(outcome.manifest.is_none());
        assert!(events.iter().any(|event| matches!(
            event,
            Event::Error {
                error: Error::Cycle { chain },
                ..
            } if chain.len() == 3 && chain.first() == chain.last()
        )));
    }

    #[tokio::test]
    async fn reports_all_sibling_failures_and_discards_the_partial_tree() {
        let temp = TempDir::new().unwrap();
        let root = temp.path().join("root");
        write(
            &root.with_extension("yaml"),
            "include:\n  - missing-one\n  - valid\n  - missing-two\n",
        );
        write(&temp.path().join("valid.yaml"), "{}\n");

        let (outcome, events) = run(logical(&root)).await;

        assert_eq!(outcome.resolved, 1);
        assert_eq!(outcome.errors, 2);
        assert!(outcome.manifest.is_none());
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(
                    event,
                    Event::Error {
                        error: Error::NotFound { .. },
                        ..
                    }
                ))
                .count(),
            2
        );
        assert!(matches!(
            events.last(),
            Some(Event::Finished {
                resolved: 1,
                errors: 2
            })
        ));
    }

    #[tokio::test]
    async fn a_dropped_event_receiver_does_not_fail_resolution() {
        let temp = TempDir::new().unwrap();
        let root = temp.path().join("root");
        write(&root.with_extension("yaml"), "{}\n");
        let (sender, receiver) = mpsc::channel(1);
        drop(receiver);

        let outcome = resolve(logical(&root), sender).await;

        assert_eq!(outcome.resolved, 1);
        assert_eq!(outcome.errors, 0);
        assert!(outcome.manifest.is_some());
    }
}
