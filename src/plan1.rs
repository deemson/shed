#![allow(dead_code, unused_variables)]
use std::path::PathBuf;

use crate::manifest_yaml::ManifestFile;

#[derive(Debug, PartialEq)]
pub struct Dir {
    pub dst: PathBuf,
    pub items: Vec<Item>,
}

#[derive(Debug, PartialEq)]
pub struct Leaf {
    pub src: PathBuf,
    pub dst: PathBuf,
}

#[derive(Debug, PartialEq)]
pub enum Item {
    Dir(Dir),
    Leaf(Leaf),
}

pub enum Direction {
    Put,
    Get,
}

pub struct Manifest {
    location: PathBuf,
    file: ManifestFile,
}

impl Manifest {
    pub fn new(location: impl Into<PathBuf>, file: ManifestFile) -> Self {
        Self {
            location: location.into(),
            file,
        }
    }
}

pub struct Planner {
    direction: Direction,
}

impl Planner {
    pub fn new(direction: Direction) -> Self {
        Self { direction }
    }

    fn plan(&self, manifests: impl IntoIterator<Item = Manifest>) -> Vec<Item> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore]
    fn some() {
        #[rustfmt::skip]
        let ms1 = [
            "items:",
            "  - path: /dir/dst-item1",
            "    shed: src-item1",
            "  - path: /dir/dst-item2",
            "    shed: src-item2",
        ].join("\n");
        let manifest1 = Manifest::new("/manifest1", ManifestFile::try_from(ms1).unwrap());

        #[rustfmt::skip]
        let ms2 = [
            "items:",
            "  - path: /dir/dst-item3",
            "    shed: src-item3",
            "  - path: /dir/dst-item4",
            "    shed: src-item4",
        ].join("\n");
        let manifest2 = Manifest::new("/manifest2", ManifestFile::try_from(ms2).unwrap());

        let actual = Planner::new(Direction::Get).plan([manifest1, manifest2]);

        let expected = vec![Item::Dir(Dir {
            dst: PathBuf::from("/dir"),
            items: vec![
                Item::Leaf(Leaf {
                    src: PathBuf::from("/manifest1/src-item1"),
                    dst: PathBuf::from("/dir/dst-item1"),
                }),
                Item::Leaf(Leaf {
                    src: PathBuf::from("/manifest1/src-item2"),
                    dst: PathBuf::from("/dir/dst-item2"),
                }),
                Item::Leaf(Leaf {
                    src: PathBuf::from("/manifest2/src-item3"),
                    dst: PathBuf::from("/dir/dst-item3"),
                }),
                Item::Leaf(Leaf {
                    src: PathBuf::from("/manifest2/src-item4"),
                    dst: PathBuf::from("/dir/dst-item4"),
                }),
            ],
        })];

        assert_eq!(actual, expected);
    }

    #[test]
    #[ignore]
    fn some2() {
        let ms1 = [
            "items:",
            "  - path: /dir/dst-dir-item1",
            "    shed: src-dir-item1",
            "  - path: /dir/subdir/dst-subdir-item1",
            "    shed: src-subdir-item1",
        ]
        .join("\n");
        let manifest1 = Manifest::new("/manifest1", ManifestFile::try_from(ms1).unwrap());

        #[rustfmt::skip]
        let ms2 = [
            "items:",
            "  - path: /dir/dst-dir-item2",
            "    shed: src-dir-item2",
            "  - path: /dir/subdir/dst-subdir-item2",
            "    shed: src-subdir-item2",
        ].join("\n");
        let manifest2 = Manifest::new("/manifest2", ManifestFile::try_from(ms2).unwrap());

        let actual = Planner::new(Direction::Get).plan([manifest1, manifest2]);

        let expected = vec![Item::Dir(Dir {
            dst: PathBuf::from("/dir"),
            items: vec![
                Item::Leaf(Leaf {
                    src: PathBuf::from("/manifest1/src-dir-item1"),
                    dst: PathBuf::from("/dir/dst-dir-item1"),
                }),
                Item::Dir(Dir {
                    dst: PathBuf::from("/dir/subdir"),
                    items: vec![
                        Item::Leaf(Leaf {
                            src: PathBuf::from("/manifest1/src-subdir-item1"),
                            dst: PathBuf::from("/dir/subdir/dst-dir-item1"),
                        }),
                        Item::Leaf(Leaf {
                            src: PathBuf::from("/manifest2/src-subdir-item2"),
                            dst: PathBuf::from("/dir/subdir/dst-dir-item1"),
                        }),
                    ],
                }),
                Item::Leaf(Leaf {
                    src: PathBuf::from("/manifest2/src-dir-item2"),
                    dst: PathBuf::from("/dir/dst-dir-item2"),
                }),
            ],
        })];

        assert_eq!(actual, expected);
    }
}
