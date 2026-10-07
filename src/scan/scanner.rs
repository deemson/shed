use super::{error::*, event::*, model::*};
use crate::{manifest::FullIndex, traversal};
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
                let metadata = match tokio::fs::symlink_metadata(&src).await {
                    Ok(metadata) => metadata,
                    Err(source) => {
                        self.send_event(Event::Error {
                            index,
                            error: Error::MetadataIo { path: src, source },
                        })
                        .await;
                        return None;
                    }
                };
                if metadata.is_file() {
                    return Some(Node::ReplaceFile { index, src, dst });
                }
                if metadata.is_dir() {
                    let entries = self.scan_entries(&index, &src, &dst).await?;
                    return Some(Node::ReplaceDirectory {
                        index,
                        src,
                        dst,
                        entries,
                    });
                }
                self.send_event(Event::Error {
                    index,
                    error: Error::UnsupportedFileType {
                        path: src,
                    },
                })
                .await;
                None
            }
        }
    }

    pub async fn scan_entries(
        &self,
        index: &FullIndex,
        src: &Path,
        dst: &Path,
    ) -> Option<Vec<Entry>> {
        let mut read_dir = match tokio::fs::read_dir(src).await {
            Ok(read_dir) => read_dir,
            Err(source) => {
                self.send_event(Event::Error {
                    index: index.clone(),
                    error: Error::ReadDirIo {
                        path: src.to_path_buf(),
                        source,
                    },
                })
                .await;
                return None;
            }
        };

        let mut child_srcs = Vec::new();
        while let Some(result) = read_dir.next_entry().await.transpose() {
            match result {
                Ok(entry) => child_srcs.push(entry.path()),
                Err(source) => {
                    self.send_event(Event::Error {
                        index: index.clone(),
                        error: Error::ReadDirIo {
                            path: src.to_path_buf(),
                            source,
                        },
                    })
                    .await;
                    return None;
                }
            }
        }
        child_srcs.sort();

        let entries = join_all(child_srcs.iter().map(|child_src| async {
            let filename = child_src
                .file_name()
                .expect("a directory entry has a filename");
            let child_dst = dst.join(filename);
            self.scan_entry(index, child_src, &child_dst).await
        }))
        .await;

        Some(entries.into_iter().flatten().collect())
    }

    pub async fn scan_entry(&self, index: &FullIndex, src: &Path, dst: &Path) -> Option<Entry> {
        let metadata = match tokio::fs::symlink_metadata(src).await {
            Ok(metadata) => metadata,
            Err(source) => {
                self.send_event(Event::Error {
                    index: index.clone(),
                    error: Error::MetadataIo {
                        path: src.to_path_buf(),
                        source,
                    },
                })
                .await;
                return None;
            }
        };
        if metadata.is_dir() {
            let entries = self.scan_entries(index, src, dst).await?;
            return Some(Entry::Directory {
                dst: dst.to_path_buf(),
                entries,
            });
        }
        if metadata.is_file() {
            return Some(Entry::File {
                src: src.to_path_buf(),
                dst: dst.to_path_buf(),
            });
        }
        self.send_event(Event::Error {
            index: index.clone(),
            error: Error::UnsupportedFileType {
                path: src.to_path_buf(),
            },
        })
        .await;
        None
    }

    async fn send_event(&self, event: Event) {
        let _ = self.event_sender.send(event).await;
    }
}

#[cfg(test)]
mod tests {

    use crate::manifest::testing as testing_m;

    use super::{super::testing as testing_s, *};
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
}
