pub type Index = Vec<usize>;

#[derive(Debug, Eq, Hash, PartialEq)]
pub struct FullIndex {
    pub manifest: Index,
    pub item: Index,
}

impl<M, I> From<(M, I)> for FullIndex
where
    M: IntoIterator<Item = usize>,
    I: IntoIterator<Item = usize>,
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
