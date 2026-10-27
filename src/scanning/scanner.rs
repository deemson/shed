use super::{error::*, event::*, index::*, model::*};
use crate::traversal;
use futures::future::join_all;
use std::path::Path;
use tokio::sync::mpsc;

pub struct Scanner {
    event_sender: mpsc::Sender<Event>,
}

impl Scanner {
    pub fn new(event_sender: mpsc::Sender<Event>) -> Self {
        Self { event_sender }
    }

    pub async fn scan(
        &self,
        traversal_roots: impl IntoIterator<Item = traversal::Node>,
    ) -> Vec<Node> {
        let roots = self.scan_nodes(traversal_roots).await;
        self.send_event(Event::Done).await;
        roots
    }

    pub async fn scan_nodes(
        &self,
        traversal_nodes: impl IntoIterator<Item = traversal::Node>,
    ) -> Vec<Node> {
        let nodes = join_all(traversal_nodes.into_iter().map(|node| self.scan_node(node))).await;
        nodes.into_iter().flatten().collect()
    }

    pub async fn scan_node(&self, node: traversal::Node) -> Option<Node> {
        match node {
            traversal::Node::Directory { dst, nodes } => Some(Node::EnsureDirectory {
                dst,
                nodes: self.scan_nodes(nodes).await,
            }),
            traversal::Node::Leaf { index, src, dst } => {
                self.send_event(Event::NodeScanningStarted {
                    index: index.clone(),
                })
                .await;
                let metadata = match tokio::fs::symlink_metadata(&src).await {
                    Ok(metadata) => metadata,
                    Err(source) => {
                        self.send_event(Event::NodeScanningFinished {
                            index,
                            error: Some(Error::MetadataIo { path: src, source }),
                        })
                        .await;
                        return None;
                    }
                };
                if metadata.is_file() {
                    let node = Some(Node::ReplaceFile {
                        index: index.clone(),
                        src,
                        dst,
                    });
                    self.send_event(Event::NodeScanningFinished { index, error: None })
                        .await;
                    return node;
                }
                if metadata.is_dir() {
                    let entries = match self
                        .scan_entries(
                            IndexRef {
                                manifest: &index.manifest,
                                item: &index.item,
                                entry: &[],
                            },
                            &src,
                            &dst,
                        )
                        .await
                    {
                        Ok(entries) => entries,
                        Err(error) => {
                            self.send_event(Event::NodeScanningFinished {
                                index,
                                error: Some(error),
                            })
                            .await;
                            return None;
                        }
                    };
                    let node = Some(Node::ReplaceDirectory {
                        index: index.clone(),
                        src,
                        dst,
                        entries,
                    });
                    self.send_event(Event::NodeScanningFinished { index, error: None })
                        .await;
                    return node;
                }
                self.send_event(Event::NodeScanningFinished {
                    index,
                    error: Some(Error::UnsupportedFileType { path: src }),
                })
                .await;
                None
            }
        }
    }

    pub async fn scan_entries(
        &self,
        index: IndexRef<'_>,
        src: &Path,
        dst: &Path,
    ) -> Result<Vec<Entry>, Error> {
        let mut read_dir = match tokio::fs::read_dir(src).await {
            Ok(read_dir) => read_dir,
            Err(source) => {
                return Err(Error::ReadDirIo {
                    path: src.to_path_buf(),
                    source,
                });
            }
        };

        let mut child_srcs = Vec::new();
        while let Some(result) = read_dir.next_entry().await.transpose() {
            match result {
                Ok(entry) => child_srcs.push(entry.path()),
                Err(source) => {
                    return Err(Error::ReadDirIo {
                        path: src.to_path_buf(),
                        source,
                    });
                }
            }
        }
        child_srcs.sort();

        let entries = join_all(child_srcs.iter().enumerate().map(|(position, child_src)| {
            let mut entry_index = index.entry.to_vec();
            entry_index.push(position);
            async {
                let filename = child_src
                    .file_name()
                    .expect("a directory entry has a filename");
                let child_dst = dst.join(filename);
                self.scan_entry(
                    Index {
                        manifest: index.manifest.to_vec(),
                        item: index.item.to_vec(),
                        entry: entry_index,
                    },
                    child_src,
                    &child_dst,
                )
                .await
            }
        }))
        .await;

        Ok(entries)
    }

    pub async fn scan_entry(&self, index: Index, src: &Path, dst: &Path) -> Entry {
        self.send_event(Event::EntryScanningStarted {
            index: index.to_owned(),
        })
        .await;
        let metadata = match tokio::fs::symlink_metadata(src).await {
            Ok(metadata) => metadata,
            Err(source) => {
                self.send_event(Event::EntryScanningFinished {
                    index: index.clone(),
                    error: Some(Error::MetadataIo {
                        path: src.to_path_buf(),
                        source,
                    }),
                })
                .await;
                return Entry::Unknown {
                    src: src.to_path_buf(),
                    dst: dst.to_path_buf(),
                };
            }
        };
        if metadata.is_dir() {
            let entries = match self.scan_entries(index.as_ref(), src, dst).await {
                Ok(entries) => entries,
                Err(error) => {
                    self.send_event(Event::EntryScanningFinished {
                        index: index.clone(),
                        error: Some(error),
                    })
                    .await;
                    return Entry::Unknown {
                        src: src.to_path_buf(),
                        dst: dst.to_path_buf(),
                    };
                }
            };
            let entry = Entry::Directory {
                dst: dst.to_path_buf(),
                entries,
            };
            self.send_event(Event::EntryScanningFinished {
                index: index.to_owned(),
                error: None,
            })
            .await;
            return entry;
        }
        if metadata.is_file() {
            let entry = Entry::File {
                src: src.to_path_buf(),
                dst: dst.to_path_buf(),
            };
            self.send_event(Event::EntryScanningFinished {
                index: index.to_owned(),
                error: None,
            })
            .await;
            return entry;
        }
        self.send_event(Event::EntryScanningFinished {
            index: index.clone(),
            error: Some(Error::UnsupportedFileType {
                path: src.to_path_buf(),
            }),
        })
        .await;
        Entry::Unknown {
            src: src.to_path_buf(),
            dst: dst.to_path_buf(),
        }
    }

    async fn send_event(&self, event: Event) {
        let _ = self.event_sender.send(event).await;
    }
}

#[cfg(test)]
mod tests {

    use crate::manifest::testing as testing_m;

    use super::{super::testing as testing_s, *};
    use std::{fs, os::unix::fs::PermissionsExt};
    use tempfile::TempDir;

    #[tokio::test]
    async fn scans_mixed_roots_and_nested_directory_contents() {
        let temp_dir = TempDir::new().unwrap();
        let src_dir = temp_dir.path().join("src");
        let dst_dir = temp_dir.path().join("dst");

        for path in [
            "dir1/dir11/dir111/file1111",
            "dir1/dir11/dir111/file1112",
            "dir1/dir11/file111",
            "dir1/dir11/file112",
            "dir1/file11",
            "dir1/file12",
            "dir1/dir12/dir121/file1211",
            "dir1/dir12/dir121/file1212",
            "dir1/dir12/dir122/file1221",
            "dir1/dir12/dir122/file1222",
            "dir1/dir12/file121",
            "dir1/dir12/file122",
            "dir2/dir21/file211",
            "dir2/dir21/file212",
            "dir2/dir22/file221",
            "dir2/dir22/file222",
            "dir2/file21",
            "dir2/file22",
            "file1",
            "file2",
        ] {
            testing_m::write(&src_dir.join(path), path);
        }

        let (actual, events) = testing_s::scan([
            traversal::Node::Directory {
                dst: dst_dir.join("dir1"),
                nodes: vec![
                    traversal::Node::Directory {
                        dst: dst_dir.join("dir1/dir11"),
                        nodes: vec![
                            traversal::Node::Leaf {
                                index: ([0], [0, 0, 0]).into(),
                                src: src_dir.join("dir1/dir11/dir111"),
                                dst: dst_dir.join("dir1/dir11/dir111"),
                            },
                            traversal::Node::Leaf {
                                index: ([0], [0, 0, 1]).into(),
                                src: src_dir.join("dir1/dir11/file111"),
                                dst: dst_dir.join("dir1/dir11/file111"),
                            },
                            traversal::Node::Leaf {
                                index: ([0], [0, 0, 2]).into(),
                                src: src_dir.join("dir1/dir11/file112"),
                                dst: dst_dir.join("dir1/dir11/file112"),
                            },
                        ],
                    },
                    traversal::Node::Leaf {
                        index: ([0], [0, 1]).into(),
                        src: src_dir.join("dir1/dir12"),
                        dst: dst_dir.join("dir1/dir12"),
                    },
                    traversal::Node::Leaf {
                        index: ([0], [0, 2]).into(),
                        src: src_dir.join("dir1/file11"),
                        dst: dst_dir.join("dir1/file11"),
                    },
                    traversal::Node::Leaf {
                        index: ([0], [0, 3]).into(),
                        src: src_dir.join("dir1/file12"),
                        dst: dst_dir.join("dir1/file12"),
                    },
                ],
            },
            traversal::Node::Leaf {
                index: ([0], [1]).into(),
                src: src_dir.join("dir2"),
                dst: dst_dir.join("dir2"),
            },
            traversal::Node::Leaf {
                index: ([0], [2]).into(),
                src: src_dir.join("file1"),
                dst: dst_dir.join("file1"),
            },
            traversal::Node::Leaf {
                index: ([0], [3]).into(),
                src: src_dir.join("file2"),
                dst: dst_dir.join("file2"),
            },
        ])
        .await;

        testing_s::assert_events_contain_no_errors(&events);
        testing_s::assert_events_contain_done_last(&events);

        let expected = vec![
            Node::EnsureDirectory {
                dst: dst_dir.join("dir1"),
                nodes: vec![
                    Node::EnsureDirectory {
                        dst: dst_dir.join("dir1/dir11"),
                        nodes: vec![
                            Node::ReplaceDirectory {
                                index: ([0], [0, 0, 0]).into(),
                                src: src_dir.join("dir1/dir11/dir111"),
                                dst: dst_dir.join("dir1/dir11/dir111"),
                                entries: vec![
                                    Entry::File {
                                        src: src_dir.join("dir1/dir11/dir111/file1111"),
                                        dst: dst_dir.join("dir1/dir11/dir111/file1111"),
                                    },
                                    Entry::File {
                                        src: src_dir.join("dir1/dir11/dir111/file1112"),
                                        dst: dst_dir.join("dir1/dir11/dir111/file1112"),
                                    },
                                ],
                            },
                            Node::ReplaceFile {
                                index: ([0], [0, 0, 1]).into(),
                                src: src_dir.join("dir1/dir11/file111"),
                                dst: dst_dir.join("dir1/dir11/file111"),
                            },
                            Node::ReplaceFile {
                                index: ([0], [0, 0, 2]).into(),
                                src: src_dir.join("dir1/dir11/file112"),
                                dst: dst_dir.join("dir1/dir11/file112"),
                            },
                        ],
                    },
                    Node::ReplaceDirectory {
                        index: ([0], [0, 1]).into(),
                        src: src_dir.join("dir1/dir12"),
                        dst: dst_dir.join("dir1/dir12"),
                        entries: vec![
                            Entry::Directory {
                                dst: dst_dir.join("dir1/dir12/dir121"),
                                entries: vec![
                                    Entry::File {
                                        src: src_dir.join("dir1/dir12/dir121/file1211"),
                                        dst: dst_dir.join("dir1/dir12/dir121/file1211"),
                                    },
                                    Entry::File {
                                        src: src_dir.join("dir1/dir12/dir121/file1212"),
                                        dst: dst_dir.join("dir1/dir12/dir121/file1212"),
                                    },
                                ],
                            },
                            Entry::Directory {
                                dst: dst_dir.join("dir1/dir12/dir122"),
                                entries: vec![
                                    Entry::File {
                                        src: src_dir.join("dir1/dir12/dir122/file1221"),
                                        dst: dst_dir.join("dir1/dir12/dir122/file1221"),
                                    },
                                    Entry::File {
                                        src: src_dir.join("dir1/dir12/dir122/file1222"),
                                        dst: dst_dir.join("dir1/dir12/dir122/file1222"),
                                    },
                                ],
                            },
                            Entry::File {
                                src: src_dir.join("dir1/dir12/file121"),
                                dst: dst_dir.join("dir1/dir12/file121"),
                            },
                            Entry::File {
                                src: src_dir.join("dir1/dir12/file122"),
                                dst: dst_dir.join("dir1/dir12/file122"),
                            },
                        ],
                    },
                    Node::ReplaceFile {
                        index: ([0], [0, 2]).into(),
                        src: src_dir.join("dir1/file11"),
                        dst: dst_dir.join("dir1/file11"),
                    },
                    Node::ReplaceFile {
                        index: ([0], [0, 3]).into(),
                        src: src_dir.join("dir1/file12"),
                        dst: dst_dir.join("dir1/file12"),
                    },
                ],
            },
            Node::ReplaceDirectory {
                index: ([0], [1]).into(),
                src: src_dir.join("dir2"),
                dst: dst_dir.join("dir2"),
                entries: vec![
                    Entry::Directory {
                        dst: dst_dir.join("dir2/dir21"),
                        entries: vec![
                            Entry::File {
                                src: src_dir.join("dir2/dir21/file211"),
                                dst: dst_dir.join("dir2/dir21/file211"),
                            },
                            Entry::File {
                                src: src_dir.join("dir2/dir21/file212"),
                                dst: dst_dir.join("dir2/dir21/file212"),
                            },
                        ],
                    },
                    Entry::Directory {
                        dst: dst_dir.join("dir2/dir22"),
                        entries: vec![
                            Entry::File {
                                src: src_dir.join("dir2/dir22/file221"),
                                dst: dst_dir.join("dir2/dir22/file221"),
                            },
                            Entry::File {
                                src: src_dir.join("dir2/dir22/file222"),
                                dst: dst_dir.join("dir2/dir22/file222"),
                            },
                        ],
                    },
                    Entry::File {
                        src: src_dir.join("dir2/file21"),
                        dst: dst_dir.join("dir2/file21"),
                    },
                    Entry::File {
                        src: src_dir.join("dir2/file22"),
                        dst: dst_dir.join("dir2/file22"),
                    },
                ],
            },
            Node::ReplaceFile {
                index: ([0], [2]).into(),
                src: src_dir.join("file1"),
                dst: dst_dir.join("file1"),
            },
            Node::ReplaceFile {
                index: ([0], [3]).into(),
                src: src_dir.join("file2"),
                dst: dst_dir.join("file2"),
            },
        ];

        assert_eq!(actual, expected);
    }

    #[tokio::test]
    async fn scans_a_directory_item_with_an_unreadable_second_entry() {
        let temp_dir = TempDir::new().unwrap();
        let src_dir = temp_dir.path().join("src");
        let dst_dir = temp_dir.path().join("dst");

        for path in ["dir/a", "dir/b/file", "dir/c"] {
            testing_m::write(&src_dir.join(path), path);
        }
        let unreadable = src_dir.join("dir/b");
        fs::set_permissions(&unreadable, fs::Permissions::from_mode(0o000)).unwrap();

        let (actual, events) = testing_s::scan([traversal::Node::Leaf {
            index: ([0], [0]).into(),
            src: src_dir.join("dir"),
            dst: dst_dir.join("dir"),
        }])
        .await;

        fs::set_permissions(&unreadable, fs::Permissions::from_mode(0o755)).unwrap();

        testing_s::assert_events_contain_done_last(&events);
        assert!(events.iter().any(|event| matches!(
            event,
            Event::EntryScanningFinished {
                index,
                error: Some(Error::ReadDirIo { .. }),
            } if index.entry == [1]
        )));

        let expected = vec![Node::ReplaceDirectory {
            index: ([0], [0]).into(),
            src: src_dir.join("dir"),
            dst: dst_dir.join("dir"),
            entries: vec![
                Entry::File {
                    src: src_dir.join("dir/a"),
                    dst: dst_dir.join("dir/a"),
                },
                Entry::Unknown {
                    src: src_dir.join("dir/b"),
                    dst: dst_dir.join("dir/b"),
                },
                Entry::File {
                    src: src_dir.join("dir/c"),
                    dst: dst_dir.join("dir/c"),
                },
            ],
        }];

        assert_eq!(actual, expected);
    }
}
