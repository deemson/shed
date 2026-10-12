pub type IndexBit = usize;

pub type IndexRef<'a> = &'a [IndexBit];
pub struct FullIndexRef<'a> {
    pub manifest: IndexRef<'a>,
    pub item: IndexRef<'a>,
}

pub type Index = Vec<IndexBit>;

#[derive(Debug, Eq, Hash, PartialEq, Clone)]
pub struct FullIndex {
    pub manifest: Index,
    pub item: Index,
}

impl FullIndex {
    pub fn as_ref(&self) -> FullIndexRef<'_> {
        FullIndexRef {
            manifest: &self.manifest,
            item: &self.item,
        }
    }
}

impl<M, I> From<(M, I)> for FullIndex
where
    M: IntoIterator<Item = IndexBit>,
    I: IntoIterator<Item = IndexBit>,
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
        let actual: FullIndex = ([0, 1], vec![2, 3]).into();
        let expected = FullIndex {
            manifest: vec![0, 1],
            item: vec![2, 3],
        };

        assert_eq!(actual, expected);
    }
}
