use crate::index;

pub struct IndexRef<'a> {
    pub manifest: index::Ref<'a>,
    pub item: index::Ref<'a>,
}

#[derive(Debug, Eq, Hash, PartialEq, Clone)]
pub struct Index {
    pub manifest: index::Index,
    pub item: index::Index,
}

impl Index {
    pub fn as_ref(&self) -> IndexRef<'_> {
        IndexRef {
            manifest: &self.manifest,
            item: &self.item,
        }
    }
}

impl<M, I> From<(M, I)> for Index
where
    M: IntoIterator<Item = index::Bit>,
    I: IntoIterator<Item = index::Bit>,
{
    fn from((manifest, item): (M, I)) -> Self {
        Self {
            manifest: manifest.into_iter().collect(),
            item: item.into_iter().collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_full_index_from_iterables() {
        let actual: Index = ([0, 1], vec![2, 3]).into();
        let expected = Index {
            manifest: vec![0, 1],
            item: vec![2, 3],
        };

        assert_eq!(actual, expected);
    }
}
