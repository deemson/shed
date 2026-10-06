use super::{event::*, model::*};
use crate::traversal;
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
        todo!()
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

        let (actual, _) = testing_s::scan([
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
