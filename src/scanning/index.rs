use crate::index;

pub struct IndexRef<'a> {
    pub manifest: index::Ref<'a>,
    pub item: index::Ref<'a>,
    pub entry: index::Ref<'a>,
}

#[derive(Debug, Eq, Hash, PartialEq, Clone)]
pub struct Index {
    pub manifest: index::Index,
    pub item: index::Index,
    pub entry: index::Index,
}

impl Index {
    pub fn as_ref(&self) -> IndexRef<'_> {
        IndexRef {
            manifest: &self.manifest,
            item: &self.item,
            entry: &self.entry,
        }
    }
}
