use super::{error::*, event::*};
use crate::{
    manifest::Index,
    scanning::{self, Node},
};
use futures::future::join_all;
use std::path::Path;
use tokio::sync::mpsc;

pub struct Executor {
    dry: bool,
    event_sender: mpsc::Sender<Event>,
}

impl Executor {
    pub fn new(dry: bool, event_sender: mpsc::Sender<Event>) -> Self {
        Self { dry, event_sender }
    }

    pub async fn execute(&self, nodes: &[scanning::Node]) {
        self.execute_nodes(nodes).await;
        self.send_event(Event::Done).await;
    }

    async fn execute_nodes(&self, nodes: &[scanning::Node]) {
        join_all(nodes.iter().map(|node| self.execute_node(node))).await;
    }

    async fn execute_node(&self, node: &scanning::Node) {
        match node {
            Node::EnsureDirectory { dst, nodes } => {
                self.send_event(Event::EnsureStarted {
                    dst: dst.to_path_buf(),
                })
                .await;
                if let Err(source) = Self::check_destination_directory(dst).await {
                    self.send_event(Event::EnsureFinished {
                        dst: dst.to_path_buf(),
                        error: Some(Error::EnsureDirectoryIo {
                            source,
                            child_indexes: Self::collect_node_child_indexes(node),
                        }),
                    })
                    .await;

                    return;
                }

                if let Err(source) = self.ensure_destination_directory(dst).await {
                    self.send_event(Event::EnsureFinished {
                        dst: dst.to_path_buf(),
                        error: Some(Error::EnsureDirectoryIo {
                            source,
                            child_indexes: Self::collect_node_child_indexes(node),
                        }),
                    })
                    .await;

                    return;
                }

                self.execute_nodes(nodes).await;
                self.send_event(Event::EnsureFinished {
                    dst: dst.to_path_buf(),
                    error: None,
                })
                .await;
            }
            Node::ReplaceDirectory {
                index,
                src,
                dst,
                entries,
            } => {
                self.send_event(Event::ReplaceStarted {
                    index: index.clone(),
                    src: src.to_path_buf(),
                    dst: dst.to_path_buf(),
                })
                .await;

                let (src_result, dst_result) = tokio::join!(
                    Self::check_source_directory(src),
                    Self::check_destination_directory(dst),
                );

                if let Err(source) = src_result.and(dst_result) {
                    self.send_event(Event::ReplaceFinished {
                        index: index.clone(),
                        src: src.to_path_buf(),
                        dst: dst.to_path_buf(),
                        error: Some(Error::ReplaceIo { source }),
                    })
                    .await;

                    return;
                }

                if let Err(source) = self.replace_destination_directory(dst).await {
                    self.send_event(Event::ReplaceFinished {
                        index: index.clone(),
                        src: src.to_path_buf(),
                        dst: dst.to_path_buf(),
                        error: Some(Error::ReplaceIo { source }),
                    })
                    .await;

                    return;
                }

                self.execute_directory_entries(src, dst, index, entries)
                    .await;
                self.send_event(Event::ReplaceFinished {
                    index: index.clone(),
                    src: src.to_path_buf(),
                    dst: dst.to_path_buf(),
                    error: None,
                })
                .await;
            }
            Node::ReplaceFile { index, src, dst } => {
                self.send_event(Event::ReplaceStarted {
                    index: index.clone(),
                    src: src.to_path_buf(),
                    dst: dst.to_path_buf(),
                })
                .await;

                let (src_result, dst_result) = tokio::join!(
                    Self::check_source_file(src),
                    Self::check_destination_file(dst),
                );

                if let Err(source) = src_result.and(dst_result) {
                    self.send_event(Event::ReplaceFinished {
                        index: index.clone(),
                        src: src.to_path_buf(),
                        dst: dst.to_path_buf(),
                        error: Some(Error::ReplaceIo { source }),
                    })
                    .await;

                    return;
                }

                if index.item.len() == 1 {
                    let dst_parent = dst.parent().expect("file destination must have a parent");
                    if let Err(source) = self.ensure_destination_directory(dst_parent).await {
                        self.send_event(Event::ReplaceFinished {
                            index: index.clone(),
                            src: src.to_path_buf(),
                            dst: dst.to_path_buf(),
                            error: Some(Error::ReplaceIo { source }),
                        })
                        .await;

                        return;
                    }
                }

                if let Err(source) = self.ensure_file(src, dst).await {
                    self.send_event(Event::ReplaceFinished {
                        index: index.clone(),
                        src: src.to_path_buf(),
                        dst: dst.to_path_buf(),
                        error: Some(Error::ReplaceIo { source }),
                    })
                    .await;

                    return;
                }

                self.send_event(Event::ReplaceFinished {
                    index: index.clone(),
                    src: src.to_path_buf(),
                    dst: dst.to_path_buf(),
                    error: None,
                })
                .await;
            }
        }
    }

    async fn execute_directory_entries(
        &self,
        src_parent: &Path,
        dst_parent: &Path,
        index: &Index,
        entries: &[scanning::Entry],
    ) {
        join_all(
            entries
                .iter()
                .map(|entry| self.execute_directory_entry(src_parent, dst_parent, index, entry)),
        )
        .await;
    }

    async fn execute_directory_entry(
        &self,
        src_parent: &Path,
        dst_parent: &Path,
        index: &Index,
        entry: &scanning::Entry,
    ) {
        match entry {
            scanning::Entry::Directory { dst, entries } => {
                let relative = dst
                    .strip_prefix(dst_parent)
                    .expect("scanned entry destination must be inside its parent");
                let src = src_parent.join(relative);

                self.send_event(Event::ReplaceStarted {
                    index: index.clone(),
                    src: src.to_path_buf(),
                    dst: dst.to_path_buf(),
                })
                .await;

                if let Err(source) = self.ensure_destination_directory(dst).await {
                    self.send_event(Event::ReplaceFinished {
                        index: index.clone(),
                        src: src.to_path_buf(),
                        dst: dst.to_path_buf(),
                        error: Some(Error::ReplaceIo { source }),
                    })
                    .await;
                    return;
                }
                self.execute_directory_entries(&src, dst, index, entries)
                    .await;
                self.send_event(Event::ReplaceFinished {
                    index: index.clone(),
                    src: src.to_path_buf(),
                    dst: dst.to_path_buf(),
                    error: None,
                })
                .await;
            }
            scanning::Entry::File { src, dst } => {
                self.send_event(Event::ReplaceStarted {
                    index: index.clone(),
                    src: src.to_path_buf(),
                    dst: dst.to_path_buf(),
                })
                .await;

                if let Err(source) = self.ensure_file(src, dst).await {
                    self.send_event(Event::ReplaceFinished {
                        index: index.clone(),
                        src: src.to_path_buf(),
                        dst: dst.to_path_buf(),
                        error: Some(Error::ReplaceIo { source }),
                    })
                    .await;

                    return;
                }

                self.send_event(Event::ReplaceFinished {
                    index: index.clone(),
                    src: src.to_path_buf(),
                    dst: dst.to_path_buf(),
                    error: None,
                })
                .await;
            }
        }
    }

    async fn check_source_directory(src: &Path) -> std::io::Result<()> {
        let metadata = match tokio::fs::symlink_metadata(src).await {
            Ok(metadata) => metadata,
            Err(source) => return Err(source),
        };

        if !metadata.is_dir() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotADirectory,
                "source is not a directory",
            ));
        }

        Ok(())
    }

    async fn check_destination_directory(dst: &Path) -> std::io::Result<()> {
        let metadata = match tokio::fs::symlink_metadata(dst).await {
            Ok(metadata) => metadata,
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
                return Ok(());
            }
            Err(source) => return Err(source),
        };

        if !metadata.is_dir() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotADirectory,
                "destination exists but is not a directory",
            ));
        }

        Ok(())
    }

    async fn ensure_destination_directory(&self, dst: &Path) -> std::io::Result<()> {
        if !self.dry {
            tokio::fs::create_dir_all(dst).await?;
        }
        Ok(())
    }

    async fn check_source_file(src: &Path) -> std::io::Result<()> {
        let metadata = match tokio::fs::symlink_metadata(src).await {
            Ok(metadata) => metadata,
            Err(source) => return Err(source),
        };

        if !metadata.is_file() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "source is not a file",
            ));
        }

        Ok(())
    }

    async fn check_destination_file(dst: &Path) -> std::io::Result<()> {
        let metadata = match tokio::fs::symlink_metadata(dst).await {
            Ok(metadata) => metadata,
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
                return Ok(());
            }
            Err(source) => return Err(source),
        };

        if !metadata.is_file() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "destination exists but is not a file",
            ));
        }

        Ok(())
    }

    async fn ensure_file(&self, src: &Path, dst: &Path) -> std::io::Result<()> {
        if !self.dry {
            tokio::fs::copy(src, dst).await?;
        }
        Ok(())
    }

    async fn replace_destination_directory(&self, dst: &Path) -> std::io::Result<()> {
        if !self.dry {
            match tokio::fs::remove_dir_all(dst).await {
                Ok(()) => {}
                Err(source) if source.kind() == std::io::ErrorKind::NotFound => {}
                Err(source) => return Err(source),
            }
        }
        self.ensure_destination_directory(dst).await?;
        Ok(())
    }

    fn collect_node_child_indexes(node: &scanning::Node) -> Vec<Index> {
        let mut indexes = Vec::new();
        Self::subcollect_node_child_indexes(&mut indexes, node);
        indexes
    }

    fn subcollect_nodes_child_indexes(indexes: &mut Vec<Index>, nodes: &[scanning::Node]) {
        for node in nodes.iter() {
            Self::subcollect_node_child_indexes(indexes, node);
        }
    }

    fn subcollect_node_child_indexes(indexes: &mut Vec<Index>, node: &scanning::Node) {
        match node {
            scanning::Node::EnsureDirectory { nodes, .. } => {
                Self::subcollect_nodes_child_indexes(indexes, nodes);
            }
            scanning::Node::ReplaceFile { index, .. }
            | scanning::Node::ReplaceDirectory { index, .. } => {
                indexes.push(index.clone());
            }
        }
    }

    async fn send_event(&self, event: Event) {
        let _ = self.event_sender.send(event).await;
    }
}

#[cfg(test)]
mod tests {
    use super::super::testing as testing_e;
    use crate::{manifest::testing as testing_m, scanning::testing as testing_s, traversal};
    use tempfile::TempDir;

    #[tokio::test]
    async fn executes_scanned_nodes() {
        let temp_dir = TempDir::new().unwrap();
        let src_dir = temp_dir.path().join("src");
        let dst_dir = temp_dir.path().join("dst");

        let paths = [
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
        ];

        for path in paths {
            testing_m::write(&src_dir.join(path), path);
        }

        let (nodes, scan_events) = testing_s::scan([
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

        testing_s::assert_events_contain_no_errors(&scan_events);
        testing_s::assert_events_contain_done_last(&scan_events);

        let execution_events = testing_e::execute(false, &nodes).await;

        testing_e::assert_events_contain_no_ensure_errors(&execution_events);
        testing_e::assert_events_contain_no_replace_errors(&execution_events);
        testing_e::assert_events_contain_done_last(&execution_events);

        for path in paths {
            assert!(dst_dir.join(path).is_file());
        }
    }
}
