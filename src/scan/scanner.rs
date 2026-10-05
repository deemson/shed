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
        traversal_roots: impl IntoIterator<Item = traversal::Root>,
    ) -> Vec<Root> {
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

        testing_m::write(&src_dir.join("dir1/dir11/dir111/file1111"), "1111");
        testing_m::write(&src_dir.join("dir1/dir11/dir111/file1112"), "1112");
        testing_m::write(&src_dir.join("dir1/dir11/file111"), "111");
        testing_m::write(&src_dir.join("dir1/dir11/file112"), "112");
        testing_m::write(&src_dir.join("dir1/file11"), "11");
        testing_m::write(&src_dir.join("dir1/file12"), "12");
        testing_m::write(&src_dir.join("dir1/dir12/dir121/file1211"), "1211");
        testing_m::write(&src_dir.join("dir1/dir12/dir121/file1212"), "1212");
        testing_m::write(&src_dir.join("dir1/dir12/dir122/file1221"), "1221");
        testing_m::write(&src_dir.join("dir1/dir12/dir122/file1222"), "1222");
        testing_m::write(&src_dir.join("dir1/dir12/file121"), "121");
        testing_m::write(&src_dir.join("dir1/dir12/file122"), "122");
        testing_m::write(&src_dir.join("dir2/dir21/file211"), "211");
        testing_m::write(&src_dir.join("dir2/dir21/file212"), "212");
        testing_m::write(&src_dir.join("dir2/dir22/file221"), "221");
        testing_m::write(&src_dir.join("dir2/dir22/file222"), "222");
        testing_m::write(&src_dir.join("dir2/file21"), "21");
        testing_m::write(&src_dir.join("dir2/file22"), "22");
        testing_m::write(&src_dir.join("file1"), "1");
        testing_m::write(&src_dir.join("file2"), "2");

        let (actual, _) = testing_s::scan([
            traversal::Root::Directory {
                dst: dst_dir.join("dir1"),
                directories: vec![traversal::Directory {
                    dst: "dir11".into(),
                    directories: Vec::new(),
                    leaves: vec![
                        traversal::Leaf {
                            index: ([0], [0, 0, 0]).into(),
                            src: src_dir.join("dir1/dir11/dir111"),
                            dst: "dir111".into(),
                        },
                        traversal::Leaf {
                            index: ([0], [0, 0, 1]).into(),
                            src: src_dir.join("dir1/dir11/file111"),
                            dst: "file111".into(),
                        },
                        traversal::Leaf {
                            index: ([0], [0, 0, 2]).into(),
                            src: src_dir.join("dir1/dir11/file112"),
                            dst: "file112".into(),
                        },
                    ],
                }],
                leaves: vec![
                    traversal::Leaf {
                        index: ([0], [0, 1]).into(),
                        src: src_dir.join("dir1/dir12"),
                        dst: "dir12".into(),
                    },
                    traversal::Leaf {
                        index: ([0], [0, 2]).into(),
                        src: src_dir.join("dir1/file11"),
                        dst: "file11".into(),
                    },
                    traversal::Leaf {
                        index: ([0], [0, 3]).into(),
                        src: src_dir.join("dir1/file12"),
                        dst: "file12".into(),
                    },
                ],
            },
            traversal::Root::Leaf {
                index: ([0], [1]).into(),
                src: src_dir.join("dir2"),
                dst: dst_dir.join("dir2"),
            },
            traversal::Root::Leaf {
                index: ([0], [2]).into(),
                src: src_dir.join("file1"),
                dst: dst_dir.join("file1"),
            },
            traversal::Root::Leaf {
                index: ([0], [3]).into(),
                src: src_dir.join("file2"),
                dst: dst_dir.join("file2"),
            },
        ])
        .await;

        let expected = vec![
            Root::Directory {
                dst: dst_dir.join("dir1"),
                directories: Vec::new(),
                leaves: vec![],
            },
            Root::Directory {
                dst: dst_dir.join("dir2"),
                directories: vec![],
                leaves: vec![],
            },
            Root::File {
                index: ([0], [2]).into(),
                src: src_dir.join("file1"),
                dst: dst_dir.join("file1"),
            },
            Root::File {
                index: ([0], [3]).into(),
                src: src_dir.join("file2"),
                dst: dst_dir.join("file2"),
            },
        ];

        assert_eq!(actual, expected);
    }
}
